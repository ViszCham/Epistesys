//! 日本語: 固定SOFT adversarial MODEL catalogの実反例検査。世界の正解・権限ではない。
//! English: Actual counterexample checks over a fixed SOFT adversarial MODEL catalog.
//! Five fields: fixed positive-weight K1 failure bounds[0,1], dimensionless;
//! actual finite search then separate existing model recheck; inhabited fixed Given
//! Boolean attack domains, no hard claims; independent handgold/mutants; minimize
//! CounterexampleCost only. Shared IR/source/reference semantics remain unassessed.
#![forbid(unsafe_code)]
mod ast;
mod evaluation;
mod selection;
use crate::{formal_boolean as formal, joint_selection::*};
pub use evaluation::{execute, OwnedCounterRisk, TrialGap, TrialState};
use lc631_analysis::creativity_measurement::{
    ComponentMetric, ComponentReport, ComponentState, IndependenceStatus, InputBasis,
    MeasurementBinding, MeasurementGap, SemanticValidation,
};
use lc631_core::{
    context::ContextRevision,
    measurement_numeric::{self as numeric, NumericError, Rational, RationalInterval},
    CandidateId,
};
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, SmtStatusQuery};
use lc631_substrate::{
    resource::{
        ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
        ResourceReservationView,
    },
    Digest32,
};
pub use selection::{select_current, AdversarialRow, OwnedAdversarialSelection};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
pub const MAX_CASES: usize = 8;
pub const MAX_ATOMS: usize = 8;
pub const MAX_NODES: usize = 512;
pub const MAX_DEPTH: usize = 32;
pub const MAX_PAYLOAD: usize = 65536;
const RAM: u64 = 16 * 1024 * 1024;
const SCRATCH: u64 = 32 * 1024 * 1024;
const PRODUCER: &[u8] = b"finite-formal-v1.4/CR-RCOUNTER/fixed-soft-reference-catalog.v1";
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CasePurpose {
    SoftFiniteValidation,
    HardUniversal,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Quantification {
    AllAdmissibleAssignments,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttackLimits {
    pub registration_ast_visits: u64,
    pub max_ast_visits: u64,
    pub max_trials: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CatalogBinding {
    pub selection: SelectionPins,
    pub context: ContextRevision,
    pub suite: Digest32,
    pub attack_scope: Digest32,
    pub oracle: Digest32,
    pub definition: Digest32,
    pub probe_set: Digest32,
    pub dependency_group: Digest32,
    pub purpose: CasePurpose,
    pub quantification: Quantification,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateBinding {
    pub id: CandidateId,
    pub candidate_digest: Digest32,
    pub context: ContextRevision,
    pub target: Digest32,
    pub transform: Digest32,
}
#[derive(Clone, Copy)]
pub struct SemanticAtom<'a> {
    pub symbol: BoolSymbolId,
    pub meaning: &'a [u8],
}
#[derive(Clone, Copy)]
pub struct AttackCase<'a> {
    pub key: &'a [u8],
    pub atoms: &'a [SemanticAtom<'a>],
    pub theory: &'a SmtStatusQuery,
    pub reference: &'a BoolExpr,
    pub weight: &'a Rational,
}
#[derive(Clone, Copy)]
pub struct Response<'a> {
    pub case: &'a [u8],
    pub term: &'a BoolExpr,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Resource(ResourceError),
    Numeric(NumericError),
    Formal(formal::Failure),
    Query(lc631_solvers::typed_query::QueryEmitError),
    Model(lc631_solvers::smt::ModelValidationError),
    Parse(lc631_solvers::protocol::sexpr::SExprError),
    Selection(SelectionError),
    EmptySuite,
    UnsupportedPurpose,
    InvalidLimits,
    InvalidWeight,
    DuplicateCase,
    InvalidCase,
    InvalidMap,
    UnsupportedFragment,
    AstLimit,
    AllocationFailed,
    Prerequisite(formal::NoComparable),
    PrerequisitePending,
    BindingMismatch,
    DefinitionMismatch,
    ForeignReservation,
    InvalidRows,
    Witness,
}
pub fn definition_revision() -> Digest32 {
    digest(PRODUCER)
}
fn digest(bytes: &[u8]) -> Digest32 {
    Digest32::from_bytes(Sha256::digest(bytes).into())
}
fn claim<'v>(
    view: &'v ResourceReservationView<'_>,
    ram: u64,
    output: u64,
) -> Result<ResourceClaim<'v>, Error> {
    let ram = ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(ram).map_err(Error::Resource)?,
    );
    if output == 0 {
        view.claim_resources(&[ram]).map_err(Error::Resource)
    } else {
        let output = ResourceRequest::new(
            ResourceKind::Output,
            ResourceAmount::checked(output).map_err(Error::Resource)?,
        );
        view.claim_resources(&[ram, output])
            .map_err(Error::Resource)
    }
}
fn source(b: CatalogBinding, view: &ResourceReservationView<'_>) -> Result<(), Error> {
    view.check().map_err(Error::Resource)?;
    if b.selection.source.digest() != view.source_revision() {
        return Err(Error::Resource(ResourceError::SourceMismatch));
    }
    if b.context.source() != b.selection.source || b.probe_set != b.selection.scope.probes() {
        return Err(Error::BindingMismatch);
    }
    if b.definition != definition_revision() {
        return Err(Error::DefinitionMismatch);
    }
    Ok(())
}
struct Atom {
    symbol: BoolSymbolId,
    meaning: Vec<u8>,
}
struct Case {
    key: Vec<u8>,
    atoms: Vec<Atom>,
    theory: SmtStatusQuery,
    reference: BoolExpr,
    weight: Rational,
    map: Vec<(BoolSymbolId, BoolSymbolId)>,
    registration_assignments: u32,
    registration_visits: u64,
}
/// Borrowed actual raw atom and its sorted-position normalized AST symbol.
pub(crate) struct ProductAtomView<'p> {
    pub(crate) raw_symbol: BoolSymbolId,
    pub(crate) normalized_symbol: BoolSymbolId,
    pub(crate) meaning: &'p [u8],
}
pub(crate) struct ProductAtomIter<'p> {
    inner: std::iter::Enumerate<std::slice::Iter<'p, Atom>>,
}
impl<'p> Iterator for ProductAtomIter<'p> {
    type Item = ProductAtomView<'p>;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(index, atom)| ProductAtomView {
            raw_symbol: atom.symbol,
            // Actual catalog constructors cap the sorted atoms at MAX_ATOMS=8.
            normalized_symbol: BoolSymbolId::new(index as u32),
            meaning: &atom.meaning,
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) { self.inner.size_hint() }
}
impl ExactSizeIterator for ProductAtomIter<'_> {
    fn len(&self) -> usize { self.inner.len() }
}
pub(crate) struct ProductCaseView<'p> {
    pub(crate) key: &'p [u8],
    pub(crate) atoms: ProductAtomIter<'p>,
    pub(crate) theory: &'p SmtStatusQuery,
    pub(crate) reference: &'p BoolExpr,
    pub(crate) response: &'p BoolExpr,
    pub(crate) weight: &'p Rational,
    pub(crate) normalized_identity_map: &'p [(BoolSymbolId, BoolSymbolId)],
}
pub(crate) struct ProductCaseIter<'p> {
    cases: std::slice::Iter<'p, Case>,
    terms: std::slice::Iter<'p, BoolExpr>,
}
impl<'p> Iterator for ProductCaseIter<'p> {
    type Item = ProductCaseView<'p>;
    fn next(&mut self) -> Option<Self::Item> {
        // Both private streams advance together. Admission checks equal lengths.
        match (self.cases.next(), self.terms.next()) {
            (Some(case), Some(response)) => Some(ProductCaseView {
                key: &case.key,
                atoms: ProductAtomIter { inner: case.atoms.iter().enumerate() },
                theory: &case.theory,
                reference: &case.reference,
                response,
                weight: &case.weight,
                normalized_identity_map: &case.map,
            }),
            _ => None,
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) { self.cases.size_hint() }
}
impl ExactSizeIterator for ProductCaseIter<'_> {
    fn len(&self) -> usize { self.cases.len() }
}
/// Actual immutable admissible attack domain/reference; candidate terms cannot
/// constrain Γ. Registration checks inhabitedness BEFORE binary uncertainty.
pub struct AdversarialCatalog<'v> {
    binding: CatalogBinding,
    cases: Vec<Case>,
    limits: AttackLimits,
    weight: Rational,
    digest: Digest32,
    retained: ResourceClaim<'v>,
}
impl AdversarialCatalog<'_> {
    pub fn fixed_weight(&self) -> &Rational {
        &self.weight
    }
    pub fn historical_digest(&self) -> Digest32 {
        self.digest
    }
    fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        source(self.binding, view)?;
        self.retained.check().map_err(Error::Resource)?;
        if !self.retained.same_reservation(view) {
            return Err(Error::ForeignReservation);
        }
        Ok(())
    }
}
pub fn register<'v>(
    binding: CatalogBinding,
    inputs: &[AttackCase<'_>],
    limits: AttackLimits,
    view: &'v ResourceReservationView<'_>,
) -> Result<AdversarialCatalog<'v>, Error> {
    source(binding, view)?;
    if binding.purpose != CasePurpose::SoftFiniteValidation {
        return Err(Error::UnsupportedPurpose);
    }
    if inputs.is_empty() {
        return Err(Error::EmptySuite);
    }
    if inputs.len() > MAX_CASES
        || limits.max_trials > MAX_CASES
        || limits.registration_ast_visits == 0
        || limits.registration_ast_visits > 1_000_000
        || limits.max_ast_visits > 1_000_000
    {
        return Err(Error::InvalidLimits);
    }
    let scratch = claim(view, SCRATCH, 0)?;
    let retained = claim(view, RAM, 2 * 1024 * 1024)?;
    // Admit borrowed payload lengths BEFORE any content equality/lookup/hash.
    let mut admitted_bytes = 0usize;
    for c in inputs {
        if c.atoms.len() > MAX_ATOMS {
            return Err(Error::InvalidMap);
        }
        admitted_bytes = admitted_bytes
            .checked_add(c.key.len())
            .ok_or(Error::InvalidCase)?;
        if c.key.is_empty() || admitted_bytes > MAX_PAYLOAD {
            return Err(Error::InvalidCase);
        }
        for atom in c.atoms {
            admitted_bytes = admitted_bytes
                .checked_add(atom.meaning.len())
                .ok_or(Error::InvalidCase)?;
            if atom.meaning.is_empty() || admitted_bytes > MAX_PAYLOAD {
                return Err(Error::InvalidCase);
            }
        }
        numeric::check(c.weight).map_err(Error::Numeric)?;
    }
    let zero = Rational::from_integer(0.into());
    let mut bytes = 0usize;
    for (i, c) in inputs.iter().enumerate() {
        if numeric::checked_compare(c.weight, &zero).map_err(Error::Numeric)? != Ordering::Greater {
            return Err(Error::InvalidWeight);
        }
        if inputs[..i].iter().any(|d| d.key == c.key) {
            return Err(Error::DuplicateCase);
        }
        bytes = bytes.checked_add(c.key.len()).ok_or(Error::InvalidCase)?;
        if c.key.is_empty() {
            return Err(Error::InvalidCase);
        }
        ast::preflight_case(c, &mut bytes)?;
    }
    if bytes > MAX_PAYLOAD {
        return Err(Error::InvalidCase);
    }
    let mut cases = Vec::new();
    cases
        .try_reserve_exact(inputs.len())
        .map_err(|_| Error::AllocationFailed)?;
    for c in inputs {
        let mut owned = ast::own_case(c)?;
        let (assignments, visits) = {
            let yes = BoolExpr::Constant(true);
            let mut request = ast::request(binding, &owned, &yes, &yes);
            request.binding.probe_map =
                formal::probe_map_digest(&request, 1_000_000).map_err(Error::Formal)?;
            match formal::evaluate(&request, limits.registration_ast_visits)
                .map_err(Error::Formal)?
            {
                formal::Evaluation::Computed(report) => (report.assignments(), report.ast_visits()),
                formal::Evaluation::NoComparable { reason, .. } => {
                    return Err(Error::Prerequisite(reason))
                }
                formal::Evaluation::Unknown { .. } => return Err(Error::PrerequisitePending),
            }
        };
        owned.registration_assignments = assignments;
        owned.registration_visits = visits;
        cases.push(owned);
    }
    cases.sort_by(|a, b| a.key.cmp(&b.key));
    let mut weight = zero;
    for c in &cases {
        weight = numeric::checked_add(&weight, &c.weight).map_err(Error::Numeric)?;
    }
    let mut h = Sha256::new();
    h.update(PRODUCER);
    for d in [
        binding.selection.source.digest(),
        binding.context.interpretation_digest(),
        binding.selection.scope.comparison(),
        binding.selection.scope.goal(),
        binding.selection.scope.probes(),
        binding.selection.scope.semantics(),
        binding.selection.baseline,
        binding.selection.policy,
        binding.selection.hard_invariants,
        binding.selection.evidence_revision,
        binding.suite,
        binding.attack_scope,
        binding.oracle,
        binding.definition,
        binding.probe_set,
        binding.dependency_group,
    ] {
        h.update(d.as_bytes());
    }
    match binding.selection.goal_profile {
        Some(d) => {
            h.update([1]);
            h.update(d.as_bytes());
        }
        None => h.update([0]),
    };
    h.update(b"SoftFiniteValidation/AllAdmissibleAssignments/reference-agreement/soundness.v1");
    for n in [
        limits.registration_ast_visits,
        limits.max_ast_visits,
        limits.max_trials as u64,
        MAX_ATOMS as u64,
        MAX_NODES as u64,
        MAX_DEPTH as u64,
    ] {
        h.update(n.to_le_bytes());
    }
    for c in &cases {
        field(&mut h, &c.key);
        for a in &c.atoms {
            field(&mut h, &a.meaning);
        }
        hash_rational(&mut h, &c.weight);
        h.update((c.theory.assertions().len() as u64).to_le_bytes());
        for expr in c.theory.assertions() {
            ast::hash_expr(&mut h, expr);
        }
        ast::hash_expr(&mut h, &c.reference);
        h.update(c.registration_assignments.to_le_bytes());
        h.update(c.registration_visits.to_le_bytes());
    }
    hash_rational(&mut h, &weight);
    let out = AdversarialCatalog {
        binding,
        cases,
        limits,
        weight,
        digest: Digest32::from_bytes(h.finalize().into()),
        retained,
    };
    scratch.check().map_err(Error::Resource)?;
    out.check(view)?;
    Ok(out)
}
pub struct FrozenResponses<'a, 'v> {
    catalog: &'a AdversarialCatalog<'v>,
    binding: CandidateBinding,
    terms: Vec<BoolExpr>,
    digest: Digest32,
    retained: ResourceClaim<'v>,
}
impl FrozenResponses<'_, '_> {
    /// Data-only, callback-scoped view of the actual immutable catalog/terms.
    pub(crate) fn inspect_product_cases_current<R>(
        &self,
        view: &ResourceReservationView<'_>,
        consume: impl for<'p> FnOnce(
            CatalogBinding, CandidateBinding, AttackLimits,
            &'p Rational, ProductCaseIter<'p>,
        ) -> R,
    ) -> Result<R, Error> {
        self.check(view)?;
        if self.catalog.cases.len() != self.terms.len() {
            return Err(Error::BindingMismatch);
        }
        let result = consume(
            self.catalog.binding, self.binding, self.catalog.limits,
            &self.catalog.weight,
            ProductCaseIter {
                cases: self.catalog.cases.iter(),
                terms: self.terms.iter(),
            },
        );
        self.check(view)?;
        Ok(result)
    }
    pub fn historical_digest(&self) -> Digest32 {
        self.digest
    }
    fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        self.catalog.check(view)?;
        if self.binding.context != self.catalog.binding.context {
            return Err(Error::BindingMismatch);
        }
        self.retained.check().map_err(Error::Resource)?;
        if !self.retained.same_reservation(view) {
            return Err(Error::ForeignReservation);
        }
        Ok(())
    }
}
/// Terms only: no candidate-side theory, reported status, model or scope override.
pub fn bind_candidate<'a, 'v>(
    catalog: &'a AdversarialCatalog<'v>,
    binding: CandidateBinding,
    responses: &[Response<'_>],
    view: &'v ResourceReservationView<'_>,
) -> Result<FrozenResponses<'a, 'v>, Error> {
    catalog.check(view)?;
    if binding.context != catalog.binding.context || responses.len() != catalog.cases.len() {
        return Err(Error::BindingMismatch);
    }
    let scratch = claim(view, SCRATCH, 0)?;
    let retained = claim(view, RAM, 2 * 1024 * 1024)?;
    let mut response_bytes = 0usize;
    for r in responses {
        response_bytes = response_bytes
            .checked_add(r.case.len())
            .ok_or(Error::InvalidCase)?;
        if r.case.is_empty() || response_bytes > MAX_PAYLOAD {
            return Err(Error::InvalidCase);
        }
    }
    for (i, r) in responses.iter().enumerate() {
        if responses[..i].iter().any(|s| s.case == r.case) {
            return Err(Error::DuplicateCase);
        }
        let case = catalog
            .cases
            .iter()
            .find(|c| c.key == r.case)
            .ok_or(Error::InvalidCase)?;
        ast::preflight_response(r.term, &case.atoms)?;
    }
    let mut terms = Vec::new();
    terms
        .try_reserve_exact(responses.len())
        .map_err(|_| Error::AllocationFailed)?;
    let mut h = Sha256::new();
    h.update(PRODUCER);
    h.update(catalog.digest.as_bytes());
    for d in [
        binding.candidate_digest,
        binding.context.interpretation_digest(),
        binding.target,
        binding.transform,
    ] {
        h.update(d.as_bytes());
    }
    for c in &catalog.cases {
        let r = responses
            .iter()
            .find(|r| r.case == c.key)
            .ok_or(Error::InvalidCase)?;
        let term = ast::normalize(r.term, &c.atoms)?;
        ast::hash_expr(&mut h, &term);
        terms.push(term);
    }
    let out = FrozenResponses {
        catalog,
        binding,
        terms,
        digest: Digest32::from_bytes(h.finalize().into()),
        retained,
    };
    scratch.check().map_err(Error::Resource)?;
    out.check(view)?;
    Ok(out)
}
fn field(h: &mut Sha256, b: &[u8]) {
    h.update((b.len() as u64).to_le_bytes());
    h.update(b);
}
fn hash_rational(h: &mut Sha256, r: &Rational) {
    field(h, &r.numer().to_signed_bytes_le());
    field(h, &r.denom().to_signed_bytes_le());
}
