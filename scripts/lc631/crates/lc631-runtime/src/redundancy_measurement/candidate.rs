//! 日本語: root固定inventoryと保護payload/宣言差分を候補に束縛する。
//! English: Bind candidates to fixed inventory, protected payloads and declared edits.
use super::*;
use lc631_substrate::resource::ResourceReservationView;

impl<'plan, 'input, 'view> BoundCandidateUnits<'plan, 'input, 'view> {
    pub fn checked(
        root: &'plan RootRedundancyPlan<'input, 'view>,
        candidate: CandidateId,
        artifact: Option<&'input SemanticArtifactInput<'input>>,
        changes: &'input [MutableChangeInput<'input>],
        view: &'view ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        root.check(view)?;
        root.debit(root::admission_envelope(
            root.input.slots.len(),
            root.limits,
            false,
        )?)?;
        let retained = ChainClaim::acquire(
            &root.budget,
            root.limits,
            resources::backing::<u8>(0, std::mem::size_of::<Self>())?,
            0,
            view,
        )?;
        let scratch = root::scratch(root.limits, view)?;
        if changes.len() > root.input.slots.len() {
            return Err(Error::InvalidDelta);
        }
        let (formulas, stats, gap, digest) = match artifact {
            None => {
                if !changes.is_empty() {
                    return Err(Error::InvalidDelta);
                }
                (
                    [None; 16],
                    [Stats::default(); 16],
                    Some(Gap::UnextractedArtifact),
                    syntax::hash(
                        b"epistesys.redundancy.unextracted-artifact.v1\0",
                        &[root.comparison_digest.as_bytes()],
                    ),
                )
            }
            Some(input) => {
                root::source(input.source, input.context, view)?;
                let admitted = syntax::artifact(
                    input,
                    root.input.slots,
                    root.model_digest,
                    root.limits,
                    root.input.model.theory.boolean_symbols(),
                )?;
                validate_changes(root, &admitted.formulas, changes)?;
                (
                    admitted.formulas,
                    admitted.stats,
                    root.baseline_gap.or(admitted.gap),
                    admitted.digest,
                )
            }
        };
        scratch.check().map_err(Error::Resource)?;
        let result = Self {
            root,
            candidate,
            artifact,
            formulas,
            stats,
            gap,
            digest,
            retained,
        };
        result.check(view)?;
        Ok(result)
    }
    pub(super) fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        self.root.check(view)?;
        self.retained.check(view)?;
        if let Some(artifact) = self.artifact {
            root::source(artifact.source, artifact.context, view)?;
        }
        Ok(())
    }
}
fn validate_changes(
    root: &RootRedundancyPlan<'_, '_>,
    formulas: &[Option<&BoolExpr>; 16],
    changes: &[MutableChangeInput<'_>],
) -> Result<(), Error> {
    for (i, change) in changes.iter().enumerate() {
        if changes[..i].iter().any(|old| old.slot == change.slot) {
            return Err(Error::InvalidDelta);
        }
        if !root
            .input
            .slots
            .iter()
            .any(|s| s.slot == change.slot && s.protection == Protection::Mutable)
        {
            return Err(Error::InvalidDelta);
        }
    }
    for (i, slot) in root.input.slots.iter().enumerate() {
        let before = root
            .input
            .baseline
            .units
            .iter()
            .find(|u| u.slot == slot.slot)
            .ok_or(Error::MissingSlot)?
            .formula;
        let change = changes.iter().find(|c| c.slot == slot.slot);
        match (before, formulas[i]) {
            (Some(a), Some(b)) if a != b => {
                // Exact declared payload equality is ONLY a no-edit guard;
                // semantic classes are always produced by actual evaluation.
                if slot.protection == Protection::HardUnchanged {
                    return Err(Error::UnsupportedProtectedRewrite);
                }
                let change = change.ok_or(Error::MissingDelta)?;
                if change.before != a || change.after != b {
                    return Err(Error::InvalidDelta);
                }
            }
            (Some(_), Some(_)) => {
                if change.is_some() {
                    return Err(Error::InvalidDelta);
                }
            }
            (None, _) | (_, None) => {
                if change.is_some() {
                    return Err(Error::InvalidDelta);
                }
            }
        }
    }
    Ok(())
}
