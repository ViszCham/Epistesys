use super::super::{AdmissionError, GoalPolicy};
use lc631_core::context::{ContextScope, SourceRevision};
use lc631_substrate::{codec::CodecError, resource::ResourceError, Digest32};
use sha2::{Digest, Sha256};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplayGoal {
    NoGoal,
    SatisfyingModel,
    DeclaredSourceObjective,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplayProfile {
    pub(super) goal: ReplayGoal,
    pub(super) evaluation: bool,
    pub(super) objective: bool,
}
impl ReplayProfile {
    pub fn checked(
        goal: ReplayGoal,
        evaluation: bool,
        objective: bool,
    ) -> Result<Self, ReplayError> {
        if objective && goal != ReplayGoal::DeclaredSourceObjective {
            return Err(ReplayError::Profile);
        }
        Ok(Self {
            goal,
            evaluation,
            objective,
        })
    }
    pub(super) fn tags(self) -> [u8; 3] {
        [
            match self.goal {
                ReplayGoal::NoGoal => 0,
                ReplayGoal::SatisfyingModel => 1,
                ReplayGoal::DeclaredSourceObjective => 2,
            },
            u8::from(self.evaluation),
            u8::from(self.objective),
        ]
    }
    pub(super) fn from_tags(tags: &[u8]) -> Result<Self, ReplayError> {
        if tags.len() != 3 || tags[1] > 1 || tags[2] > 1 {
            return Err(ReplayError::Profile);
        }
        let goal = match tags[0] {
            0 => ReplayGoal::NoGoal,
            1 => ReplayGoal::SatisfyingModel,
            2 => ReplayGoal::DeclaredSourceObjective,
            _ => return Err(ReplayError::Profile),
        };
        Self::checked(goal, tags[1] == 1, tags[2] == 1)
    }
    pub fn digest(self) -> Digest32 {
        let mut hash = Sha256::new();
        hash.update(b"Epistesys/TLCX/CanonicalReplay/cli-formal-profile-v1/finite-boolean-v1/objective-3-15000\0");
        hash.update(self.tags());
        Digest32::from_bytes(hash.finalize().into())
    }
    pub fn key(self, source: &str) -> ArtifactKey {
        ArtifactKey {
            source: SourceRevision::of(source).digest(),
            profile: self.digest(),
        }
    }
    pub(super) fn base_scope(self) -> ContextScope {
        let goal = match self.goal {
            ReplayGoal::NoGoal => "epistesys.cli.formal.v1\0no-goal",
            ReplayGoal::SatisfyingModel => "epistesys.cli.formal.v1\0declared-satisfying-model",
            ReplayGoal::DeclaredSourceObjective => {
                "epistesys.cli.formal.v1\0declared-source-objective"
            }
        };
        ContextScope::declared(
            SourceRevision::of("epistesys.cli.formal.v1\0no-comparison").digest(),
            SourceRevision::of(goal).digest(),
            SourceRevision::of("epistesys.cli.formal.v1\0no-probes").digest(),
            SourceRevision::of("epistesys.cli.formal.v1\0explicit-qf-lia").digest(),
        )
    }
    pub(super) fn goal_policy(self) -> GoalPolicy {
        let revision = self.base_scope().goal();
        match self.goal {
            ReplayGoal::NoGoal => GoalPolicy::NoGoal,
            ReplayGoal::SatisfyingModel => GoalPolicy::SatisfyingModel { revision },
            ReplayGoal::DeclaredSourceObjective => GoalPolicy::DeclaredSourceObjective { revision },
        }
    }
    pub(super) fn scope(self, source: &str) -> Result<ContextScope, ReplayError> {
        let base = self.base_scope();
        if self.evaluation {
            Ok(super::super::DeclaredEvaluation::finite_boolean(
                SourceRevision::of(source),
                base,
                self.goal_policy(),
            )?
            .scope())
        } else {
            Ok(base)
        }
    }
}

/// A source/profile expectation, not authentication, authorization or a path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactKey {
    pub(super) source: Digest32,
    pub(super) profile: Digest32,
}
impl ArtifactKey {
    pub fn parse(text: &str) -> Result<Self, ReplayError> {
        let body = text.strip_prefix("canonical:").ok_or(ReplayError::Key)?;
        let (source, profile) = body.split_once(':').ok_or(ReplayError::Key)?;
        Ok(Self {
            source: unhex(source)?,
            profile: unhex(profile)?,
        })
    }
    pub const fn source(self) -> Digest32 {
        self.source
    }
    pub const fn profile(self) -> Digest32 {
        self.profile
    }
}
impl fmt::Display for ArtifactKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("canonical:")?;
        for byte in self.source.as_bytes() {
            write!(f, "{byte:02x}")?;
        }
        f.write_str(":")?;
        for byte in self.profile.as_bytes() {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}
fn unhex(text: &str) -> Result<Digest32, ReplayError> {
    if text.len() != 64 {
        return Err(ReplayError::Key);
    }
    let mut out = [0; 32];
    let digit = |b| match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        _ => Err(ReplayError::Key),
    };
    for (slot, pair) in out
        .iter_mut()
        .zip(text.as_bytes().as_chunks::<2>().0.iter())
    {
        *slot = (digit(pair[0])? << 4) | digit(pair[1])?;
    }
    Ok(Digest32::from_bytes(out))
}

#[derive(Debug)]
pub enum ReplayError {
    Key,
    Profile,
    Shape,
    Limit,
    Binding,
    MissingNativeModel,
    MissingFreshRegistry,
    Allocation,
    Root,
    StorageRead,
    StorageOpenMayHaveApplied,
    StorageMayHaveApplied,
    Codec(CodecError),
    Resource(ResourceError),
    Admission(AdmissionError),
    PostWriteMayHaveApplied(ReplayPostWriteCause),
    PostOpenMayHaveApplied(ReplayPostWriteCause),
}
/// A late failure preserves its cause without allocating after a physical write.
#[derive(Debug)]
pub enum ReplayPostWriteCause {
    Resource(ResourceError),
    Admission(AdmissionError),
    Codec(CodecError),
    Invariant(ReplayFailureKind),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplayFailureKind {
    Key,
    Profile,
    Shape,
    Limit,
    Binding,
    MissingNativeModel,
    MissingFreshRegistry,
    Allocation,
    Root,
    StorageRead,
    StorageOpenMayHaveApplied,
    StorageMayHaveApplied,
}
impl ReplayError {
    pub(super) fn after_write(error: Self) -> Self {
        Self::PostWriteMayHaveApplied(Self::late_cause(error))
    }
    pub(super) fn after_open(error: Self) -> Self {
        Self::PostOpenMayHaveApplied(Self::late_cause(error))
    }
    fn late_cause(error: Self) -> ReplayPostWriteCause {
        match error {
            Self::Resource(error) => ReplayPostWriteCause::Resource(error),
            Self::Admission(error) => ReplayPostWriteCause::Admission(error),
            Self::Codec(error) => ReplayPostWriteCause::Codec(error),
            Self::PostWriteMayHaveApplied(cause) => cause,
            Self::PostOpenMayHaveApplied(cause) => cause,
            Self::Key => ReplayPostWriteCause::Invariant(ReplayFailureKind::Key),
            Self::Profile => ReplayPostWriteCause::Invariant(ReplayFailureKind::Profile),
            Self::Shape => ReplayPostWriteCause::Invariant(ReplayFailureKind::Shape),
            Self::Limit => ReplayPostWriteCause::Invariant(ReplayFailureKind::Limit),
            Self::Binding => ReplayPostWriteCause::Invariant(ReplayFailureKind::Binding),
            Self::MissingNativeModel => {
                ReplayPostWriteCause::Invariant(ReplayFailureKind::MissingNativeModel)
            }
            Self::MissingFreshRegistry => {
                ReplayPostWriteCause::Invariant(ReplayFailureKind::MissingFreshRegistry)
            }
            Self::Allocation => ReplayPostWriteCause::Invariant(ReplayFailureKind::Allocation),
            Self::Root => ReplayPostWriteCause::Invariant(ReplayFailureKind::Root),
            Self::StorageRead => ReplayPostWriteCause::Invariant(ReplayFailureKind::StorageRead),
            Self::StorageOpenMayHaveApplied => {
                ReplayPostWriteCause::Invariant(ReplayFailureKind::StorageOpenMayHaveApplied)
            }
            Self::StorageMayHaveApplied => {
                ReplayPostWriteCause::Invariant(ReplayFailureKind::StorageMayHaveApplied)
            }
        }
    }
}
impl From<ResourceError> for ReplayError {
    fn from(e: ResourceError) -> Self {
        Self::Resource(e)
    }
}
impl From<CodecError> for ReplayError {
    fn from(e: CodecError) -> Self {
        Self::Codec(e)
    }
}
impl From<AdmissionError> for ReplayError {
    fn from(e: AdmissionError) -> Self {
        Self::Admission(e)
    }
}

/// Raw historical bytes with structural bindings; never a current-model token.
pub struct HistoricalView<'a> {
    pub(super) source: &'a str,
    pub(super) text: &'a str,
    pub(super) profile: ReplayProfile,
    pub(super) context: Digest32,
    pub(super) scope: [Digest32; 4],
}
impl HistoricalView<'_> {
    pub fn source(&self) -> &str {
        self.source
    }
    pub fn historical_text(&self) -> &str {
        self.text
    }
    pub const fn profile(&self) -> ReplayProfile {
        self.profile
    }
    pub const fn current_model(&self) -> bool {
        false
    }
    pub const fn authority_created(&self) -> bool {
        false
    }
    pub const fn host_delivery_observed(&self) -> bool {
        false
    }
}
