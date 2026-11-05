//! Target-only inverse has no source, plan, owner or semantic control port.
use super::machine::{Engine, Meter, Phase, Resources, Work};
use super::*;
use lc631_substrate::resource::{ResourceClaim, ResourceReservationView};
#[derive(Clone, Copy)]
pub struct FixedBootstrap(());
impl FixedBootstrap {
    pub fn k6_tree_v1() -> Self {
        Self(())
    }
}
pub struct ExecutionControls<'a> {
    resources: &'a dyn Resources,
    fuel: u64,
}
impl<'a> ExecutionControls<'a> {
    pub fn from_view(view: &'a ResourceReservationView<'_>, fuel: u64) -> Result<Self, CostError> {
        view.check()?;
        if fuel > CostLimits::default().fuel_per_direction {
            return Err(CostError::UnsupportedProfile);
        }
        Ok(Self {
            resources: view,
            fuel,
        })
    }
}
#[derive(Clone, Copy)]
pub struct TargetBundleView<'b> {
    parts: [&'b [u8]; 7],
    limits: CostLimits,
    codec_fuel: u64,
}
impl<'b> TargetBundleView<'b> {
    pub fn decode(bytes: &'b [u8], profile: &DeclaredCostProfile<'_>) -> Result<Self, CostError> {
        profile.check_alive()?;
        let mut work = Admission { profile, used: 0 };
        let ranges = extract(bytes, profile.limits, &mut work)?;
        let parts = std::array::from_fn(|i| &bytes[ranges[i].clone()]);
        if parts[5] != profile.runtime_bytes {
            return Err(CostError::ProfileMismatch);
        }
        profile.check_alive()?;
        Ok(Self {
            parts,
            limits: profile.limits,
            codec_fuel: work.used,
        })
    }
    pub(super) fn observed(
        bytes: &'b [u8],
        limits: CostLimits,
        ranges: &[std::ops::Range<usize>; 7],
    ) -> Self {
        Self {
            parts: std::array::from_fn(|i| &bytes[ranges[i].clone()]),
            limits,
            codec_fuel: 0,
        }
    }
}
struct Admission<'a, 'v> {
    profile: &'a DeclaredCostProfile<'v>,
    used: u64,
}
impl Work for Admission<'_, '_> {
    fn debit_work(&mut self, n: u64) -> Result<(), CostError> {
        let mut left = n;
        while left > 0 {
            self.profile.check_alive()?;
            let c = left.min(1024);
            self.used = self.used.checked_add(c).ok_or(CostError::InvalidProgram)?;
            if self.used > self.profile.limits.fuel_per_direction {
                return Err(CostError::FuelExhausted);
            }
            left -= c;
        }
        Ok(())
    }
}
fn extract(
    bytes: &[u8],
    limits: CostLimits,
    work: &mut dyn Work,
) -> Result<[std::ops::Range<usize>; 7], CostError> {
    // Establish canonical encoding before package-shape checks can return
    // InvalidProgram. This real allocation-free pass uses the same admission
    // meter; malformed bytes never become a merely wrong program shape.
    codec::scan(bytes, limits, work)?;
    let mut s = codec::Scan::new(bytes, limits, work)?;
    s.header("bundle-v1", 3, 1)?;
    let mut ranges = std::array::from_fn(|_| 0..0);
    ranges[0] = s.span(2)?;
    s.header("decoder-v1", 5, 2)?;
    for r in &mut ranges[1..6] {
        *r = s.span(3)?
    }
    ranges[6] = s.span(2)?;
    if s.at != bytes.len() {
        return Err(CostError::InvalidCanonicalEncoding);
    }
    Ok(ranges)
}
pub struct IsolatedReconstruction<'a> {
    bytes: Option<Vec<u8>>,
    state: ExecutionState,
    stats: ExecutionStats,
    _retained: ResourceClaim<'a>,
}
impl IsolatedReconstruction<'_> {
    pub fn bytes(&self) -> Option<&[u8]> {
        self.bytes.as_deref()
    }
    pub fn state(&self) -> ExecutionState {
        self.state
    }
    pub fn stats(&self) -> &ExecutionStats {
        &self.stats
    }
}
pub fn execute_inverse<'a>(
    view: TargetBundleView<'_>,
    _boot: FixedBootstrap,
    controls: ExecutionControls<'a>,
) -> Result<IsolatedReconstruction<'a>, CostError> {
    controls.resources.check()?;
    if view.limits.additional_ram_bytes < CostLimits::default().additional_ram_bytes {
        return Err(CostError::ResourceExhausted);
    }
    let scratch = controls.resources.claim(view.limits.additional_ram_bytes)?;
    let mut codec_meter = Meter::new(controls.resources, view.limits.fuel_per_direction);
    codec_meter.debit(view.codec_fuel)?;
    let p = view.parts;
    let other_host = (p[0].len() as u64)
        .checked_mul(128)
        .and_then(|v| v.checked_add(1024 * 1024))
        .ok_or(CostError::ResourceExhausted)?;
    let package = codec::Package::parse(
        InlinePackage {
            adapter: p[1],
            projection: p[2],
            inverse: p[3],
            helpers: p[4],
            runtime: p[5],
            dictionary: p[6],
        },
        view.limits,
        &mut codec_meter,
        view.limits.additional_ram_bytes / 4 * 3,
        other_host,
    )?;
    let n = codec::scan(p[0], view.limits, &mut codec_meter)?;
    codec_meter.chunks(2 * (p[0].len() as u64 + n))?;
    let target = term::decode(p[0], view.limits)?;
    codec::forbidden_metered(&target, &mut codec_meter)?;
    let mut stats = ExecutionStats::default();
    let engine = Engine::new(&package, view.limits, &mut codec_meter, &mut stats)?;
    let input = engine.input(&target, &mut codec_meter, &mut stats)?;
    let mut inverse_meter = Meter::new(
        controls.resources,
        controls.fuel.min(view.limits.fuel_per_direction),
    );
    let result = machine::run(
        &engine,
        Phase::Inverse,
        input,
        &mut inverse_meter,
        &mut stats,
    )
    .and_then(|n| engine.encode(&n, &mut inverse_meter, &mut stats));
    let (bytes, state) = match result {
        Ok(bytes) => (Some(bytes), ExecutionState::Completed),
        Err(CostError::FuelExhausted) => (None, ExecutionState::FuelExhausted),
        Err(CostError::InvalidProgram) | Err(CostError::ResourceExhausted) => {
            (None, ExecutionState::Failed)
        }
        Err(e) => return Err(e),
    };
    stats.codec_fuel = codec_meter.used;
    stats.inverse_fuel = inverse_meter.used;
    stats.total()?;
    let retained = controls
        .resources
        .claim(bytes.as_ref().map_or(4096, |b| b.len() as u64 + 4096))?;
    scratch.check()?;
    controls.resources.check()?;
    drop(engine);
    drop(package);
    drop(scratch);
    Ok(IsolatedReconstruction {
        bytes,
        state,
        stats,
        _retained: retained,
    })
}
