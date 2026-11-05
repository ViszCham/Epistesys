use super::connector::ExplanationBinding;
use super::{DependencyCoverage, OutputError, PreparedDecision, MEASUREMENT_REVISION};
use crate::joint_selection::{AxisKind, AxisValue, JointSelectionReport};
use lc631_core::context::{ContextState, PremiseEdit, SourceRevision};
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};
use std::fmt::{self, Write};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplanationTl {
    NeedsEvidence,
}

/// Immutable exact UTF-8 capture, not a signature or host-delivery observation.
pub struct RenderedDecision {
    source: SourceRevision,
    text: String,
    binding: Digest32,
    dependency_coverage: DependencyCoverage,
}

impl RenderedDecision {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub const fn binding(&self) -> Digest32 {
        self.binding
    }

    pub const fn explanation_tl(&self) -> ExplanationTl {
        ExplanationTl::NeedsEvidence
    }

    pub const fn dependency_coverage(&self) -> DependencyCoverage {
        self.dependency_coverage
    }

    pub fn matches_exact(&self, source: &str, bytes: &[u8]) -> bool {
        let source = SourceRevision::of(source);
        source == self.source
            && bytes == self.text.as_bytes()
            && bind(source, bytes) == self.binding
    }
}

impl PreparedDecision<'_> {
    /// JA prose precedes equivalent EN prose; raw reports never become truth.
    pub fn render(&self, max_bytes: usize) -> Result<RenderedDecision, OutputError> {
        self.render_complete(max_bytes, None)
    }

    pub(super) fn render_connected(
        &self,
        max_bytes: usize,
        report: &JointSelectionReport<'_, '_>,
        axes: &[AxisKind],
        bindings: &[ExplanationBinding],
    ) -> Result<RenderedDecision, OutputError> {
        self.render_complete(
            max_bytes,
            Some(SelectionRendering {
                report,
                axes,
                bindings,
            }),
        )
    }

    fn render_complete(
        &self,
        max_bytes: usize,
        selection: Option<SelectionRendering<'_, '_, '_>>,
    ) -> Result<RenderedDecision, OutputError> {
        if max_bytes == 0 || max_bytes > 64 * 1024 {
            return Err(OutputError::OutputBudget);
        }
        let mut out = BoundedText::new(max_bytes)?;
        self.render_language(&mut out, true, selection.as_ref())
            .map_err(|_| OutputError::OutputBudget)?;
        self.render_language(&mut out, false, selection.as_ref())
            .map_err(|_| OutputError::OutputBudget)?;
        let text = out.text;
        let binding = bind(self.source, text.as_bytes());
        Ok(RenderedDecision {
            source: self.source,
            text,
            binding,
            dependency_coverage: self.dependency_coverage(),
        })
    }

    fn render_language(
        &self,
        out: &mut BoundedText,
        japanese: bool,
        selection: Option<&SelectionRendering<'_, '_, '_>>,
    ) -> fmt::Result {
        let title = if japanese {
            "日本語・準備的説明"
        } else {
            "English: preparatory explanation"
        };
        writeln!(out, "{title}")?;
        writeln!(out, "{MEASUREMENT_REVISION}")?;
        writeln!(out, "source: {}", Hex(self.source.digest()))?;
        let boundary = if japanese {
            "意味検証・最適性・説明TLはNeedsEvidence。権限・完了・host配信は未観測。"
        } else {
            "Semantic validation, optimality and explanation TL: NeedsEvidence. Authority, completion and host delivery: unobserved."
        };
        writeln!(out, "{boundary}")?;
        writeln!(out, "dependency coverage: {:?}", self.dependency_coverage())?;
        writeln!(
            out,
            "{}",
            if japanese {
                "構造的な前提照合だけであり、自由文の依存完全性と意味はNeedsEvidence。Givenは宣言前提。"
            } else {
                "Premise accounting is structural only; arbitrary-prose dependency completeness and semantics: NeedsEvidence. Given is a declared premise."
            }
        )?;
        writeln!(
            out,
            "selection: {:?}; goal: {}",
            self.state(),
            OptionalHex(self.declared_goal)
        )?;
        if let Some(selection) = selection {
            render_selection(out, selection)?;
        }
        for (index, candidate) in self.candidates.iter().enumerate() {
            let role = if self.recommendation == Some(index) {
                if japanese {
                    "宣言された作業上の推奨"
                } else {
                    "Declared operational recommendation"
                }
            } else if japanese {
                "代替候補"
            } else {
                "Alternative candidate"
            };
            writeln!(out, "{role}: {}", Hex(candidate.result))?;
            writeln!(
                out,
                "context: {}; applicability: {:?}",
                Hex(candidate.context.revision().interpretation_digest()),
                candidate.context.applicability()
            )?;
            writeln!(out, "reported explanation: {}", candidate.explanation)?;
            let original = candidate
                .context_history
                .first()
                .copied()
                .unwrap_or(candidate.context);
            render_context(out, "original", original)?;
            render_context(out, "current", candidate.context)?;
            writeln!(
                out,
                "D: {}; Delta: {}; U: {}; G: {} (reported refs; no measured zero)",
                OptionalHex(candidate.facets.defects),
                OptionalHex(candidate.facets.delta),
                OptionalHex(candidate.facets.uncertainty),
                OptionalHex(candidate.facets.gain)
            )?;
            for ancestor in candidate.context_history {
                render_changes(out, ancestor.declared_delta())?;
            }
            render_changes(out, candidate.context.declared_delta())?;
            for dependency in candidate.dependencies {
                writeln!(
                    out,
                    "declared premise dependency: {} {:?}",
                    Hex(dependency.premise),
                    dependency.state
                )?;
            }
        }
        writeln!(out)
    }
}

fn render_context(out: &mut BoundedText, role: &str, context: &ContextState) -> fmt::Result {
    writeln!(
        out,
        "{role} context: {}; premises: {}",
        Hex(context.revision().interpretation_digest()),
        context.premises().len()
    )?;
    for premise in context.premises() {
        write!(
            out,
            "{role} premise: {}; basis: {:?}; changeable: {}; source span: ",
            Hex(premise.id()),
            premise.basis(),
            premise.changeable()
        )?;
        match premise.source_span() {
            Some(span) => write!(out, "[{},{}); ", span.start(), span.end())?,
            None => write!(out, "none (reported text); ")?,
        }
        // Quoted display escapes controls/newlines; it is not a new semantic
        // canonicalization. The existing final-byte binding captures the result.
        writeln!(out, "statement: {:?}", premise.statement())?;
    }
    Ok(())
}

struct SelectionRendering<'s, 'a, 'v> {
    report: &'s JointSelectionReport<'a, 'v>,
    axes: &'s [AxisKind],
    bindings: &'s [ExplanationBinding],
}

fn render_selection(
    out: &mut BoundedText,
    selection: &SelectionRendering<'_, '_, '_>,
) -> fmt::Result {
    let report = selection.report;
    writeln!(
        out,
        "comparison scope: {}; goal scope: {}; probes: {}; semantics: {}",
        Hex(report.pins.scope.comparison()),
        Hex(report.pins.scope.goal()),
        Hex(report.pins.scope.probes()),
        Hex(report.pins.scope.semantics())
    )?;
    writeln!(
        out,
        "reported selection: {:?}; reported optimality: {:?}",
        report.recommendation_state, report.optimality
    )?;
    writeln!(
        out,
        "policy: {}; baseline: {}; hard invariants: {}; evidence revision: {}",
        Hex(report.pins.policy),
        Hex(report.pins.baseline),
        Hex(report.pins.hard_invariants),
        Hex(report.pins.evidence_revision)
    )?;
    for ((row, disposition), binding) in report
        .candidates
        .iter()
        .zip(&report.dispositions)
        .zip(selection.bindings)
    {
        writeln!(
            out,
            "result: {}; measurement: {}; disposition: {:?}; reported hard/map-back: {:?}/{:?}",
            Hex(binding.result),
            Hex(binding.measurement),
            disposition,
            row.hard_gate,
            row.map_back
        )?;
        for (axis, value) in selection.axes.iter().zip(row.axes) {
            write!(out, "reported axis {:?}: ", axis)?;
            render_value(out, value)?;
        }
        write!(out, "reported total loss: ")?;
        render_value(out, &row.total_loss)?;
    }
    Ok(())
}

fn render_value(out: &mut BoundedText, value: &AxisValue<'_>) -> fmt::Result {
    match value {
        AxisValue::Exact(interval) => writeln!(
            out,
            "ExactArithmetic [{},{}] (input semantics unverified)",
            interval.lower(),
            interval.upper()
        ),
        AxisValue::Bounded(interval) => writeln!(
            out,
            "Bounded [{},{}] (input semantics unverified)",
            interval.lower(),
            interval.upper()
        ),
        AxisValue::Estimated(interval) => writeln!(
            out,
            "Estimated [{},{}] (input semantics unverified)",
            interval.lower(),
            interval.upper()
        ),
        AxisValue::Unavailable(reason) => writeln!(out, "Unavailable/{reason:?}"),
    }
}

fn render_changes(
    out: &mut BoundedText,
    delta: Option<&lc631_core::context::ContextDelta>,
) -> fmt::Result {
    match delta {
        None => writeln!(out, "context changes: none in supplied structural context"),
        Some(delta) => {
            writeln!(
                out,
                "parent: {}",
                Hex(delta.parent().interpretation_digest())
            )?;
            for edit in delta.edits() {
                match edit {
                    PremiseEdit::Add(premise) => writeln!(
                        out,
                        "context change/add: {} {:?}: {:?}",
                        Hex(premise.id()),
                        premise.basis(),
                        premise.statement()
                    )?,
                    PremiseEdit::Replace {
                        id,
                        statement,
                        basis,
                    } => writeln!(
                        out,
                        "context change/replace: {} {:?}: {:?}",
                        Hex(*id),
                        basis,
                        statement
                    )?,
                    PremiseEdit::Retract { id } => {
                        writeln!(out, "context change/retract: {}", Hex(*id))?
                    }
                }
            }
            Ok(())
        }
    }
}

fn bind(source: SourceRevision, bytes: &[u8]) -> Digest32 {
    let mut hash = Sha256::new();
    hash.update(b"epistesys.tlcx29.preparatory-output.v1\0");
    hash.update(source.digest().as_bytes());
    hash.update(bytes);
    Digest32::from_bytes(hash.finalize().into())
}

struct BoundedText {
    text: String,
    max_bytes: usize,
}

impl BoundedText {
    fn new(max_bytes: usize) -> Result<Self, OutputError> {
        let mut text = String::new();
        text.try_reserve_exact(max_bytes)
            .map_err(|_| OutputError::AllocationFailed)?;
        Ok(Self { text, max_bytes })
    }
}

impl Write for BoundedText {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let end = self.text.len().checked_add(value.len()).ok_or(fmt::Error)?;
        if end > self.max_bytes {
            return Err(fmt::Error);
        }
        self.text.push_str(value);
        Ok(())
    }
}

struct Hex(Digest32);
impl fmt::Display for Hex {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0.as_bytes() {
            write!(out, "{byte:02x}")?;
        }
        Ok(())
    }
}

struct OptionalHex(Option<Digest32>);
impl fmt::Display for OptionalHex {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(value) => write!(out, "{}", Hex(value)),
            None => out.write_str("Unknown/NeedsEvidence"),
        }
    }
}
