//! Preallocated owned copy of actual checked native evidence, not a constructor
//! for NativeCheckedModel, a proof, durable storage or renewed resource lease.
use super::{OptimizationError, OBJECTIVE_STREAM_CAP};
use lc631_core::context::{ContextRevision, SourceRevision};
use lc631_solvers::{
    runner::{QueryDigest, QuerySourceIdentity, SolverExecutionReport},
    smt::native_model::NativeCheckedModel,
    typed_query::{BoolSymbolId, IntSymbolId, SmtStatusQuery},
};

pub(super) struct ModelSnapshot {
    pub(super) stdout: Vec<u8>,
    pub(super) query: Vec<u8>,
    pub(super) report: Option<SolverExecutionReport>,
    pub(super) integers: Vec<(IntSymbolId, i128)>,
    pub(super) booleans: Vec<(BoolSymbolId, bool)>,
    context: Option<ContextRevision>,
    expected_query: Option<QueryDigest>,
    pub(super) assertions_checked: usize,
}
fn reserved<T>(count: usize) -> Result<Vec<T>, OptimizationError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| OptimizationError::AllocationFailed)?;
    Ok(values)
}
impl ModelSnapshot {
    /// Called before either additional native process may execute. The enclosing
    /// receipt's RAM/output claim must already be held for all backing buffers.
    pub(super) fn reserved(query: &SmtStatusQuery) -> Result<Self, OptimizationError> {
        Ok(Self {
            stdout: reserved(OBJECTIVE_STREAM_CAP)?,
            query: reserved(OBJECTIVE_STREAM_CAP)?,
            report: None,
            integers: reserved(query.integer_symbols().len())?,
            booleans: reserved(query.boolean_symbols().len())?,
            context: None,
            expected_query: None,
            assertions_checked: 0,
        })
    }
    /// Prepare canonical model-query bytes BEFORE the corresponding additional
    /// native model call. Afterward its real report must bind this exact query.
    pub(super) fn prepare(
        &mut self,
        query: &SmtStatusQuery,
        source: SourceRevision,
    ) -> Result<(), OptimizationError> {
        let prepared = query
            .emit_model(QuerySourceIdentity::from_bytes(*source.digest().as_bytes()))
            .map_err(OptimizationError::Query)?;
        let actual = prepared.solver_query().map_err(OptimizationError::Query)?;
        if prepared.payload().len() > self.query.capacity() {
            return Err(OptimizationError::AllocationFailed);
        }
        self.query.clear();
        self.query.extend_from_slice(prepared.payload());
        self.expected_query = Some(actual.digest());
        Ok(())
    }
    /// Copies only bounded actual observations and validated assignments. No
    /// allocation is needed here after a process has executed; buffers were
    /// reserved under the receipt claim before dispatch.
    pub(super) fn capture(
        &mut self,
        native: &NativeCheckedModel<'_, '_>,
    ) -> Result<(), OptimizationError> {
        if native.observation().stdout().len() > self.stdout.capacity()
            || native.model().query().integer_symbols().len() > self.integers.capacity()
            || native.model().query().boolean_symbols().len() > self.booleans.capacity()
        {
            return Err(OptimizationError::SourceBinding);
        }
        self.stdout.clear();
        self.stdout.extend_from_slice(native.observation().stdout());
        self.report = Some(native.observation().report());
        self.context = Some(native.context());
        if self.expected_query != Some(native.observation().query()) {
            return Err(OptimizationError::SourceBinding);
        }
        self.integers.clear();
        self.booleans.clear();
        for &id in native.model().query().integer_symbols() {
            self.integers.push((
                id,
                native
                    .model()
                    .integer(id)
                    .ok_or(OptimizationError::SourceBinding)?,
            ));
        }
        for &id in native.model().query().boolean_symbols() {
            self.booleans.push((
                id,
                native
                    .model()
                    .boolean(id)
                    .ok_or(OptimizationError::SourceBinding)?,
            ));
        }
        self.assertions_checked = native.model().assertions_checked();
        Ok(())
    }
}
