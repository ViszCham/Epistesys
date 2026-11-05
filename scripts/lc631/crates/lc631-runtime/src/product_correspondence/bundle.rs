//! Exact accounting of PROVIDED complete bytes, never executable closure proof.
//! No source/context/owner/envelope lookup or supplied-code execution occurs.
use super::codec::{app, decode, encode, sym};
use super::{claim, digest, reserved, DeferredReason, ProductError, ProductPlan, SCRATCH_BYTES};
use lc631_substrate::{
    resource::{ResourceClaim, ResourceReservationView},
    Digest32,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodeRole {
    Projection,
    Unprojection,
    CorrespondenceAdapter,
}
#[derive(Clone, Copy)]
pub enum ProvidedPayload<'a> {
    Inline(&'a [u8]),
    ExternalReference(&'a [u8]),
}
pub struct ProvidedCodeUnit<'a> {
    pub role: CodeRole,
    pub payload: ProvidedPayload<'a>,
}
pub struct ProvidedBundleInput<'a> {
    pub semantic: &'a [u8],
    pub code_units: &'a [ProvidedCodeUnit<'a>],
    pub semantic_dictionary: ProvidedPayload<'a>,
}
pub struct ProvidedBundleAccounting<'v> {
    bytes: Vec<u8>,
    bits: u64,
    binding: Digest32,
    _retained: ResourceClaim<'v>,
}
impl ProvidedBundleAccounting<'_> {
    pub fn bundle_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn bits(&self) -> u64 {
        self.bits
    }
    pub fn binding(&self) -> Digest32 {
        self.binding
    }
    pub fn scalar_status(&self) -> DeferredReason {
        DeferredReason::FullCostOwnerMissing
    }
}
fn inline<'a>(
    payload: ProvidedPayload<'a>,
    missing: DeferredReason,
) -> Result<&'a [u8], ProductError> {
    match payload {
        ProvidedPayload::Inline(bytes) if !bytes.is_empty() => Ok(bytes),
        ProvidedPayload::Inline(_) => Err(ProductError::Deferred(missing)),
        ProvidedPayload::ExternalReference(_) => Err(ProductError::Deferred(
            DeferredReason::ExternalLookupRequired,
        )),
    }
}
impl<'v> ProductPlan<'v> {
    /// Role labels are Given declarations. Successful byte counting NEVER
    /// authenticates that supplied text implements those roles or is complete.
    pub fn account_provided_bundle(
        &self,
        input: ProvidedBundleInput<'_>,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<ProvidedBundleAccounting<'v>, ProductError> {
        self.check_current(view)?;
        if input.code_units.len() != 3 {
            return Err(ProductError::Deferred(DeferredReason::MissingInlineCode));
        }
        let dictionary = inline(
            input.semantic_dictionary,
            DeferredReason::MissingSemanticDictionary,
        )?;
        let mut bytes = input
            .semantic
            .len()
            .checked_add(dictionary.len())
            .ok_or(ProductError::UnsupportedProjection)?;
        for unit in input.code_units {
            bytes = bytes
                .checked_add(inline(unit.payload, DeferredReason::MissingInlineCode)?.len())
                .ok_or(ProductError::UnsupportedProjection)?;
        }
        if bytes > self.data.limits.max_bytes {
            return Err(ProductError::UnsupportedProjection);
        }
        let retained = claim(view, 1024 * 1024)?;
        let scratch = claim(view, SCRATCH_BYTES)?;
        let semantic = decode(input.semantic, self.data.limits)?;
        super::codec::validate(&self.data, &semantic)?;
        let dictionary = decode(dictionary, self.data.limits)?;
        let mut code = reserved(3)?;
        for (role, name) in [
            (CodeRole::Projection, "Projection"),
            (CodeRole::Unprojection, "Unprojection"),
            (CodeRole::CorrespondenceAdapter, "CorrespondenceAdapter"),
        ] {
            let mut matching = input.code_units.iter().filter(|u| u.role == role);
            let unit = matching
                .next()
                .ok_or(ProductError::Deferred(DeferredReason::MissingInlineCode))?;
            if matching.next().is_some() {
                return Err(ProductError::Deferred(DeferredReason::MissingInlineCode));
            }
            let text =
                std::str::from_utf8(inline(unit.payload, DeferredReason::MissingInlineCode)?)
                    .map_err(|_| ProductError::UnsupportedProjection)?;
            code.push(app("code-unit-v1", vec![sym(name), sym(text)]));
        }
        let term = app(
            "bundle-v1",
            vec![semantic, app("provided-code-closure-v1", code), dictionary],
        );
        let bytes = encode(&term, self.data.limits)?;
        let bits = u64::try_from(bytes.len())
            .map_err(|_| ProductError::UnsupportedProjection)?
            .checked_mul(8)
            .ok_or(ProductError::UnsupportedProjection)?;
        let binding = digest(b"epistesys.stage-b.provided-bundle-given-only.v1\0", &bytes);
        scratch.check()?;
        self.check_current(view)?;
        retained.check()?;
        Ok(ProvidedBundleAccounting {
            bytes,
            bits,
            binding,
            _retained: retained,
        })
    }
}
