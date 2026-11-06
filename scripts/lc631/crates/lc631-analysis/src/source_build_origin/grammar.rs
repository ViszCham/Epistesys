//! 日本語: pin済Rust CSTを通した単一safe関数の有限文法。意味的正答は判定しない。
//! English: A finite single-safe-function grammar after pinned Rust CST; no semantic score.
use super::BuildError;

const LIMIT: usize = 512;
const PARSER: &str = "tree-sitter-rust-0.24.2-tree-sitter-0.27.0";
pub(super) fn admit(source: &str) -> Result<(), BuildError> {
    if source.bytes().any(|b| b == b'\x60' || !b.is_ascii()) {
        return Err(BuildError::Unsupported(
            "subject outside ASCII finite Rust profile",
        ));
    }
    let mut wrapped = String::new();
    wrapped.try_reserve_exact(source.len() + 14).map_err(|_| {
        BuildError::Resource(lc631_substrate::resource::ResourceError::AllocationFailed)
    })?;
    wrapped.push_str("\x60\x60\x60rust\n");
    wrapped.push_str(source);
    wrapped.push_str("\n\x60\x60\x60\n");
    let report = lc631_tldg::analyze_dg1(
        &wrapped,
        lc631_tldg::Dg1Budget {
            max_source_bytes: 32784,
            max_region_depth: 8,
            max_regions: 64,
            max_syntax_nodes: 4096,
            max_requirements: 16,
            max_alternatives: 4,
        },
    )
    .map_err(|_| BuildError::Unsupported("bounded Rust CST unavailable"))?;
    let tree = report
        .rust_trees
        .first()
        .ok_or(BuildError::Unsupported("Rust CST missing"))?;
    if report.rust_trees.len() != 1
        || tree.grammar_revision != PARSER
        || tree.root_has_error
        || tree
            .nodes
            .iter()
            .any(|n| n.has_error || n.is_error || n.missing)
        || tree
            .nodes
            .iter()
            .filter(|n| n.kind == "function_item")
            .count()
            != 1
        || tree.nodes.iter().any(|n| {
            matches!(
                n.kind.as_str(),
                "mod_item"
                    | "use_declaration"
                    | "extern_crate_declaration"
                    | "attribute_item"
                    | "macro_invocation"
                    | "unsafe_block"
                    | "foreign_mod_item"
            )
        })
    {
        return Err(BuildError::Unsupported(
            "subject is not one safe standalone function",
        ));
    }
    let root = tree
        .nodes
        .iter()
        .find(|n| n.parent_id.is_none())
        .ok_or(BuildError::Unsupported("Rust CST root missing"))?;
    if tree.nodes.iter().any(|n| {
        n.parent_id == Some(root.id)
            && n.named
            && !matches!(
                n.kind.as_str(),
                "function_item" | "line_comment" | "block_comment"
            )
    }) {
        return Err(BuildError::Unsupported(
            "top-level item outside standalone profile",
        ));
    }
    let mut parser = Tokens {
        items: tokenize(source)?,
        at: 0,
        args: ["", ""],
    };
    for t in ["pub", "fn", "checked_add", "("] {
        parser.want(t)?;
    }
    parser.args[0] = parser.parameter()?;
    parser.want(":")?;
    parser.want("u32")?;
    parser.want(",")?;
    parser.args[1] = parser.parameter()?;
    if parser.args[0] == parser.args[1] {
        return Err(BuildError::FrameMismatch);
    }
    for t in [":", "u32", ")", "->", "Option", "<", "u32", ">", "{"] {
        parser.want(t)?;
    }
    let returns = parser.take("return");
    parser.option(0)?;
    if returns {
        parser.want(";")?;
    }
    parser.want("}")?;
    if parser.at != parser.items.len() {
        return Err(BuildError::Unsupported("extra subject tokens"));
    }
    Ok(())
}
fn tokenize(source: &str) -> Result<Vec<&str>, BuildError> {
    let b = source.as_bytes();
    let mut tokens = Vec::new();
    tokens.try_reserve_exact(LIMIT).map_err(|_| {
        BuildError::Resource(lc631_substrate::resource::ResourceError::AllocationFailed)
    })?;
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if b.get(i..i + 2) == Some(b"//") {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b.get(i..i + 2) == Some(b"/*") {
            i += 2;
            let mut nesting = 1usize;
            while i < b.len() && nesting != 0 {
                if b.get(i..i + 2) == Some(b"/*") {
                    nesting += 1;
                    i += 2;
                    if nesting > 16 {
                        return Err(BuildError::InputLimit);
                    }
                } else if b.get(i..i + 2) == Some(b"*/") {
                    nesting -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            if nesting != 0 {
                return Err(BuildError::Unsupported("unterminated comment"));
            }
            continue;
        }
        let start = i;
        if b[i].is_ascii_alphabetic() || b[i] == b'_' {
            i += 1;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
        } else if b[i].is_ascii_digit() {
            i += 1;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
        } else if b.get(i..i + 2) == Some(b"->") {
            i += 2;
        } else if b"():,<>{}.;".contains(&b[i]) {
            i += 1;
        } else {
            return Err(BuildError::Unsupported(
                "token outside finite arithmetic profile",
            ));
        }
        if tokens.len() == LIMIT {
            return Err(BuildError::InputLimit);
        }
        tokens.push(&source[start..i]);
    }
    Ok(tokens)
}
struct Tokens<'a> {
    items: Vec<&'a str>,
    at: usize,
    args: [&'a str; 2],
}
impl<'a> Tokens<'a> {
    fn next(&mut self) -> Result<&'a str, BuildError> {
        let t = self
            .items
            .get(self.at)
            .copied()
            .ok_or(BuildError::FrameMismatch)?;
        self.at += 1;
        Ok(t)
    }
    fn want(&mut self, token: &str) -> Result<(), BuildError> {
        if self.next()? != token {
            return Err(BuildError::FrameMismatch);
        }
        Ok(())
    }
    fn take(&mut self, token: &str) -> bool {
        if self.items.get(self.at) == Some(&token) {
            self.at += 1;
            true
        } else {
            false
        }
    }
    fn parameter(&mut self) -> Result<&'a str, BuildError> {
        let t = self.next()?;
        if t == "_"
            || !t
                .as_bytes()
                .first()
                .is_some_and(|b| b.is_ascii_alphabetic() || *b == b'_')
            || ["pub", "fn", "return", "Self", "self"].contains(&t)
        {
            return Err(BuildError::FrameMismatch);
        }
        Ok(t)
    }
    fn option(&mut self, depth: usize) -> Result<(), BuildError> {
        if depth > 16 {
            return Err(BuildError::InputLimit);
        }
        if self.take("None") {
            return Ok(());
        }
        if self.take("Some") {
            self.want("(")?;
            self.number(depth + 1)?;
            return self.want(")");
        }
        let var = self.next()?;
        if !self.args.contains(&var) {
            return Err(BuildError::Unsupported("unregistered function"));
        }
        for t in [".", "checked_add", "("] {
            self.want(t)?;
        }
        self.number(depth + 1)?;
        self.want(")")
    }
    fn number(&mut self, depth: usize) -> Result<(), BuildError> {
        if depth > 16 {
            return Err(BuildError::InputLimit);
        }
        if self.take("(") {
            self.number(depth + 1)?;
            return self.want(")");
        }
        let token = self.next()?;
        if token.bytes().all(|b| b.is_ascii_digit()) {
            token
                .parse::<u32>()
                .map_err(|_| BuildError::Unsupported("u32 literal out of range"))?;
        } else if !self.args.contains(&token) {
            return Err(BuildError::Unsupported("identifier outside arguments"));
        }
        let mut methods = 0;
        while self.take(".") {
            methods += 1;
            if methods > 16 {
                return Err(BuildError::InputLimit);
            }
            if ![
                "wrapping_add",
                "wrapping_sub",
                "wrapping_mul",
                "saturating_add",
                "saturating_sub",
            ]
            .contains(&self.next()?)
            {
                return Err(BuildError::Unsupported(
                    "method outside finite arithmetic profile",
                ));
            }
            self.want("(")?;
            self.number(depth + 1)?;
            self.want(")")?;
        }
        Ok(())
    }
}
