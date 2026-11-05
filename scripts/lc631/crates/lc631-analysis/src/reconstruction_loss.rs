//! TLCX-18 bounded preparation over declared finite inputs.
//! No parser, solver, receipt, isolation sandbox, scalar K1 or independent proof.

use lc631_substrate::Digest32;

const MAX_PROBES: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LossError {
    EmptyProbeSet,
    ResourceExhausted,
    ProbeSetChanged,
    DecoderChanged,
    TargetSlotMissing,
    Incomparable,
    TruthTableLength,
    ContextInconsistent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbeOutcome {
    Preserved,
    Violated,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputValidation {
    ReportedInputs,
}

/// Concrete decoder instructions contain target indexes only.
/// A host must separately establish process/source isolation if required.
#[derive(Debug)]
pub struct TargetDecoder<'a> {
    revision: Digest32,
    target_slots: &'a [usize],
}

impl<'a> TargetDecoder<'a> {
    pub fn new(revision: Digest32, target_slots: &'a [usize]) -> Result<Self, LossError> {
        check_count(target_slots.len())?;
        Ok(Self {
            revision,
            target_slots,
        })
    }

    pub fn decode(&self, target: &TargetPayload<'_>) -> Result<DecodedFeatures, LossError> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(self.target_slots.len())
            .map_err(|_| LossError::ResourceExhausted)?;
        for &slot in self.target_slots {
            values.push(
                *target
                    .features
                    .get(slot)
                    .ok_or(LossError::TargetSlotMissing)?,
            );
        }
        Ok(DecodedFeatures {
            decoder_revision: self.revision,
            target_revision: target.revision,
            values,
        })
    }
}

/// Declared target payload: no source handle, source key or callback is exposed.
#[derive(Debug)]
pub struct TargetPayload<'a> {
    revision: Digest32,
    features: &'a [Option<bool>],
}

impl<'a> TargetPayload<'a> {
    pub fn new(revision: Digest32, features: &'a [Option<bool>]) -> Result<Self, LossError> {
        check_count(features.len())?;
        Ok(Self { revision, features })
    }
}

#[derive(Debug)]
pub struct DecodedFeatures {
    decoder_revision: Digest32,
    target_revision: Digest32,
    values: Vec<Option<bool>>,
}

/// Owned by the comparison caller, never exposed to TargetDecoder.
#[derive(Debug)]
pub struct ReconstructionPlan<'a> {
    probe_set: Digest32,
    decoder_revision: Digest32,
    expected: &'a [bool],
}

impl<'a> ReconstructionPlan<'a> {
    pub fn new(
        probe_set: Digest32,
        decoder_revision: Digest32,
        expected: &'a [bool],
    ) -> Result<Self, LossError> {
        check_count(expected.len())?;
        Ok(Self {
            probe_set,
            decoder_revision,
            expected,
        })
    }

    pub fn compare(&self, decoded: &DecodedFeatures) -> Result<ReconstructionReport, LossError> {
        if self.decoder_revision != decoded.decoder_revision {
            return Err(LossError::DecoderChanged);
        }
        if self.expected.len() != decoded.values.len() {
            return Err(LossError::ProbeSetChanged);
        }
        let mut outcomes = Vec::new();
        outcomes
            .try_reserve_exact(self.expected.len())
            .map_err(|_| LossError::ResourceExhausted)?;
        for (expected, actual) in self.expected.iter().zip(&decoded.values) {
            outcomes.push(match actual {
                Some(value) if value == expected => ProbeOutcome::Preserved,
                Some(_) => ProbeOutcome::Violated,
                None => ProbeOutcome::Unknown,
            });
        }
        Ok(ReconstructionReport {
            probe_set: self.probe_set,
            decoder_revision: decoded.decoder_revision,
            target_revision: decoded.target_revision,
            outcomes,
        })
    }
}

/// Per-probe data for a future canonical ledger; never a completed metric.
#[derive(Debug)]
pub struct ReconstructionReport {
    probe_set: Digest32,
    decoder_revision: Digest32,
    target_revision: Digest32,
    outcomes: Vec<ProbeOutcome>,
}

impl ReconstructionReport {
    pub fn outcomes(&self) -> &[ProbeOutcome] {
        &self.outcomes
    }
    pub const fn probe_set(&self) -> Digest32 {
        self.probe_set
    }
    pub const fn decoder_revision(&self) -> Digest32 {
        self.decoder_revision
    }
    pub const fn target_revision(&self) -> Digest32 {
        self.target_revision
    }
    pub const fn basis(&self) -> InputValidation {
        InputValidation::ReportedInputs
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Preservation {
    Equivalence,
    Soundness,
    Completeness,
}

/// An explicit predicate truth table over a caller-declared finite signature.
/// Row order is the little-endian valuation index; equal signature digests are
/// reported bindings, not independently checked symbol correspondences.
#[derive(Debug)]
pub struct FiniteTruthSet<'a> {
    signature: Digest32,
    variables: u8,
    rows: &'a [bool],
}

impl<'a> FiniteTruthSet<'a> {
    pub fn new(signature: Digest32, variables: u8, rows: &'a [bool]) -> Result<Self, LossError> {
        if variables > 8 {
            return Err(LossError::ResourceExhausted);
        }
        if rows.len() != (1usize << variables) {
            return Err(LossError::TruthTableLength);
        }
        Ok(Self {
            signature,
            variables,
            rows,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticReport {
    outcome: ProbeOutcome,
    counterexample: Option<u16>,
    preservation: Preservation,
    signature: Digest32,
}

impl SemanticReport {
    pub const fn outcome(self) -> ProbeOutcome {
        self.outcome
    }
    pub const fn counterexample(self) -> Option<u16> {
        self.counterexample
    }
    pub const fn preservation(self) -> Preservation {
        self.preservation
    }
    pub const fn signature(self) -> Digest32 {
        self.signature
    }
    pub const fn basis(self) -> InputValidation {
        InputValidation::ReportedInputs
    }
}

pub fn compare_semantics(
    source: &FiniteTruthSet<'_>,
    target: &FiniteTruthSet<'_>,
    preservation: Preservation,
) -> Result<SemanticReport, LossError> {
    compare_rows(None, source, target, preservation)
}

/// A classical inconsistent context is rejected before entailment comparison.
/// This pure table operation is not the K5 bridge from external solver reports.
pub fn compare_in_context(
    context: &FiniteTruthSet<'_>,
    source: &FiniteTruthSet<'_>,
    target: &FiniteTruthSet<'_>,
    preservation: Preservation,
) -> Result<SemanticReport, LossError> {
    compatible(context, source)?;
    if !context.rows.iter().any(|&row| row) {
        return Err(LossError::ContextInconsistent);
    }
    compare_rows(Some(context), source, target, preservation)
}

fn compare_rows(
    context: Option<&FiniteTruthSet<'_>>,
    source: &FiniteTruthSet<'_>,
    target: &FiniteTruthSet<'_>,
    preservation: Preservation,
) -> Result<SemanticReport, LossError> {
    compatible(source, target)?;
    let mut counterexample = None;
    for (index, (&left, &right)) in source.rows.iter().zip(target.rows).enumerate() {
        if context.is_some_and(|ctx| !ctx.rows[index]) {
            continue;
        }
        let violated = match preservation {
            Preservation::Equivalence => left != right,
            Preservation::Soundness => right && !left,
            Preservation::Completeness => left && !right,
        };
        if violated {
            counterexample = Some(index as u16); // At most 256 checked rows.
            break;
        }
    }
    Ok(SemanticReport {
        outcome: if counterexample.is_some() {
            ProbeOutcome::Violated
        } else {
            ProbeOutcome::Preserved
        },
        counterexample,
        preservation,
        signature: source.signature,
    })
}

fn compatible(left: &FiniteTruthSet<'_>, right: &FiniteTruthSet<'_>) -> Result<(), LossError> {
    if left.signature != right.signature || left.variables != right.variables {
        return Err(LossError::Incomparable);
    }
    Ok(())
}

fn check_count(count: usize) -> Result<(), LossError> {
    if count == 0 {
        return Err(LossError::EmptyProbeSet);
    }
    if count > MAX_PROBES {
        return Err(LossError::ResourceExhausted);
    }
    Ok(())
}
