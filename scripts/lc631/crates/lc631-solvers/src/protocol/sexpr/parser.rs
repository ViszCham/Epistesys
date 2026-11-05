use super::{
    ByteSpan, ListFrame, NodeId, SExprDocument, SExprError, SExprKind, SExprLimits, SExprNode,
};

/// Parse UTF-8 S-expression syntax under explicit byte, node and depth limits.
/// Semantics, atom meaning and solver-result validity are intentionally out of
/// scope. Strings use SMT-LIB doubled-quote escaping; quoted symbols reject
/// backslashes and require a closing `|`.
pub fn parse(input: &[u8], limits: SExprLimits) -> Result<SExprDocument<'_>, SExprError> {
    parse_inner(input, limits, false)
}

/// Caller must retain an actual resource claim for scratch_bytes(limits).
pub(crate) fn parse_preallocated(
    input: &[u8],
    limits: SExprLimits,
) -> Result<SExprDocument<'_>, SExprError> {
    parse_inner(input, limits, true)
}

fn parse_inner(
    input: &[u8],
    limits: SExprLimits,
    preallocate: bool,
) -> Result<SExprDocument<'_>, SExprError> {
    if input.len() > limits.max_input_bytes {
        return Err(SExprError::InputTooLarge {
            actual: input.len(),
            limit: limits.max_input_bytes,
        });
    }
    std::str::from_utf8(input).map_err(|error| SExprError::InvalidUtf8 {
        valid_up_to: error.valid_up_to(),
    })?;

    let mut nodes = Vec::new();
    let mut roots = Vec::new();
    let mut stack = Vec::<ListFrame>::new();
    if preallocate {
        nodes
            .try_reserve_exact(limits.max_nodes)
            .map_err(|_| SExprError::AllocationFailed)?;
        roots
            .try_reserve_exact(limits.max_nodes)
            .map_err(|_| SExprError::AllocationFailed)?;
        stack
            .try_reserve_exact(limits.max_depth)
            .map_err(|_| SExprError::AllocationFailed)?;
        if nodes.capacity() > limits.max_nodes
            || roots.capacity() > limits.max_nodes
            || stack.capacity() > limits.max_depth
        {
            return Err(SExprError::AllocationFailed);
        }
    }
    let mut cursor = 0;

    while cursor < input.len() {
        skip_space_and_comments(input, &mut cursor);
        if cursor == input.len() {
            break;
        }

        match input[cursor] {
            b'(' => {
                let depth = stack
                    .len()
                    .checked_add(1)
                    .ok_or(SExprError::DepthLimitExceeded)?;
                if depth > limits.max_depth {
                    return Err(SExprError::DepthLimitExceeded);
                }
                let id = push_node(
                    &mut nodes,
                    &mut roots,
                    &mut stack,
                    limits.max_nodes,
                    SExprKind::List,
                    ByteSpan::new(cursor, cursor + 1),
                    None,
                )?;
                stack
                    .try_reserve(1)
                    .map_err(|_| SExprError::AllocationFailed)?;
                stack.push(ListFrame {
                    list: id,
                    last_child: None,
                });
                cursor += 1;
            }
            b')' => {
                let frame = stack
                    .pop()
                    .ok_or(SExprError::UnexpectedClose { offset: cursor })?;
                nodes[frame.list.0].span.end = cursor + 1;
                cursor += 1;
            }
            b'"' => {
                let (end, content) = scan_string(input, cursor)?;
                ensure_token_separator(input, end)?;
                push_node(
                    &mut nodes,
                    &mut roots,
                    &mut stack,
                    limits.max_nodes,
                    SExprKind::StringLiteral,
                    ByteSpan::new(cursor, end),
                    Some(content),
                )?;
                cursor = end;
            }
            b'|' => {
                let (end, content) = scan_quoted_symbol(input, cursor)?;
                ensure_token_separator(input, end)?;
                push_node(
                    &mut nodes,
                    &mut roots,
                    &mut stack,
                    limits.max_nodes,
                    SExprKind::QuotedSymbol,
                    ByteSpan::new(cursor, end),
                    Some(content),
                )?;
                cursor = end;
            }
            _ => {
                let end = scan_atom(input, cursor)?;
                if input
                    .get(end)
                    .is_some_and(|byte| matches!(*byte, b'"' | b'|'))
                {
                    return Err(SExprError::MissingSeparator { offset: end });
                }
                let span = ByteSpan::new(cursor, end);
                push_node(
                    &mut nodes,
                    &mut roots,
                    &mut stack,
                    limits.max_nodes,
                    SExprKind::Atom,
                    span,
                    Some(span),
                )?;
                cursor = end;
            }
        }
    }

    if let Some(frame) = stack.last() {
        return Err(SExprError::UnclosedList {
            offset: nodes[frame.list.0].span.start,
        });
    }

    Ok(SExprDocument {
        source: input,
        nodes,
        roots,
    })
}

fn push_node(
    nodes: &mut Vec<SExprNode>,
    roots: &mut Vec<NodeId>,
    stack: &mut [ListFrame],
    max_nodes: usize,
    kind: SExprKind,
    span: ByteSpan,
    content_span: Option<ByteSpan>,
) -> Result<NodeId, SExprError> {
    if nodes.len() >= max_nodes {
        return Err(SExprError::NodeLimitExceeded);
    }
    if stack.is_empty() {
        roots
            .try_reserve(1)
            .map_err(|_| SExprError::AllocationFailed)?;
    }
    nodes
        .try_reserve(1)
        .map_err(|_| SExprError::AllocationFailed)?;
    let id = NodeId(nodes.len());
    let parent = stack.last().map(|frame| frame.list);
    nodes.push(SExprNode {
        id,
        kind,
        span,
        content_span,
        parent,
        first_child: None,
        next_sibling: None,
    });

    if let Some(frame) = stack.last_mut() {
        if let Some(previous) = frame.last_child {
            nodes[previous.0].next_sibling = Some(id);
        } else {
            nodes[frame.list.0].first_child = Some(id);
        }
        frame.last_child = Some(id);
    } else {
        roots.push(id);
    }
    Ok(id)
}

fn skip_space_and_comments(input: &[u8], cursor: &mut usize) {
    loop {
        while input
            .get(*cursor)
            .is_some_and(|byte| byte.is_ascii_whitespace())
        {
            *cursor += 1;
        }
        if input.get(*cursor) != Some(&b';') {
            return;
        }
        while *cursor < input.len() && input[*cursor] != b'\n' {
            *cursor += 1;
        }
    }
}

fn scan_atom(input: &[u8], start: usize) -> Result<usize, SExprError> {
    let mut end = start;
    while let Some(byte) = input.get(end) {
        if byte.is_ascii_whitespace() || matches!(*byte, b'(' | b')' | b';' | b'"' | b'|') {
            break;
        }
        end += 1;
    }
    if end == start {
        return Err(SExprError::InvalidToken { offset: start });
    }
    Ok(end)
}

fn scan_string(input: &[u8], start: usize) -> Result<(usize, ByteSpan), SExprError> {
    let mut cursor = start + 1;
    while cursor < input.len() {
        if input[cursor] == b'"' {
            if input.get(cursor + 1) == Some(&b'"') {
                cursor += 2;
            } else {
                return Ok((cursor + 1, ByteSpan::new(start + 1, cursor)));
            }
        } else {
            cursor += 1;
        }
    }
    Err(SExprError::UnclosedString { offset: start })
}

fn scan_quoted_symbol(input: &[u8], start: usize) -> Result<(usize, ByteSpan), SExprError> {
    let mut cursor = start + 1;
    while cursor < input.len() {
        match input[cursor] {
            b'|' => return Ok((cursor + 1, ByteSpan::new(start + 1, cursor))),
            b'\\' => return Err(SExprError::InvalidQuotedSymbol { offset: cursor }),
            _ => cursor += 1,
        }
    }
    Err(SExprError::UnclosedQuotedSymbol { offset: start })
}

fn ensure_token_separator(input: &[u8], end: usize) -> Result<(), SExprError> {
    if input
        .get(end)
        .is_some_and(|byte| !byte.is_ascii_whitespace() && !matches!(*byte, b'(' | b')' | b';'))
    {
        return Err(SExprError::MissingSeparator { offset: end });
    }
    Ok(())
}
