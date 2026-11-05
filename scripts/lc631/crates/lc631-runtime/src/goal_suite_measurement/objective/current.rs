//! Bounded closed root checks and exact legacy invocation-v1 binding.
use super::*;
use lc631_solvers::{
    protocol::SolverProtocol,
    registry::{BackendId, LogicProfile},
    runner::{InvocationDigest, QuerySourceIdentity},
    typed_query::{BoolExpr, IntExpr},
};
use std::path::Path;

fn shape(query: &SmtStatusQuery, symbol: IntSymbolId) -> Result<(), Error> {
    if !query.boolean_symbols().is_empty()
        || query.integer_symbols() != [symbol]
        || query.assertions().len() != 3
    {
        return Err(Error::Unsupported);
    }
    Ok(())
}
fn lower(expr: &BoolExpr, symbol: IntSymbolId) -> bool {
    matches!(expr, BoolExpr::LessEqual(IntExpr::Constant(0),IntExpr::Variable(x)) if *x==symbol)
}
fn upper(expr: &BoolExpr, symbol: IntSymbolId, limit: i64) -> bool {
    matches!(expr, BoolExpr::LessEqual(IntExpr::Variable(x),IntExpr::Constant(v)) if *x==symbol && *v==limit)
}
pub(super) fn root_query(query: &SmtStatusQuery, symbol: IntSymbolId) -> Result<(), Error> {
    shape(query, symbol)?;
    let a = query.assertions();
    if !lower(&a[0], symbol) || !upper(&a[1], symbol, 3) || !upper(&a[2], symbol, 2) {
        return Err(Error::HardInvariantFailed);
    }
    Ok(())
}
pub(super) fn registration(registry: &SolverRegistry) -> Result<&SolverRegistration, Error> {
    let [entry] = registry.entries() else {
        return Err(Error::Unsupported);
    };
    if entry.id() != BackendId::Z3
        || entry.profile() != LogicProfile::SmtBooleanQfLia
        || entry.protocol() != SolverProtocol::SmtLibPayloadWireV2
    {
        return Err(Error::Unsupported);
    }
    if entry.executable().len() > 4096
        || entry.version().len() > 128
        || entry.fixed_arguments().len() > 16
    {
        return Err(Error::InputLimit);
    }
    let mut bytes = entry.executable().len();
    for arg in entry.fixed_arguments() {
        bytes = bytes.checked_add(arg.len()).ok_or(Error::InputLimit)?;
        if bytes > 4096 {
            return Err(Error::InputLimit);
        }
    }
    let c = entry.caps();
    if c.max_argv_count != 16
        || c.max_argv_bytes != 4096
        || c.max_stdin_bytes != 4096
        || c.max_stdout_bytes != 65536
        || c.max_stderr_bytes != 16384
        || c.max_timeout_ms != 5000
        || c.max_ram_bytes != 64 * 1024 * 1024
        || c.max_disk_bytes != 0
        || c.max_cleanup_bytes != 65536
        || c.max_child_count != 1
    {
        return Err(Error::InvalidLimits);
    }
    Ok(entry)
}
/// Exact mirror of runner::compute_invocation_digest domain v1, with the CLOSED
/// execution budget below. This is encoding equality, not authentication. No
/// private/report success is reconstructed; a schema change must fail/revise.
pub(super) fn invocation(entry: &SolverRegistration) -> InvocationDigest {
    let mut h = Sha256::new();
    fn field(h: &mut Sha256, value: &[u8]) {
        h.update((value.len() as u64).to_le_bytes());
        h.update(value);
    }
    h.update(b"epistesys.solver-invocation.v1\0");
    h.update([1, 1, 4]);
    field(&mut h, entry.executable().as_bytes());
    field(&mut h, entry.version().as_bytes());
    h.update(entry.binary_digest());
    h.update((entry.fixed_arguments().len() as u64).to_le_bytes());
    for arg in entry.fixed_arguments() {
        field(&mut h, arg.as_bytes());
    }
    let c = entry.caps();
    for n in [
        c.max_argv_count as u64,
        c.max_argv_bytes as u64,
        c.max_stdin_bytes as u64,
        c.max_stdout_bytes as u64,
        c.max_stderr_bytes as u64,
        c.max_timeout_ms,
        c.max_ram_bytes,
        c.max_disk_bytes,
        c.max_cleanup_bytes,
    ] {
        h.update(n.to_le_bytes());
    }
    h.update(c.max_child_count.to_le_bytes());
    // Closed actual native fixture budget: same ordering/types as runner v1.
    for n in [
        5000u64,
        16,
        4096,
        4096,
        65536,
        16384,
        64 * 1024 * 1024,
        0,
        65536,
    ] {
        h.update(n.to_le_bytes());
    }
    h.update(1u32.to_le_bytes());
    InvocationDigest::from_bytes(h.finalize().into())
}
#[derive(Clone, Copy)]
pub(super) struct Fact {
    pub value: i64,
    pub binding: Digest32,
    pub target: Digest32,
}
fn model(
    root: &ObjectiveRoot<'_, '_>,
    owner: &NativeCheckedModel<'_, '_>,
    view: &ResourceReservationView<'_>,
) -> Result<Fact, Error> {
    owner.check_current(view)?;
    if owner.context() != root.context {
        return Err(Error::BindingMismatch);
    }
    let observed = owner.observation();
    if observed.stdout().len() > 65536 {
        return Err(Error::InputLimit);
    }
    if observed.source() != QuerySourceIdentity::from_bytes(*view.source_revision().as_bytes())
        || observed.invocation() != Some(root.invocation)
        || observed.payload().is_none()
    {
        return Err(Error::BindingMismatch);
    }
    let report = observed.report();
    let actual: [u8; 32] = Sha256::digest(observed.stdout()).into();
    if report.stdout_digest() != Some(actual)
        || !report.stdout_fully_observed()
        || !report.stderr_fully_observed()
        || report.stdout_truncated()
        || report.stderr_truncated()
    {
        return Err(Error::BindingMismatch);
    }
    let checked = owner.model();
    let query = checked.query();
    shape(query, root.symbol)?;
    let a = query.assertions();
    if !lower(&a[0], root.symbol) || !upper(&a[1], root.symbol, 3) {
        return Err(Error::Unsupported);
    }
    let BoolExpr::Equal(IntExpr::Variable(symbol), IntExpr::Constant(expected)) = &a[2] else {
        return Err(Error::Unsupported);
    };
    if *symbol != root.symbol || checked.assertions_checked() != 3 {
        return Err(Error::BindingMismatch);
    }
    let value = checked.integer(root.symbol).ok_or(Error::NoScalar)?;
    if value != i128::from(*expected) {
        return Err(Error::BindingMismatch);
    }
    // Reevaluate the admitted immutable root's actual0<=x<=3 AND x<=2.
    // A model's own assignment query being SAT does not satisfy this hard root.
    if !(0..=2).contains(&value) {
        return Err(Error::HardInvariantFailed);
    }
    let value = i64::try_from(value).map_err(|_| Error::NoScalar)?;
    let binding = hash(&[
        b"actual-native-model-goal-fact.v1",
        observed.source().as_bytes(),
        observed.query().as_bytes(),
        root.invocation.as_bytes(),
        &actual,
        &value.to_le_bytes(),
        root.identity.as_bytes(),
    ]);
    owner.check_current(view)?;
    Ok(Fact {
        value,
        binding,
        target: Digest32::from_bytes(actual),
    })
}
pub(super) fn check(
    root: &ObjectiveRoot<'_, '_>,
    pair: NativePair<'_, '_, '_>,
    view: &ResourceReservationView<'_>,
) -> Result<[Fact; 2], Error> {
    root.check(view)?;
    let a = model(root, pair.baseline, view)?;
    let b = model(root, pair.candidate, view)?;
    if a.value != 1 {
        return Err(Error::RoleOrOriginMismatch);
    }
    let entry = registration(root.registry)?;
    let actual = lc631_solvers::supervisor_adapter::hash_registered_executable(
        view,
        Path::new(entry.executable()),
    )
    .map_err(Error::NativeFingerprint)?;
    if actual != *entry.binary_digest() {
        return Err(Error::BindingMismatch);
    }
    pair.baseline.check_current(view)?;
    pair.candidate.check_current(view)?;
    root.check(view)?;
    Ok([a, b])
}
