//! Copyable requests cannot construct private actual ownership.
use super::*;
#[derive(Clone, Copy)]
pub struct ArchiveEntry<'a> {
    pub source: &'a SourceStore,
    pub query: &'a SmtStatusQuery,
    pub atoms: &'a [n::AtomMap],
    pub kernel: f::Kernel,
}
#[derive(Clone, Copy)]
pub enum ArchiveInput<'a> {
    Read(&'a [ArchiveEntry<'a>]),
    Unread,
    Unavailable,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchiveStatus {
    Complete,
    Empty,
    Unread,
    Unavailable,
    Unsupported,
    NoComparable,
    Partial,
}
#[derive(Clone, Copy)]
pub struct Limits {
    pub max_features: usize,
    pub max_entries: usize,
    pub max_comparisons: usize,
    pub max_archive_dispatches: usize,
    pub max_assessments: usize,
    pub max_current_checks: usize,
    pub max_feature_bytes: usize,
    pub max_payload_bytes: usize,
    pub max_weight_bits: u32,
    pub max_intermediate_bits: u32,
    pub plan_ram: u64,
    pub assessment_ram: u64,
    pub scratch_ram: u64,
    pub output_bytes: u64,
}
#[derive(Clone, Copy)]
pub struct Registration<'a> {
    pub constructions: &'a [n::ConstructionFeature<'a>],
    pub archive: ArchiveInput<'a>,
    pub corpus_revision: Digest32,
    pub definition_revision: Digest32,
    pub oracle_revision: Digest32,
    pub policy_revision: Digest32,
    pub limits: Limits,
}
#[derive(Clone, Copy)]
pub struct CaseRef<'s, 'a, 'v> {
    pub owner: &'s f::OwnedFrameworkCase<'s, 'a, 'v>,
    pub expected: f::ExpectedInputs<'s, 'a, 'v>,
}
#[derive(Clone, Copy)]
pub struct ExpectedPair<'s, 'a, 'v> {
    pub root: &'s Root<'a, 'v>,
    pub registration: &'a Registration<'a>,
    pub source: CaseRef<'s, 'a, 'v>,
    pub target: CaseRef<'s, 'a, 'v>,
    pub archive: ArchiveInput<'a>,
    pub corpus_revision: Digest32,
}
#[derive(Clone, Copy, Debug)]
pub struct Counts {
    pub archive_entries_seen: usize,
    pub archive_comparisons: usize,
    pub native_archive_dispatches: usize,
    pub native_archive_store_entries: usize,
}
#[derive(Debug)]
pub enum Error {
    AlreadyGenerated,
    DuplicateFeature,
    InvalidWeight,
    NumericBudget,
    PayloadBudget,
    EmptyConstruction,
    UnsupportedConstruction,
    UniverseMismatch,
    InvalidMap,
    InvalidLimits,
    ForeignRoot,
    ForeignCase,
    ForeignSource,
    ExpectedInputMismatch,
    ForeignReservation,
    AssessmentBudget,
    CurrentBudget,
    WorkBudget,
    RamBudget,
    OutputBudget,
    Framework(f::Error),
    Resource(ResourceError),
    Arithmetic(MeasurementError),
    /// Exact additive arithmetic cause; never collapse invalid input/division to a cap.
    Numeric(numeric::NumericError),
}
