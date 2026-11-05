//! Source-region context lowering. Parsed declarations remain advisory;
//! task-specific logical interpretation belongs to the later solver projection.
use crate::{
    verify_dgcl_pipeline_identity, DgclInstructionParse, DgclInstructionParseState,
    DgclPipelineReport, DgclRequirementBinding,
};
use lc631_core::{context::*, AuthorityRevision};
use lc631_substrate::{
    source::{SourceError, SourceSpan, SourceStore},
    Digest32,
};
use lc631_tldg::{Dg1Language, Dg1ResidualReason, Dg1SourceRole, SourceTextView};
use sha2::{Digest, Sha256};

/// Syntax support is not a verified proposition or permission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoweredSemantics {
    AdvisoryParsed,
    Ambiguous,
    Unresolved,
    Unsupported,
    ResourceExhausted,
    Unknown,
    NotAsserted,
}

#[derive(Clone, Debug)]
pub struct LoweredRegion {
    id: Digest32,
    span: SourceSpan,
    role: Dg1SourceRole,
    legacy_region_id: Option<u32>,
    residual_reason: Option<Dg1ResidualReason>,
    semantics: LoweredSemantics,
}

impl LoweredRegion {
    pub const fn id(&self) -> Digest32 {
        self.id
    }
    pub const fn span(&self) -> SourceSpan {
        self.span
    }
    pub const fn role(&self) -> Dg1SourceRole {
        self.role
    }
    pub const fn legacy_region_id(&self) -> Option<u32> {
        self.legacy_region_id
    }
    pub const fn residual_reason(&self) -> Option<Dg1ResidualReason> {
        self.residual_reason
    }
    pub const fn semantics(&self) -> LoweredSemantics {
        self.semantics
    }
}

#[derive(Clone, Debug)]
pub struct LoweredResidual {
    text: SourceTextView,
    reason: Dg1ResidualReason,
    language: Dg1Language,
}

impl LoweredResidual {
    pub fn text(&self) -> &str {
        self.text.as_str()
    }
    pub fn span(&self) -> SourceSpan {
        self.text.span()
    }
    pub const fn reason(&self) -> Dg1ResidualReason {
        self.reason
    }
    pub const fn language(&self) -> Dg1Language {
        self.language
    }
}

/// A checked pipeline-to-core bridge, not a verifier or receipt issuer.
/// Every source byte remains addressable, including quote/code/unknown regions.
pub struct LoweredContext {
    source: SourceStore,
    original: OriginalContext,
    regions: Vec<LoweredRegion>,
    residuals: Vec<LoweredResidual>,
    parses: Vec<DgclInstructionParse>,
    bindings: Vec<DgclRequirementBinding>,
    unsupported_operative_region_ids: Vec<u32>,
}

impl LoweredContext {
    pub fn source_store(&self) -> &SourceStore {
        &self.source
    }
    pub fn original(&self) -> &OriginalContext {
        &self.original
    }
    pub fn regions(&self) -> &[LoweredRegion] {
        &self.regions
    }
    pub fn residuals(&self) -> &[LoweredResidual] {
        &self.residuals
    }
    /// Arc-backed selected/alternative ASTs stay attached to their source.
    pub fn advisory_parses(&self) -> &[DgclInstructionParse] {
        &self.parses
    }
    pub fn advisory_bindings(&self) -> &[DgclRequirementBinding] {
        &self.bindings
    }
    pub fn unsupported_operative_region_ids(&self) -> &[u32] {
        &self.unsupported_operative_region_ids
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextLoweringError {
    PipelineIdentity,
    Source(SourceError),
    SourcePartition,
    ResidualMismatch,
    AllocationFailed,
    Context(ContextError),
}

/// Freeze parser-reported source regions, without treating extraction as
/// exhaustive semantics. Explicit instruction and nonoperative regions cannot
/// be edited as ordinary premises; only declared Context regions are mutable.
pub fn lower_dgcl_context(
    report: &DgclPipelineReport,
    scope: ContextScope,
    authority: AuthorityRevision,
    limits: ContextLimits,
) -> Result<LoweredContext, ContextLoweringError> {
    let store = report.source_store();
    if !verify_dgcl_pipeline_identity(store.text(), report) {
        return Err(ContextLoweringError::PipelineIdentity);
    }
    let source = SourceRevision::of(store.text());
    let validated = OriginalContext::checked(source, scope, authority, vec![], limits)
        .map_err(ContextLoweringError::Context)?;
    let limits = validated.limits();
    check_counts(report, limits)?;
    let (regions, premises) = lower_regions(report)?;
    let residuals = lower_residuals(report)?;
    let original = OriginalContext::checked(source, scope, authority, premises, limits)
        .map_err(ContextLoweringError::Context)?;
    Ok(LoweredContext {
        source: store.clone(),
        original,
        regions,
        residuals,
        parses: checked_clone(&report.instruction_parses)?,
        bindings: checked_clone(&report.requirement_bindings)?,
        unsupported_operative_region_ids: checked_clone(&report.unsupported_operative_region_ids)?,
    })
}

fn check_counts(
    report: &DgclPipelineReport,
    limits: ContextLimits,
) -> Result<(), ContextLoweringError> {
    for count in [
        report.dg1.source_accounting.len(),
        report.dg1.instruction_residuals.len(),
        report.instruction_parses.len(),
        report.requirement_bindings.len(),
        report.unsupported_operative_region_ids.len(),
    ] {
        if count > limits.max_premises {
            return Err(ContextLoweringError::Context(ContextError::BudgetExceeded));
        }
    }
    if report.source_store().text().len() > limits.max_statement_bytes {
        return Err(ContextLoweringError::Context(ContextError::BudgetExceeded));
    }
    Ok(())
}

fn reserved<T>(count: usize) -> Result<Vec<T>, ContextLoweringError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| ContextLoweringError::AllocationFailed)?;
    Ok(values)
}

fn checked_clone<T: Clone>(input: &[T]) -> Result<Vec<T>, ContextLoweringError> {
    let mut values = reserved(input.len())?;
    values.extend_from_slice(input);
    Ok(values)
}

fn lower_regions(
    report: &DgclPipelineReport,
) -> Result<(Vec<LoweredRegion>, Vec<ContextPremise>), ContextLoweringError> {
    let store = report.source_store();
    let mut regions = reserved(report.dg1.source_accounting.len())?;
    let mut premises = reserved(report.dg1.source_accounting.len())?;
    let mut end = 0;
    for segment in &report.dg1.source_accounting {
        let range = &segment.source_span;
        if range.start != end || range.start >= range.end {
            return Err(ContextLoweringError::SourcePartition);
        }
        let span = SourceSpan::checked(store, range.start, range.end)
            .map_err(ContextLoweringError::Source)?;
        let id = region_id(span, segment.role);
        let premise = ContextPremise::from_source(
            id,
            store,
            span,
            PremiseBasis::Proposed,
            segment.role == Dg1SourceRole::Context,
        )
        .map_err(ContextLoweringError::Source)?;
        premises.push(premise);
        regions.push(LoweredRegion {
            id,
            span,
            role: segment.role,
            legacy_region_id: segment.region_id,
            residual_reason: segment.residual_reason,
            semantics: region_semantics(report, span, segment.role),
        });
        end = range.end;
    }
    if end != store.text().len() {
        return Err(ContextLoweringError::SourcePartition);
    }
    Ok((regions, premises))
}

fn lower_residuals(
    report: &DgclPipelineReport,
) -> Result<Vec<LoweredResidual>, ContextLoweringError> {
    let mut residuals = reserved(report.dg1.instruction_residuals.len())?;
    for residual in &report.dg1.instruction_residuals {
        let text = SourceTextView::checked(
            report.source_store(),
            residual.source_span.start,
            residual.source_span.end,
        )
        .map_err(ContextLoweringError::Source)?;
        if text.as_str() != residual.source_text {
            return Err(ContextLoweringError::ResidualMismatch);
        }
        residuals.push(LoweredResidual {
            text,
            reason: residual.reason,
            language: residual.language,
        });
    }
    Ok(residuals)
}

fn region_semantics(
    report: &DgclPipelineReport,
    span: SourceSpan,
    role: Dg1SourceRole,
) -> LoweredSemantics {
    match role {
        Dg1SourceRole::Quoted
        | Dg1SourceRole::Code
        | Dg1SourceRole::Opaque
        | Dg1SourceRole::MarkdownSyntax => LoweredSemantics::NotAsserted,
        Dg1SourceRole::Unsupported => LoweredSemantics::Unsupported,
        Dg1SourceRole::Ambiguous => LoweredSemantics::Ambiguous,
        Dg1SourceRole::Context => LoweredSemantics::Unknown,
        Dg1SourceRole::InstructionCandidate => report
            .instruction_parses
            .iter()
            .find(|parse| {
                parse.source_span.start <= span.start() && span.end() <= parse.source_span.end
            })
            .map_or(LoweredSemantics::Unknown, |parse| match parse.state {
                DgclInstructionParseState::Parsed => LoweredSemantics::AdvisoryParsed,
                DgclInstructionParseState::Ambiguous => LoweredSemantics::Ambiguous,
                DgclInstructionParseState::Unresolved => LoweredSemantics::Unresolved,
                DgclInstructionParseState::Unsupported => LoweredSemantics::Unsupported,
                DgclInstructionParseState::BudgetRejected => LoweredSemantics::ResourceExhausted,
            }),
    }
}

fn region_id(span: SourceSpan, role: Dg1SourceRole) -> Digest32 {
    let mut hash = Sha256::new();
    hash.update(b"epistesys.source-context-region.v1\0");
    hash.update(span.source_id().digest().as_bytes());
    hash.update((span.start() as u64).to_le_bytes());
    hash.update((span.end() as u64).to_le_bytes());
    hash.update([match role {
        Dg1SourceRole::InstructionCandidate => 0,
        Dg1SourceRole::Context => 1,
        Dg1SourceRole::Quoted => 2,
        Dg1SourceRole::Code => 3,
        Dg1SourceRole::Opaque => 4,
        Dg1SourceRole::Unsupported => 5,
        Dg1SourceRole::Ambiguous => 6,
        Dg1SourceRole::MarkdownSyntax => 7,
    }]);
    Digest32::from_bytes(hash.finalize().into())
}
