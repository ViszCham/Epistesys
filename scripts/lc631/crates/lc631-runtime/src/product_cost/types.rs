use lc631_substrate::resource::ResourceError;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticGoal {
    LocalRoles,
    ExactNativeIdentity,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionState {
    Completed,
    FuelExhausted,
    Failed,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReconstructionState {
    Matched,
    Mismatched,
    Unobserved,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompressionEligibility {
    Pending,
    Ineligible,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CostGap {
    NativeCanonicalizationCostClosurePending,
    NeedsProfileDecision,
    FullCostOwnerMissing,
    ApplicabilityUnknown,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvidenceGap {
    NativeGoalPinMissing,
    HardInvariantPinMissing,
    EvidencePinMissing,
    ApplicabilityUnknown,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputBasis {
    Given,
    ActualOwnerObservedWithGivenMeanings,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndependenceStatus {
    SharedDependency,
    Unassessed,
}
#[derive(Debug)]
pub enum CostError {
    InvalidProgram,
    MissingDependency,
    ExternalLookupForbidden,
    UnsupportedProfile,
    ProfileMismatch,
    ForeignOwner,
    StaleBaseline,
    CorrespondenceMismatch,
    InvalidCanonicalEncoding,
    ResourceExhausted,
    FuelExhausted,
    Resource(ResourceError),
    NeedsEvidence(CostGap),
}
impl From<ResourceError> for CostError {
    fn from(e: ResourceError) -> Self {
        Self::Resource(e)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CostLimits {
    pub semantic_bytes: u64,
    pub bundle_bytes: u64,
    pub total_nodes: u64,
    pub instructions: u64,
    pub local_entries: u64,
    pub value_cells: u64,
    pub fuel_per_direction: u64,
    pub iteration_bound: u64,
    pub depth: u64,
    pub additional_ram_bytes: u64,
}
impl Default for CostLimits {
    fn default() -> Self {
        Self {
            semantic_bytes: 65536,
            bundle_bytes: 262144,
            total_nodes: 16384,
            instructions: 1024,
            local_entries: 1024,
            value_cells: 4096,
            fuel_per_direction: 1048576,
            iteration_bound: 1024,
            depth: 64,
            additional_ram_bytes: 33554432,
        }
    }
}
impl CostLimits {
    pub(super) fn checked(self) -> Result<Self, CostError> {
        let d = Self::default();
        let a = [
            self.semantic_bytes,
            self.bundle_bytes,
            self.total_nodes,
            self.instructions,
            self.local_entries,
            self.value_cells,
            self.fuel_per_direction,
            self.iteration_bound,
            self.depth,
            self.additional_ram_bytes,
        ];
        let b = [
            d.semantic_bytes,
            d.bundle_bytes,
            d.total_nodes,
            d.instructions,
            d.local_entries,
            d.value_cells,
            d.fuel_per_direction,
            d.iteration_bound,
            d.depth,
            d.additional_ram_bytes,
        ];
        if a.iter().zip(b).any(|(a, b)| *a == 0 || *a > b) {
            return Err(CostError::UnsupportedProfile);
        }
        Ok(self)
    }
}
#[derive(Clone, Copy)]
pub struct ProfileSpec<'p> {
    pub goal: SemanticGoal,
    pub limits: CostLimits,
    pub runtime: &'p [u8],
}
impl<'p> ProfileSpec<'p> {
    pub fn copy_pack_tree_v1(runtime: &'p [u8]) -> Self {
        Self {
            goal: SemanticGoal::LocalRoles,
            limits: CostLimits::default(),
            runtime,
        }
    }
}
#[derive(Clone, Copy)]
pub struct InlinePackage<'p> {
    pub adapter: &'p [u8],
    pub projection: &'p [u8],
    pub inverse: &'p [u8],
    pub helpers: &'p [u8],
    pub runtime: &'p [u8],
    pub dictionary: &'p [u8],
}
pub struct OwnedInlinePackage {
    pub(super) adapter: Vec<u8>,
    pub(super) projection: Vec<u8>,
    pub(super) inverse: Vec<u8>,
    pub(super) helpers: Vec<u8>,
    pub(super) runtime: Vec<u8>,
    pub(super) dictionary: Vec<u8>,
}
impl OwnedInlinePackage {
    pub fn as_inline(&self) -> InlinePackage<'_> {
        InlinePackage {
            adapter: &self.adapter,
            projection: &self.projection,
            inverse: &self.inverse,
            helpers: &self.helpers,
            runtime: &self.runtime,
            dictionary: &self.dictionary,
        }
    }
    pub fn dictionary(&self) -> &[u8] {
        &self.dictionary
    }
}
pub struct ReportedFieldInput<'p> {
    pub fields: &'p [u8],
    pub declarations: &'p [u8],
}
#[derive(Clone, Debug, Default)]
pub struct ExecutionStats {
    pub adapter_steps: u64,
    pub projection_steps: u64,
    pub inverse_steps: u64,
    pub runtime_steps: u64,
    pub native_field_reads: u64,
    pub meaning_declarations: u64,
    pub matched_dictionary_declarations: u64,
    pub dictionary_reads: u64,
    pub copied_bytes: u64,
    pub codec_fuel: u64,
    pub adapter_fuel: u64,
    pub projection_fuel: u64,
    pub inverse_fuel: u64,
    pub fuel_used: u64,
    pub external_lookups: u64,
}
impl ExecutionStats {
    pub(super) fn total(&mut self) -> Result<(), CostError> {
        self.fuel_used = self
            .codec_fuel
            .checked_add(self.adapter_fuel)
            .and_then(|x| x.checked_add(self.projection_fuel))
            .and_then(|x| x.checked_add(self.inverse_fuel))
            .ok_or(CostError::InvalidProgram)?;
        Ok(())
    }
}
