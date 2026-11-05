use super::*;
use lc631_solvers::{
    protocol::sexpr::{self, NodeId, SExprDocument, SExprKind, SExprLimits},
    typed_query::{BoolExpr, SmtEmitLimits},
};

pub(super) struct Builder<'source, 'document> {
    pub(super) source: &'source SourceStore,
    pub(super) doc: &'document SExprDocument<'source>,
    pub(super) symbols: Vec<SymbolBinding>,
    pub(super) booleans: Vec<BoolSymbolId>,
    pub(super) integers: Vec<IntSymbolId>,
    pub(super) assertions: Vec<BoolExpr>,
    pub(super) assertion_spans: Vec<SourceSpan>,
    pub(super) logic: Option<DeclaredLogic>,
    pub(super) goal: Option<(ObjectiveDirection, IntSymbolId, SourceSpan)>,
    pub(super) checked_sat: bool,
    pub(super) nodes: usize,
}
impl Builder<'_, '_> {
    pub(super) fn span(&self, node: NodeId) -> SourceSpan {
        let raw = self
            .doc
            .node(node)
            .expect("NodeId came from this document")
            .span();
        SourceSpan::checked(self.source, raw.start(), raw.end())
            .expect("parser spans preserve source UTF-8 boundaries")
    }
    pub(super) fn error(&self, node: NodeId, kind: FormalErrorKind) -> FormalError {
        FormalError {
            source: self.source.clone(),
            span: self.span(node),
            kind,
        }
    }
    pub(super) fn atom(&self, node: NodeId) -> Option<&str> {
        if self.doc.node(node)?.kind() != SExprKind::Atom {
            return None;
        }
        std::str::from_utf8(self.doc.raw(node)?).ok()
    }
    pub(super) fn children(&self, node: NodeId) -> Result<Vec<NodeId>, FormalError> {
        let children = self
            .doc
            .children(node)
            .ok_or_else(|| self.error(node, FormalErrorKind::MalformedCommand))?;
        let count = children.count();
        let mut result = Vec::new();
        result
            .try_reserve_exact(count)
            .map_err(|_| self.error(node, FormalErrorKind::AllocationFailed))?;
        result.extend(self.doc.children(node).expect("node was checked as a list"));
        Ok(result)
    }
    pub(super) fn symbol(&self, node: NodeId) -> Result<FormalSymbol, FormalError> {
        let text = self.atom(node).ok_or_else(|| {
            self.error(
                node,
                FormalErrorKind::Unsupported(ResidualReason::UnsupportedSymbol),
            )
        })?;
        for symbol in &self.symbols {
            if self.source.slice(symbol.span).ok() == Some(text) {
                return Ok(symbol.symbol);
            }
        }
        Err(self.error(node, FormalErrorKind::UndeclaredSymbol))
    }
    pub(super) fn visit(&mut self, node: NodeId, depth: usize) -> Result<(), FormalError> {
        self.nodes += 1;
        if self.nodes > MAX_NODES || depth > MAX_DEPTH {
            return Err(self.error(node, FormalErrorKind::LimitExceeded));
        }
        Ok(())
    }
    fn declaration(&mut self, node: NodeId, name: NodeId, sort: NodeId) -> Result<(), FormalError> {
        let text = self.atom(name).ok_or_else(|| {
            self.error(
                node,
                FormalErrorKind::Unsupported(ResidualReason::UnsupportedSymbol),
            )
        })?;
        if [
            "true", "false", "and", "or", "not", "let", "forall", "exists", "Bool", "Int",
        ]
        .contains(&text)
        {
            return Err(self.error(name, FormalErrorKind::ReservedSymbol));
        }
        if !simple_symbol(text) {
            return Err(self.error(
                node,
                FormalErrorKind::Unsupported(ResidualReason::UnsupportedSymbol),
            ));
        }
        if self
            .symbols
            .iter()
            .any(|old| self.source.slice(old.span).ok() == Some(text))
        {
            return Err(self.error(name, FormalErrorKind::DuplicateSymbol));
        }
        let symbol = match self.atom(sort) {
            Some("Bool") => {
                if self.booleans.len() >= 256 {
                    return Err(self.error(node, FormalErrorKind::LimitExceeded));
                }
                let id = BoolSymbolId::new(self.booleans.len() as u32);
                self.booleans.push(id);
                FormalSymbol::Boolean(id)
            }
            Some("Int") if self.logic == Some(DeclaredLogic::QfLia) => {
                if self.integers.len() >= 256 {
                    return Err(self.error(node, FormalErrorKind::LimitExceeded));
                }
                let id = IntSymbolId::new(self.integers.len() as u32);
                self.integers.push(id);
                FormalSymbol::Integer(id)
            }
            _ => {
                return Err(self.error(
                    sort,
                    FormalErrorKind::Unsupported(ResidualReason::UnsupportedLogic),
                ))
            }
        };
        self.symbols.push(SymbolBinding {
            span: self.span(name),
            declaration: self.span(node),
            symbol,
        });
        Ok(())
    }
    fn command(&mut self, node: NodeId) -> Result<(), FormalError> {
        if self.checked_sat {
            return Err(self.error(
                node,
                FormalErrorKind::Unsupported(ResidualReason::CommandAfterCheckSat),
            ));
        }
        let fields = self.children(node)?;
        let Some(head) = fields.first().and_then(|n| self.atom(*n)) else {
            return Err(self.error(node, FormalErrorKind::MalformedCommand));
        };
        if head == "set-logic" {
            if fields.len() != 2
                || self.logic.is_some()
                || !self.symbols.is_empty()
                || !self.assertions.is_empty()
            {
                return Err(self.error(node, FormalErrorKind::MalformedCommand));
            }
            self.logic = Some(match self.atom(fields[1]) {
                Some("QF_LIA") => DeclaredLogic::QfLia,
                Some("QF_UF") => DeclaredLogic::Boolean,
                _ => {
                    return Err(self.error(
                        node,
                        FormalErrorKind::Unsupported(ResidualReason::UnsupportedLogic),
                    ))
                }
            });
            return Ok(());
        }
        if self.logic.is_none() {
            return Err(self.error(node, FormalErrorKind::MissingLogic));
        }
        match head {
            "declare-const" if fields.len() == 3 => self.declaration(node, fields[1], fields[2]),
            "declare-fun" if fields.len() == 4 => {
                if !self.children(fields[2])?.is_empty() {
                    return Err(self.error(
                        node,
                        FormalErrorKind::Unsupported(ResidualReason::UnsupportedCommand),
                    ));
                }
                self.declaration(node, fields[1], fields[3])
            }
            "assert" if fields.len() == 2 => {
                if self.assertions.len() >= 256 {
                    return Err(self.error(node, FormalErrorKind::LimitExceeded));
                }
                let expr = self.boolean(fields[1], 1)?;
                self.assertions.push(expr);
                self.assertion_spans.push(self.span(node));
                Ok(())
            }
            "check-sat" if fields.len() == 1 => {
                self.checked_sat = true;
                Ok(())
            }
            "minimize" | "maximize" if fields.len() == 2 => {
                if self.goal.is_some() {
                    return Err(self.error(node, FormalErrorKind::MultipleGoals));
                }
                let symbol = match self.symbol(fields[1])? {
                    FormalSymbol::Integer(id) => id,
                    FormalSymbol::Boolean(_) => {
                        return Err(self.error(fields[1], FormalErrorKind::SortMismatch))
                    }
                };
                let direction = if head == "minimize" {
                    ObjectiveDirection::Minimize
                } else {
                    ObjectiveDirection::Maximize
                };
                self.goal = Some((direction, symbol, self.span(node)));
                Ok(())
            }
            "declare-const" | "declare-fun" | "assert" | "check-sat" | "minimize" | "maximize" => {
                Err(self.error(node, FormalErrorKind::MalformedCommand))
            }
            _ => Err(self.error(
                node,
                FormalErrorKind::Unsupported(ResidualReason::UnsupportedCommand),
            )),
        }
    }
}
fn simple_symbol(text: &str) -> bool {
    let mut bytes = text.bytes();
    bytes
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

pub(super) fn lower<'view>(
    source: &SourceStore,
    context: ContextRevision,
    scope: ContextScope,
    current: &'view ResourceReservationView<'_>,
    claim: ResourceClaim<'view>,
) -> Result<FormalLowering<'view>, FormalError> {
    let whole = SourceSpan::checked(source, 0, source.text().len())
        .expect("whole SourceStore span is valid");
    let fail = |kind| FormalError {
        source: source.clone(),
        span: whole,
        kind,
    };
    let limits = SExprLimits::checked(MAX_SOURCE_BYTES, MAX_NODES, MAX_DEPTH)
        .map_err(|e| fail(FormalErrorKind::Syntax(e)))?;
    let document = sexpr::parse(source.text().as_bytes(), limits)
        .map_err(|e| fail(FormalErrorKind::Syntax(e)))?;
    let mut builder = Builder {
        source,
        doc: &document,
        symbols: Vec::new(),
        booleans: Vec::new(),
        integers: Vec::new(),
        assertions: Vec::new(),
        assertion_spans: Vec::new(),
        logic: None,
        goal: None,
        checked_sat: false,
        nodes: 0,
    };
    // Reserve bounded top-level tables before declaration/assertion insertion.
    builder
        .symbols
        .try_reserve_exact(512)
        .map_err(|_| fail(FormalErrorKind::AllocationFailed))?;
    builder
        .booleans
        .try_reserve_exact(256)
        .map_err(|_| fail(FormalErrorKind::AllocationFailed))?;
    builder
        .integers
        .try_reserve_exact(256)
        .map_err(|_| fail(FormalErrorKind::AllocationFailed))?;
    builder
        .assertions
        .try_reserve_exact(256)
        .map_err(|_| fail(FormalErrorKind::AllocationFailed))?;
    builder
        .assertion_spans
        .try_reserve_exact(256)
        .map_err(|_| fail(FormalErrorKind::AllocationFailed))?;
    for root in document.roots() {
        if let Err(error) = builder.command(*root) {
            current
                .check()
                .map_err(|e| fail(FormalErrorKind::Resource(e)))?;
            if let FormalErrorKind::Unsupported(reason) = error.kind {
                return residual(
                    source,
                    context,
                    scope,
                    builder.logic,
                    builder.span(*root),
                    error.span,
                    reason,
                    claim,
                );
            }
            return Err(error);
        }
    }
    current
        .check()
        .map_err(|e| fail(FormalErrorKind::Resource(e)))?;
    let logic = builder
        .logic
        .ok_or_else(|| fail(FormalErrorKind::MissingLogic))?;
    if !builder.checked_sat {
        return Err(fail(FormalErrorKind::MissingCheckSat));
    }
    let emit = SmtEmitLimits::checked(MAX_EMITTED_BYTES, 256, 256, 256, MAX_NODES, MAX_DEPTH)
        .map_err(|e| fail(FormalErrorKind::Emission(e)))?;
    let query =
        SmtStatusQuery::checked(builder.booleans, builder.integers, builder.assertions, emit)
            .map_err(|e| fail(FormalErrorKind::Emission(e)))?;
    let goal = if let Some((direction, symbol, span)) = builder.goal {
        match lc631_solvers::smt::native_probe::bounded::FiniteObjectiveBounds::from_query(
            &query, symbol,
        ) {
            Ok(bounds) => Some(ExplicitGoal {
                direction,
                symbol,
                span,
                lower: bounds.lower(),
                upper: bounds.upper(),
            }),
            Err(_) => {
                return residual(
                    source,
                    context,
                    scope,
                    Some(logic),
                    span,
                    span,
                    ResidualReason::UnboundedOrUnsupportedGoal,
                    claim,
                )
            }
        }
    } else {
        None
    };
    Ok(FormalLowering::Checked(FormalProblem {
        source: source.clone(),
        query,
        context,
        scope,
        logic,
        goal,
        symbols: builder.symbols,
        assertion_spans: builder.assertion_spans,
        _claim: claim,
    }))
}

#[allow(clippy::too_many_arguments)]
fn residual<'view>(
    source: &SourceStore,
    context: ContextRevision,
    scope: ContextScope,
    logic: Option<DeclaredLogic>,
    span: SourceSpan,
    detail: SourceSpan,
    reason: ResidualReason,
    claim: ResourceClaim<'view>,
) -> Result<FormalLowering<'view>, FormalError> {
    let mut residuals = Vec::new();
    residuals.try_reserve_exact(2).map_err(|_| FormalError {
        source: source.clone(),
        span,
        kind: FormalErrorKind::AllocationFailed,
    })?;
    residuals.push(FormalResidual {
        span,
        detail,
        reason,
    });
    if reason != ResidualReason::UnboundedOrUnsupportedGoal && span.end() < source.text().len() {
        let tail = SourceSpan::checked(source, span.end(), source.text().len())
            .expect("command end is a source boundary");
        residuals.push(FormalResidual {
            span: tail,
            detail: tail,
            reason: ResidualReason::UnprocessedRemainder,
        });
    }
    Ok(FormalLowering::Unsupported(FormalResidualReport {
        source: source.clone(),
        context,
        scope,
        logic,
        residuals,
        _claim: claim,
    }))
}
