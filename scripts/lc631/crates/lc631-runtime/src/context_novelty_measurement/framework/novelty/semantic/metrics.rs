//! 日本語: 登録Boolean観測にはK1、原式の三成分T_ncにはK4/K7を使う。
//! 共有kernelの実行前に、入力と中間有理数のサイズを制限する。
//! English: Use K1 for registered Boolean observations and K4/K7 for the original three-leaf T_nc.
//! Bound input and intermediate rational sizes before running the shared kernels.
use super::*;
use lc631_analysis::creativity_measurement::{weighted_assessment, WeightedProbe};
use numeric::{BigInt, NumericBudget, NumericEvaluation};

pub(super) struct Values {
    pub states: [ComponentState; 3],
    pub semantic_total: Rational,
    pub bridge_total: Rational,
    pub bridges: Vec<BridgeResult>,
    pub status: Status,
}
fn zero() -> Rational { Rational::from_integer(BigInt::from(0)) }
fn bits(q: &Rational) -> u64 { q.numer().bits().max(q.denom().bits()) }
fn cap(bits: u64, l: Limits) -> Result<(), Error> {
    if bits > u64::from(l.max_intermediate_bits) { Err(Error::NumericBudget) } else { Ok(()) }
}
pub(super) fn weight(q: &Rational, l: Limits, w: &mut n::Work<'_>) -> Result<(), Error> {
    if bits(q) > u64::from(l.max_weight_bits) { return Err(Error::NumericBudget); }
    cap(bits(q), l)?;
    w.charge(bits(q)*16 + 64).map_err(native)?;
    numeric::check(q).map_err(Error::Numeric)?;
    if q.numer() <= &BigInt::from(0) { return Err(Error::InvalidWeight); }
    Ok(())
}
fn add(a: &Rational, b: &Rational, l: Limits, w: &mut n::Work<'_>) -> Result<Rational, Error> {
    let x = a.numer().bits() + b.denom().bits();
    let y = b.numer().bits() + a.denom().bits();
    cap((x.max(y)+1).max(a.denom().bits()+b.denom().bits()), l)?;
    w.charge((bits(a)+bits(b)+1)*32).map_err(native)?;
    numeric::checked_add(a,b).map_err(Error::Numeric)
}
fn total<'a>(weights: impl Iterator<Item=&'a Rational>, l: Limits, w: &mut n::Work<'_>)
    -> Result<Rational, Error> {
    let mut total = zero();
    for q in weights { total = add(&total, q, l, w)?; }
    Ok(total)
}
fn role_key(role: f::QueryRole) -> &'static [u8] {
    match role { f::QueryRole::GoalEcho => b"goal-echo", f::QueryRole::Placement => b"placement", f::QueryRole::BridgeLaw => b"bridge-law" }
}
fn assess(entries: &[(f::QueryRole, &Rational, bool)], denom: &Rational, l: Limits,
    w: &mut n::Work<'_>) -> Result<ComponentState, Error> {
    if entries.is_empty() { return Ok(ComponentState::Undefined(MeasurementError::EmptyDenominator)); }
    // 日本語: 共有K1 kernelはscale前に正確なunit intervalを構築する。
    // English: The shared K1 kernel constructs exact unit intervals before scaling.
    cap(2, l)?;
    let one = RationalInterval::exact_ratio(1,1).map_err(Error::Numeric)?;
    let nil = RationalInterval::exact_ratio(0,1).map_err(Error::Numeric)?;
    let mut probes = reserve(entries.len(), 3)?;
    let mut successes = zero();
    for &(role, weight, yes) in entries {
        let value = if yes { &one } else { &nil };
        // 日本語: 0のprobeも含め、K1が実行するscale・加算・区間比較を事前検査する。
        // 下で公開するのはK1の結果だけである。
        // English: Preflight the same scale/add/interval comparisons that K1 executes,
        // including zero-valued probes. Only K1's result is published below.
        let v = value.lower();
        cap((v.numer().bits() + weight.numer().bits())
            .max(v.denom().bits() + weight.denom().bits()), l)?;
        w.charge((bits(v) + bits(weight) + 1) * 32).map_err(native)?;
        let scaled = numeric::checked_multiply(v, weight).map_err(Error::Numeric)?;
        cap(scaled.numer().bits() + scaled.denom().bits(), l)?;
        successes = add(&successes, &scaled, l, w)?;
        cap(successes.numer().bits() + successes.denom().bits(), l)?;
        probes.push(WeightedProbe { canonical_payload: role_key(role), weight, value });
    }
    let numerator_bits = successes.numer().bits() + denom.denom().bits();
    let denominator_bits = successes.denom().bits() + denom.numer().bits();
    // 日本語: 共通interval constructorは、同じ有理endpoint同士も比較する。
    // English: The common interval constructors compare equal rational endpoints as well.
    cap(numerator_bits + denominator_bits + 2, l)?;
    w.charge((bits(denom)+bits(&successes)+1)*64 + 2048).map_err(native)?;
    Ok(ComponentState::Measured(weighted_assessment(&probes).map_err(Error::Arithmetic)?))
}
fn multiply(a: &RationalInterval, b: &RationalInterval, l: Limits, w: &mut n::Work<'_>)
    -> Result<RationalInterval, Error> {
    let bits_a = bits(a.lower()).max(bits(a.upper()));
    let bits_b = bits(b.lower()).max(bits(b.upper()));
    let bound = bits_a+bits_b;
    // 日本語: 共有interval kernelはendpointの積も交差乗算で比較する。
    // English: The shared interval kernel also compares its endpoint products by cross multiplication.
    cap(bound*2, l)?;
    w.charge((bound+1)*64).map_err(native)?;
    a.multiply(b).map_err(Error::Numeric)
}
fn sqrt(v: &Rational, l: Limits, w: &mut n::Work<'_>) -> Result<RationalInterval, Error> {
    cap((v.numer().bits()+u64::from(l.precision)*2).max(v.denom().bits()+1), l)?;
    w.charge((bits(v)+u64::from(l.precision)*2+1)*64).map_err(native)?;
    let budget = NumericBudget::new(l.precision).map_err(Error::Numeric)?;
    match numeric::sqrt_enclosure(v,budget).map_err(Error::Numeric)? {
        NumericEvaluation::Complete(bounds) => Ok(bounds),
        NumericEvaluation::ResourceExhausted { .. } => Err(Error::Numeric(numeric::NumericError::ResourceExhausted)),
    }
}
fn tnc(framework: &ComponentState, semantic: &ComponentState, compat: &ComponentState, l: Limits,
    w: &mut n::Work<'_>) -> Result<ComponentState, Error> {
    // 日本語: 既知の0も含め、全primitiveを読み取ってから算術を行う。
    // English: Read every primitive before doing arithmetic, including known zeros.
    let ComponentState::Measured(a) = framework else { return Ok(framework.clone()); };
    let ComponentState::Measured(b) = semantic else { return Ok(semantic.clone()); };
    let ComponentState::Measured(c) = compat else { return Ok(compat.clone()); };
    let product = multiply(a,b,l,w)?;
    let lo = sqrt(product.lower(),l,w)?;
    let hi = sqrt(product.upper(),l,w)?;
    let enclosure = RationalInterval::new(lo.lower().clone(),hi.upper().clone()).map_err(Error::Numeric)?;
    Ok(ComponentState::Measured(multiply(&enclosure,c,l,w)?))
}
pub(super) fn evaluate(root: &Root<'_, '_>, pair: m::ExpectedPair<'_, '_, '_>,
    dpar: &ComponentReport, bank: &bank::Bank<'_>, work: &mut n::Work<'_>) -> Result<Values, Error> {
    let r = root.registration;
    // 日本語: root identityとK1と同じrole順序を使い、resource admissionを
    // 呼出し側の登録到着順序に依存させない。
    // English: Use the fixed role order shared by root identity and K1; resource
    // admission must not depend on the caller's registration arrival order.
    let semantic_total = total(ROLES.into_iter().filter_map(|role|
        r.semantic.iter().find(|p| p.role == role).map(|p| p.weight)), r.limits, work)?;
    let bridge_total = total(ROLES.into_iter().filter_map(|role|
        r.bridges.iter().find(|p| p.role == role).map(|p| p.weight)), r.limits, work)?;
    let missing_data = || ComponentState::ResourceExhausted { partial: None };
    let no_map = || ComponentState::Incomparable { scope: root.digest };
    let empty = || ComponentState::Undefined(MeasurementError::EmptyDenominator);
    let sem = if r.semantic.is_empty() {
        empty()
    } else if r.status_map.is_none() {
        no_map()
    } else if !bank.is_complete() {
        missing_data()
    } else {
        let mut changes = reserve(r.semantic.len(), 3)?;
        for role in ROLES {
            let Some(p) = r.semantic.iter().find(|p| p.role == role) else { continue };
            let source = bank.query(Side::Source, role).ok_or(Error::InvalidProbe)?;
            let target = bank.query(Side::Target, role).ok_or(Error::InvalidProbe)?;
            changes.push((role, p.weight, source.status() != target.status()));
        }
        assess(&changes, &semantic_total, r.limits, work)?
    };
    let mut bridges = reserve(r.bridges.len(), 3)?;
    let compat = if r.bridges.is_empty() {
        empty()
    } else if r.bridge_map.is_none() {
        no_map()
    } else if !bank.is_complete() {
        missing_data()
    } else {
        let mut passes = reserve(r.bridges.len(), 3)?;
        for role in ROLES {
            let Some(&p) = r.bridges.iter().find(|p| p.role == role) else { continue };
            let checked = bridge::inspect(p, pair, work)?;
            passes.push((role, p.weight, checked.passed()));
            bridges.push(checked);
        }
        assess(&passes, &bridge_total, r.limits, work)?
    };
    let status = if r.status_map.is_none() || r.bridge_map.is_none() { Status::NoComparable }
        else if r.semantic.is_empty() || r.bridges.is_empty() { Status::Undefined }
        else if !bank.is_complete() { Status::Partial }
        else { Status::Complete };
    let novelty = tnc(&dpar.state, &sem, &compat, r.limits, work)?;
    Ok(Values { states: [sem, compat, novelty], semantic_total, bridge_total, bridges, status })
}
