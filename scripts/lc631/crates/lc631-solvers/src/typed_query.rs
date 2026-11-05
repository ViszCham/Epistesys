//! Safe bounded emitter for typed SMT-LIB QF_LIA status queries.
//!
//! The IR is intentionally small and uses numeric symbol identities instead
//! of caller-provided names. This module emits bytes accepted by the existing
//! `SolverQuery::checked` API; it is not connected to DeepGrammar or the
//! analysis crate and does not validate formula semantics.

use crate::protocol::QueryResultKind;
use crate::registry::{BackendId, LogicProfile, MAX_SOLVER_STDIN_BYTES};
use crate::runner::{QueryError, QuerySourceIdentity, SolverQuery};

const MAX_TYPED_SYMBOLS_PER_SORT: usize = 256;
const MAX_TYPED_ASSERTIONS: usize = 1024;
const MAX_TYPED_AST_NODES: usize = 16_384;
const MAX_TYPED_AST_DEPTH: usize = 128;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BoolSymbolId(u32);

impl BoolSymbolId {
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct IntSymbolId(u32);

impl IntSymbolId {
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IntExpr {
    Constant(i64),
    Variable(IntSymbolId),
    Add(Vec<IntExpr>),
    Subtract(Box<IntExpr>, Box<IntExpr>),
    Negate(Box<IntExpr>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BoolExpr {
    Constant(bool),
    Variable(BoolSymbolId),
    Not(Box<BoolExpr>),
    And(Vec<BoolExpr>),
    Or(Vec<BoolExpr>),
    Equal(IntExpr, IntExpr),
    LessEqual(IntExpr, IntExpr),
    LessThan(IntExpr, IntExpr),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryEmitError {
    InvalidLimits,
    LimitsTooLarge,
    TooManyBooleanSymbols,
    TooManyIntegerSymbols,
    TooManyAssertions,
    DuplicateBooleanSymbol,
    DuplicateIntegerSymbol,
    UndeclaredBooleanSymbol,
    UndeclaredIntegerSymbol,
    EmptyConjunction,
    EmptyDisjunction,
    DegenerateAddition,
    NodeLimitExceeded,
    DepthLimitExceeded,
    OutputTooLarge,
    ArithmeticOverflow,
    AllocationFailed,
    SolverQuery(QueryError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SmtEmitLimits {
    max_output_bytes: usize,
    max_boolean_symbols: usize,
    max_integer_symbols: usize,
    max_assertions: usize,
    max_nodes: usize,
    max_depth: usize,
}

impl SmtEmitLimits {
    #[allow(clippy::too_many_arguments)]
    pub fn checked(
        max_output_bytes: usize,
        max_boolean_symbols: usize,
        max_integer_symbols: usize,
        max_assertions: usize,
        max_nodes: usize,
        max_depth: usize,
    ) -> Result<Self, QueryEmitError> {
        if max_output_bytes == 0 || max_nodes == 0 || max_depth == 0 {
            return Err(QueryEmitError::InvalidLimits);
        }
        if max_output_bytes > MAX_SOLVER_STDIN_BYTES
            || max_boolean_symbols > MAX_TYPED_SYMBOLS_PER_SORT
            || max_integer_symbols > MAX_TYPED_SYMBOLS_PER_SORT
            || max_assertions > MAX_TYPED_ASSERTIONS
            || max_nodes > MAX_TYPED_AST_NODES
            || max_depth > MAX_TYPED_AST_DEPTH
        {
            return Err(QueryEmitError::LimitsTooLarge);
        }
        Ok(Self {
            max_output_bytes,
            max_boolean_symbols,
            max_integer_symbols,
            max_assertions,
            max_nodes,
            max_depth,
        })
    }

    pub const fn max_output_bytes(self) -> usize {
        self.max_output_bytes
    }

    pub const fn max_boolean_symbols(self) -> usize {
        self.max_boolean_symbols
    }

    pub const fn max_integer_symbols(self) -> usize {
        self.max_integer_symbols
    }

    pub const fn max_assertions(self) -> usize {
        self.max_assertions
    }

    pub const fn max_nodes(self) -> usize {
        self.max_nodes
    }

    pub const fn max_depth(self) -> usize {
        self.max_depth
    }
}

impl Default for SmtEmitLimits {
    fn default() -> Self {
        Self {
            max_output_bytes: 1024 * 1024,
            max_boolean_symbols: MAX_TYPED_SYMBOLS_PER_SORT,
            max_integer_symbols: MAX_TYPED_SYMBOLS_PER_SORT,
            max_assertions: MAX_TYPED_ASSERTIONS,
            max_nodes: MAX_TYPED_AST_NODES,
            max_depth: MAX_TYPED_AST_DEPTH,
        }
    }
}

/// Validated source-side QF_LIA request. It keeps no unbounded text field and
/// serializes only after its symbol, node, and depth contracts pass.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmtStatusQuery {
    boolean_symbols: Vec<BoolSymbolId>,
    integer_symbols: Vec<IntSymbolId>,
    assertions: Vec<BoolExpr>,
    limits: SmtEmitLimits,
}

impl SmtStatusQuery {
    pub const fn limits(&self) -> SmtEmitLimits {
        self.limits
    }

    pub fn boolean_symbols(&self) -> &[BoolSymbolId] {
        &self.boolean_symbols
    }

    pub fn integer_symbols(&self) -> &[IntSymbolId] {
        &self.integer_symbols
    }

    pub fn assertions(&self) -> &[BoolExpr] {
        &self.assertions
    }

    pub fn checked(
        mut boolean_symbols: Vec<BoolSymbolId>,
        mut integer_symbols: Vec<IntSymbolId>,
        assertions: Vec<BoolExpr>,
        limits: SmtEmitLimits,
    ) -> Result<Self, QueryEmitError> {
        if boolean_symbols.len() > limits.max_boolean_symbols {
            return Err(QueryEmitError::TooManyBooleanSymbols);
        }
        if integer_symbols.len() > limits.max_integer_symbols {
            return Err(QueryEmitError::TooManyIntegerSymbols);
        }
        if assertions.len() > limits.max_assertions {
            return Err(QueryEmitError::TooManyAssertions);
        }

        boolean_symbols.sort_unstable();
        if boolean_symbols.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(QueryEmitError::DuplicateBooleanSymbol);
        }
        integer_symbols.sort_unstable();
        if integer_symbols.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(QueryEmitError::DuplicateIntegerSymbol);
        }

        let mut budget = AstBudget { nodes: 0, limits };
        for assertion in &assertions {
            validate_bool(
                assertion,
                1,
                &mut budget,
                &boolean_symbols,
                &integer_symbols,
            )?;
        }

        Ok(Self {
            boolean_symbols,
            integer_symbols,
            assertions,
            limits,
        })
    }

    pub fn emit(
        &self,
        source: QuerySourceIdentity,
    ) -> Result<PreparedSmtStatusQuery, QueryEmitError> {
        Ok(PreparedSmtStatusQuery {
            source,
            payload: self.emit_bytes(false)?,
        })
    }

    pub fn emit_model(
        &self,
        source: QuerySourceIdentity,
    ) -> Result<PreparedSmtModelQuery, QueryEmitError> {
        Ok(PreparedSmtModelQuery {
            source,
            payload: self.emit_bytes(true)?,
        })
    }

    fn emit_bytes(&self, model: bool) -> Result<Vec<u8>, QueryEmitError> {
        let mut output = BoundedWriter::new(self.limits.max_output_bytes);
        output.push_str("(set-logic QF_LIA)\n")?;
        if model {
            output.push_str("(set-option :produce-models true)\n")?;
        }
        for symbol in &self.boolean_symbols {
            output.push_str("(declare-const ")?;
            output.push_symbol(b'b', symbol.get())?;
            output.push_str(" Bool)\n")?;
        }
        for symbol in &self.integer_symbols {
            output.push_str("(declare-const ")?;
            output.push_symbol(b'i', symbol.get())?;
            output.push_str(" Int)\n")?;
        }
        for assertion in &self.assertions {
            output.push_str("(assert ")?;
            emit_bool(assertion, &mut output)?;
            output.push_str(")\n")?;
        }
        output.push_str("(check-sat)\n")?;
        if model {
            output.push_str("(get-model)\n")?;
        }
        Ok(output.finish())
    }
}

/// Owned payload with stable source identity; `solver_query` safely lends its
/// bytes for the lifetime of this prepared request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedSmtStatusQuery {
    source: QuerySourceIdentity,
    payload: Vec<u8>,
}

impl PreparedSmtStatusQuery {
    pub const fn source(&self) -> QuerySourceIdentity {
        self.source
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn solver_query(&self) -> Result<SolverQuery<'_>, QueryEmitError> {
        SolverQuery::checked(
            BackendId::Z3,
            self.source,
            LogicProfile::SmtBooleanQfLia,
            QueryResultKind::Satisfiability,
            &self.payload,
        )
        .map_err(QueryEmitError::SolverQuery)
    }
}

/// Owned model-query bytes; process execution and semantic checking are separate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedSmtModelQuery {
    source: QuerySourceIdentity,
    payload: Vec<u8>,
}

impl PreparedSmtModelQuery {
    pub const fn source(&self) -> QuerySourceIdentity {
        self.source
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn solver_query(&self) -> Result<SolverQuery<'_>, QueryEmitError> {
        SolverQuery::checked(
            BackendId::Z3,
            self.source,
            LogicProfile::SmtBooleanQfLia,
            QueryResultKind::ModelPayload,
            &self.payload,
        )
        .map_err(QueryEmitError::SolverQuery)
    }
}

struct AstBudget {
    nodes: usize,
    limits: SmtEmitLimits,
}

impl AstBudget {
    fn node(&mut self, depth: usize) -> Result<(), QueryEmitError> {
        if depth > self.limits.max_depth {
            return Err(QueryEmitError::DepthLimitExceeded);
        }
        self.nodes = self
            .nodes
            .checked_add(1)
            .ok_or(QueryEmitError::ArithmeticOverflow)?;
        if self.nodes > self.limits.max_nodes {
            return Err(QueryEmitError::NodeLimitExceeded);
        }
        Ok(())
    }
}

fn validate_bool(
    expression: &BoolExpr,
    depth: usize,
    budget: &mut AstBudget,
    booleans: &[BoolSymbolId],
    integers: &[IntSymbolId],
) -> Result<(), QueryEmitError> {
    budget.node(depth)?;
    match expression {
        BoolExpr::Constant(_) => Ok(()),
        BoolExpr::Variable(symbol) => {
            if booleans.binary_search(symbol).is_ok() {
                Ok(())
            } else {
                Err(QueryEmitError::UndeclaredBooleanSymbol)
            }
        }
        BoolExpr::Not(inner) => {
            validate_bool(inner, next_depth(depth)?, budget, booleans, integers)
        }
        BoolExpr::And(children) => {
            if children.len() < 2 {
                return Err(QueryEmitError::EmptyConjunction);
            }
            for child in children {
                validate_bool(child, next_depth(depth)?, budget, booleans, integers)?;
            }
            Ok(())
        }
        BoolExpr::Or(children) => {
            if children.len() < 2 {
                return Err(QueryEmitError::EmptyDisjunction);
            }
            for child in children {
                validate_bool(child, next_depth(depth)?, budget, booleans, integers)?;
            }
            Ok(())
        }
        BoolExpr::Equal(left, right)
        | BoolExpr::LessEqual(left, right)
        | BoolExpr::LessThan(left, right) => {
            validate_int(left, next_depth(depth)?, budget, integers)?;
            validate_int(right, next_depth(depth)?, budget, integers)
        }
    }
}

fn validate_int(
    expression: &IntExpr,
    depth: usize,
    budget: &mut AstBudget,
    integers: &[IntSymbolId],
) -> Result<(), QueryEmitError> {
    budget.node(depth)?;
    match expression {
        IntExpr::Constant(_) => Ok(()),
        IntExpr::Variable(symbol) => {
            if integers.binary_search(symbol).is_ok() {
                Ok(())
            } else {
                Err(QueryEmitError::UndeclaredIntegerSymbol)
            }
        }
        IntExpr::Add(children) => {
            if children.len() < 2 {
                return Err(QueryEmitError::DegenerateAddition);
            }
            for child in children {
                validate_int(child, next_depth(depth)?, budget, integers)?;
            }
            Ok(())
        }
        IntExpr::Subtract(left, right) => {
            validate_int(left, next_depth(depth)?, budget, integers)?;
            validate_int(right, next_depth(depth)?, budget, integers)
        }
        IntExpr::Negate(inner) => validate_int(inner, next_depth(depth)?, budget, integers),
    }
}

fn next_depth(depth: usize) -> Result<usize, QueryEmitError> {
    depth
        .checked_add(1)
        .ok_or(QueryEmitError::ArithmeticOverflow)
}

struct BoundedWriter {
    bytes: Vec<u8>,
    max_bytes: usize,
}

impl BoundedWriter {
    fn new(max_bytes: usize) -> Self {
        Self {
            bytes: Vec::new(),
            max_bytes,
        }
    }

    fn push_str(&mut self, text: &str) -> Result<(), QueryEmitError> {
        self.push_bytes(text.as_bytes())
    }

    fn push_symbol(&mut self, prefix: u8, id: u32) -> Result<(), QueryEmitError> {
        self.push_bytes(&[prefix])?;
        self.push_str(&id.to_string())
    }

    fn push_i64(&mut self, value: i64) -> Result<(), QueryEmitError> {
        if value < 0 {
            self.push_str("(- ")?;
            self.push_str(&value.unsigned_abs().to_string())?;
            self.push_str(")")
        } else {
            self.push_str(&value.to_string())
        }
    }

    fn push_bytes(&mut self, bytes: &[u8]) -> Result<(), QueryEmitError> {
        let next_len = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or(QueryEmitError::ArithmeticOverflow)?;
        if next_len > self.max_bytes {
            return Err(QueryEmitError::OutputTooLarge);
        }
        self.bytes
            .try_reserve(bytes.len())
            .map_err(|_| QueryEmitError::AllocationFailed)?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

fn emit_bool(expression: &BoolExpr, output: &mut BoundedWriter) -> Result<(), QueryEmitError> {
    match expression {
        BoolExpr::Constant(true) => output.push_str("true"),
        BoolExpr::Constant(false) => output.push_str("false"),
        BoolExpr::Variable(symbol) => output.push_symbol(b'b', symbol.get()),
        BoolExpr::Not(inner) => {
            output.push_str("(not ")?;
            emit_bool(inner, output)?;
            output.push_str(")")
        }
        BoolExpr::And(children) => emit_bool_list("and", children, output),
        BoolExpr::Or(children) => emit_bool_list("or", children, output),
        BoolExpr::Equal(left, right) => emit_int_comparison("=", left, right, output),
        BoolExpr::LessEqual(left, right) => emit_int_comparison("<=", left, right, output),
        BoolExpr::LessThan(left, right) => emit_int_comparison("<", left, right, output),
    }
}

fn emit_bool_list(
    operator: &str,
    children: &[BoolExpr],
    output: &mut BoundedWriter,
) -> Result<(), QueryEmitError> {
    output.push_str("(")?;
    output.push_str(operator)?;
    for child in children {
        output.push_str(" ")?;
        emit_bool(child, output)?;
    }
    output.push_str(")")
}

fn emit_int_comparison(
    operator: &str,
    left: &IntExpr,
    right: &IntExpr,
    output: &mut BoundedWriter,
) -> Result<(), QueryEmitError> {
    output.push_str("(")?;
    output.push_str(operator)?;
    output.push_str(" ")?;
    emit_int(left, output)?;
    output.push_str(" ")?;
    emit_int(right, output)?;
    output.push_str(")")
}

fn emit_int(expression: &IntExpr, output: &mut BoundedWriter) -> Result<(), QueryEmitError> {
    match expression {
        IntExpr::Constant(value) => output.push_i64(*value),
        IntExpr::Variable(symbol) => output.push_symbol(b'i', symbol.get()),
        IntExpr::Add(children) => {
            output.push_str("(+")?;
            for child in children {
                output.push_str(" ")?;
                emit_int(child, output)?;
            }
            output.push_str(")")
        }
        IntExpr::Subtract(left, right) => {
            output.push_str("(- ")?;
            emit_int(left, output)?;
            output.push_str(" ")?;
            emit_int(right, output)?;
            output.push_str(")")
        }
        IntExpr::Negate(inner) => {
            output.push_str("(- ")?;
            emit_int(inner, output)?;
            output.push_str(")")
        }
    }
}
