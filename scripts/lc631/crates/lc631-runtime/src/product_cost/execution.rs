//! Claims precede parsing/collection. Only completed A/P publish exact K.
use super::machine::{Engine, Meter, Phase};
use super::*;
use lc631_substrate::resource::{ResourceClaim, ResourceReservationView};
pub(super) struct Workspace<'v> {
    claim: ResourceClaim<'v>,
    native_host_bytes: u64,
}
impl Workspace<'_> {
    pub(super) fn admit_native_host(&mut self, bytes: u64) -> Result<(), CostError> {
        self.claim.check()?;
        if bytes > 8 * 1024 * 1024 {
            return Err(CostError::ResourceExhausted);
        }
        self.native_host_bytes = self.native_host_bytes.max(bytes);
        Ok(())
    }
}
pub(super) fn reserve<'v>(
    view: &'v ResourceReservationView<'_>,
    limits: CostLimits,
) -> Result<Workspace<'v>, CostError> {
    // This implementation admits the bounded 32MiB execution workspace only.
    // Smaller declared budgets remain valid caps, but cannot fund this engine.
    if limits.additional_ram_bytes < CostLimits::default().additional_ram_bytes {
        return Err(CostError::ResourceExhausted);
    }
    Ok(Workspace {
        claim: claim(view, limits.additional_ram_bytes)?,
        native_host_bytes: 0,
    })
}
pub struct OwnedExecutableBundleCost<'v> {
    pub(super) report: CostObservation,
    retained: ResourceClaim<'v>,
    limits: CostLimits,
    ranges: [std::ops::Range<usize>; 7],
}
impl OwnedExecutableBundleCost<'_> {
    pub fn historical_report(&self) -> &CostObservation {
        &self.report
    }
    pub(super) fn check_current(
        &self,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), CostError> {
        view.check()?;
        self.retained.check()?;
        if !self.retained.same_reservation(view) {
            return Err(CostError::ForeignOwner);
        }
        Ok(())
    }
    pub fn target_bundle(&self) -> TargetBundleView<'_> {
        TargetBundleView::observed(&self.report.bundle, self.limits, &self.ranges)
    }
}
pub fn execute_reported<'v>(
    profile: &DeclaredCostProfile<'v>,
    input: ReportedFieldInput<'_>,
    package: InlinePackage<'_>,
    view: &'v ResourceReservationView<'_>,
) -> Result<OwnedExecutableBundleCost<'v>, CostError> {
    profile.check_current(view)?;
    // Cheap byte caps and missing-payload cuts precede any parse/copy.
    preflight(input.fields, input.declarations, package, profile.limits)?;
    let workspace = reserve(view, profile.limits)?;
    let mut meter = Meter::new(view, profile.limits.fuel_per_direction);
    let fields = decode_input(input.fields, profile.limits, &mut meter)?;
    term::children(&fields, "reported-fields-v1")?;
    let declarations = decode_input(input.declarations, profile.limits, &mut meter)?;
    let initial = meter.used;
    execute(
        profile,
        fields,
        declarations,
        package,
        view,
        workspace,
        initial,
    )
}
pub(super) fn execute_fields<'v>(
    profile: &DeclaredCostProfile<'v>,
    fields: Term,
    forward_declarations: Term,
    package: InlinePackage<'_>,
    view: &'v ResourceReservationView<'_>,
    workspace: Workspace<'v>,
    initial_codec_fuel: u64,
) -> Result<OwnedExecutableBundleCost<'v>, CostError> {
    profile.check_current(view)?;
    workspace.claim.check()?;
    if !workspace.claim.same_reservation(view) {
        return Err(CostError::ForeignOwner);
    }
    let c = term::children(&fields, "field-stream-v1")?;
    if c.len() != 3 {
        return Err(CostError::CorrespondenceMismatch);
    }
    let mut checks = Meter::new(view, profile.limits.fuel_per_direction);
    checks.debit(initial_codec_fuel)?;
    if !codec::equal_term(&c[1], &forward_declarations, &mut checks)? {
        return Err(CostError::CorrespondenceMismatch);
    }
    execute(
        profile,
        fields,
        forward_declarations,
        package,
        view,
        workspace,
        checks.used,
    )
}
fn preflight(
    fields: &[u8],
    declarations: &[u8],
    p: InlinePackage<'_>,
    l: CostLimits,
) -> Result<(), CostError> {
    let parts = [
        p.adapter,
        p.projection,
        p.inverse,
        p.helpers,
        p.runtime,
        p.dictionary,
    ];
    if parts.iter().any(|p| p.is_empty()) {
        return Err(CostError::MissingDependency);
    }
    if fields.len() as u64 > l.semantic_bytes
        || declarations.len() as u64 > l.semantic_bytes
        || parts.iter().any(|p| p.len() as u64 > l.bundle_bytes)
    {
        return Err(CostError::ResourceExhausted);
    }
    Ok(())
}
fn decode_input(bytes: &[u8], l: CostLimits, m: &mut Meter<'_>) -> Result<Term, CostError> {
    let nodes = codec::scan(bytes, l, m)?;
    m.chunks(2 * (bytes.len() as u64 + nodes))?;
    term::decode(bytes, l)
}
fn execute<'v>(
    profile: &DeclaredCostProfile<'v>,
    fields: Term,
    declarations: Term,
    p: InlinePackage<'_>,
    view: &'v ResourceReservationView<'_>,
    workspace: Workspace<'v>,
    initial_codec: u64,
) -> Result<OwnedExecutableBundleCost<'v>, CostError> {
    profile.check_current(view)?;
    workspace.claim.check()?;
    let limits = profile.limits;
    let mut codec_meter = Meter::new(view, limits.fuel_per_direction);
    codec_meter.debit(initial_codec)?;
    let input_bytes = term::length(&fields, limits)?
        .checked_add(term::length(&declarations, limits)?)
        .ok_or(CostError::ResourceExhausted)?;
    let input_nodes = term::nodes(&fields)?
        .checked_add(term::nodes(&declarations)?)
        .ok_or(CostError::ResourceExhausted)?;
    let other_host = input_bytes
        .checked_mul(8)
        .and_then(|v| input_nodes.checked_mul(256).and_then(|n| v.checked_add(n)))
        .and_then(|v| v.checked_add(1024 * 1024))
        .ok_or(CostError::ResourceExhausted)?;
    let host_budget = limits
        .additional_ram_bytes
        .checked_mul(3)
        .map(|v| v / 4)
        .and_then(|v| v.checked_sub(workspace.native_host_bytes))
        .ok_or(CostError::ResourceExhausted)?;
    let package = codec::Package::parse(p, limits, &mut codec_meter, host_budget, other_host)?;
    codec::input_allowed(&fields, &mut codec_meter)?;
    if p.runtime != profile.runtime_bytes {
        return Err(CostError::ProfileMismatch);
    }
    let forward = codec::declaration_rows(&declarations, false, limits, &mut codec_meter)?;
    let paid = package.dictionary_rows()?;
    if forward.len() != paid.len() {
        return Err(CostError::CorrespondenceMismatch);
    }
    for row in forward {
        let mut matches = 0;
        for p in paid {
            if codec::equal_term(row, p, &mut codec_meter)? {
                matches += 1
            }
        }
        if matches != 1 {
            return Err(CostError::CorrespondenceMismatch);
        }
    }
    // Inventory comparison is real payload comparison, not diagnostic counts.
    codec_meter.chunks(term::work(&declarations, limits)?)?;
    let declaration_bytes = term::encode(&declarations, limits)?;
    let mut stats = ExecutionStats {
        meaning_declarations: forward.len() as u64,
        matched_dictionary_declarations: paid.len() as u64,
        ..ExecutionStats::default()
    };
    let mut adapter_meter = Meter::new(view, limits.fuel_per_direction);
    let engine = Engine::new(&package, limits, &mut adapter_meter, &mut stats)?;
    let field_value = engine.input(&fields, &mut adapter_meter, &mut stats)?;
    let semantic = machine::run(
        &engine,
        Phase::Adapter,
        field_value,
        &mut adapter_meter,
        &mut stats,
    )?;
    let semantic_bytes = engine.encode(&semantic, &mut adapter_meter, &mut stats)?;
    let mut projection_meter = Meter::new(view, limits.fuel_per_direction);
    let target = machine::run(
        &engine,
        Phase::Projection,
        semantic,
        &mut projection_meter,
        &mut stats,
    )?;
    let representation = engine.encode(&target, &mut projection_meter, &mut stats)?;
    let (bundle, ranges) = bundle(&representation, p, limits, &mut projection_meter)?;
    let dictionary_bytes = copy_bytes(p.dictionary, &mut projection_meter)?;
    let bits = (bundle.len() as u64)
        .checked_mul(8)
        .ok_or(CostError::InvalidProgram)?;
    let mut inverse_meter = Meter::new(view, limits.fuel_per_direction);
    let inverse = machine::run(
        &engine,
        Phase::Inverse,
        target,
        &mut inverse_meter,
        &mut stats,
    )
    .and_then(|out| engine.encode(&out, &mut inverse_meter, &mut stats));
    let (reconstruction, inverse_state, reconstruction_state, compression) = match inverse {
        Ok(bytes) => {
            let matched = bytes == semantic_bytes;
            (
                Some(bytes),
                ExecutionState::Completed,
                if matched {
                    ReconstructionState::Matched
                } else {
                    ReconstructionState::Mismatched
                },
                if matched {
                    CompressionEligibility::Pending
                } else {
                    CompressionEligibility::Ineligible
                },
            )
        }
        Err(CostError::FuelExhausted) => (
            None,
            ExecutionState::FuelExhausted,
            ReconstructionState::Unobserved,
            CompressionEligibility::Pending,
        ),
        Err(CostError::InvalidProgram) | Err(CostError::ResourceExhausted) => (
            None,
            ExecutionState::Failed,
            ReconstructionState::Unobserved,
            CompressionEligibility::Ineligible,
        ),
        Err(e) => return Err(e),
    };
    stats.codec_fuel = codec_meter.used;
    stats.adapter_fuel = adapter_meter.used;
    stats.projection_fuel = projection_meter.used;
    stats.inverse_fuel = inverse_meter.used;
    stats.total()?;
    let mut h = Sha256::new();
    h.update(b"CopyPackTree-v1/actual-executable-bundle");
    h.update(profile.digest.as_bytes());
    h.update(&bundle);
    h.update(&semantic_bytes);
    h.update(&declaration_bytes);
    let report = CostObservation {
        baseline_bits: None,
        basis: InputBasis::Given,
        independence: IndependenceStatus::SharedDependency,
        native_gap: Some(CostGap::NativeCanonicalizationCostClosurePending),
        evidence_gaps: Vec::new(),
        binding: lc631_substrate::Digest32::from_bytes(h.finalize().into()),
        profile_digest: profile.digest,
        bits,
        bundle,
        semantic: semantic_bytes,
        representation,
        reconstruction,
        inverse_state,
        reconstruction_state,
        compression,
        stats,
        declarations: declaration_bytes,
        dictionary: dictionary_bytes,
    };
    // Retain actual immutable report bytes before releasing execution scratch.
    let retained = claim(view, report.retained_bytes()?)?;
    profile.check_current(view)?;
    retained.check()?;
    workspace.claim.check()?;
    drop(engine);
    drop(package);
    drop(fields);
    drop(declarations);
    drop(workspace);
    Ok(OwnedExecutableBundleCost {
        report,
        retained,
        limits,
        ranges,
    })
}
fn bundle(
    t: &[u8],
    p: InlinePackage<'_>,
    l: CostLimits,
    m: &mut Meter<'_>,
) -> Result<(Vec<u8>, [std::ops::Range<usize>; 7]), CostError> {
    let a = term::encode(&term::app("bundle-v1", Vec::new()), l)?;
    let d = term::encode(&term::app("decoder-v1", Vec::new()), l)?;
    let parts = [
        t,
        p.adapter,
        p.projection,
        p.inverse,
        p.helpers,
        p.runtime,
        p.dictionary,
    ];
    let length = parts.iter().try_fold((a.len() + d.len()) as u64, |n, p| {
        n.checked_add(p.len() as u64)
            .ok_or(CostError::ResourceExhausted)
    })?;
    if length > l.bundle_bytes {
        return Err(CostError::ResourceExhausted);
    }
    let mut out = Vec::new();
    out.try_reserve_exact(length as usize)
        .map_err(|_| CostError::ResourceExhausted)?;
    extend(&mut out, &a[..a.len() - 1], m)?;
    m.debit(1)?;
    out.push(3);
    let mut ranges: [std::ops::Range<usize>; 7] = std::array::from_fn(|_| 0..0);
    for (i, p) in parts.iter().enumerate() {
        if i == 1 {
            extend(&mut out, &d[..d.len() - 1], m)?;
            m.debit(1)?;
            out.push(5)
        }
        let start = out.len();
        extend(&mut out, p, m)?;
        ranges[i] = start..out.len();
    }
    // The constructed bytes are checked as one aggregate: depth/nodes cannot
    // hide in separately valid children. This actual pass consumes P fuel.
    codec::scan(&out, l, m)?;
    Ok((out, ranges))
}
fn extend(out: &mut Vec<u8>, bytes: &[u8], m: &mut Meter<'_>) -> Result<(), CostError> {
    for part in bytes.chunks(1024) {
        m.debit(part.len() as u64)?;
        out.extend_from_slice(part);
        m.resources.check()?
    }
    Ok(())
}
fn copy_bytes(bytes: &[u8], m: &mut Meter<'_>) -> Result<Vec<u8>, CostError> {
    let mut out = Vec::new();
    out.try_reserve_exact(bytes.len())
        .map_err(|_| CostError::ResourceExhausted)?;
    extend(&mut out, bytes, m)?;
    Ok(out)
}
