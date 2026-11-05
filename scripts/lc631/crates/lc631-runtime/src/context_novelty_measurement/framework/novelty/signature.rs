//! Canonical role payloads derived from retained ACTUAL P1 outputs,not gold labels.
use super::*;
pub(super) struct Signature {
    pub universe: n::Universe,
    pub packages: [usize; 3],
    pub role_counts: [[u16; 10]; 2],
    pub positive_rows: usize,
}
pub(super) fn build(
    root: &f::FrameworkRoot<'_, '_>,
    l: Limits,
    w: &mut Work<'_>,
) -> Result<Signature, Error> {
    w.charge(100_000).map_err(native)?; // bounded960record extraction/map/sort before allocation
    if root.capability.point_recipe_values.len() != 15
        || root.capability.history.records.len() != 960
        || root.capability.point != [1, 2, 4, 8]
        || root.capability.chain != [1, 3, 9, 27]
    {
        return Err(Error::Framework(f::Error::ProfileContractMismatch));
    }
    let mut point = budget::vector(15, 15)?;
    point.extend_from_slice(&root.capability.point_recipe_values);
    let mut chain = budget::vector(40, 40)?;
    for model in 0..40 {
        let mut row = [0u64; 24];
        let mut seen = [false; 24];
        for r in &root.capability.history.records {
            if r.model == model {
                let n = family(model, true)?;
                let recipe = r.formula.checked_sub(n * 24).ok_or(Error::InvalidMap)?;
                if recipe >= 24 || seen[recipe] {
                    return Err(Error::InvalidMap);
                }
                seen[recipe] = true;
                row[recipe] = r.bits;
            }
        }
        if seen.iter().any(|v| !*v) {
            return Err(Error::InvalidMap);
        }
        chain.push(row);
    }
    let mut role_counts = [[0u16; 10]; 2];
    for role in root.declaration.partition {
        let k = role.kernel as usize;
        let i = role.role as usize;
        role_counts[k][i] = role_counts[k][i].checked_add(1).ok_or(Error::InvalidMap)?;
    }
    if role_counts != [[1; 10]; 2] {
        return Err(Error::InvalidMap);
    }
    let common = common(&point, &chain)?;
    let point_payload = closure(&point, false)?;
    let chain_payload = closure(&chain, true)?;
    let payloads = [common, point_payload, chain_payload];
    let mut features = budget::vector(3, 3)?;
    for (i, payload) in payloads.into_iter().enumerate() {
        if payload.is_empty() || payload.len() > l.max_feature_bytes {
            return Err(Error::PayloadBudget);
        }
        let package = [f::Package::C, f::Package::Kc, f::Package::Ki][i];
        let weight = root
            .declaration
            .universe
            .iter()
            .find(|v| v.package == package)
            .ok_or(Error::InvalidMap)?
            .weight;
        budget::weight(weight, l, w)?;
        features.push(n::CanonicalFeature {
            payload,
            weight: weight.clone(),
        });
    }
    // Package indices are joins only. K3 identity is the complete byte payload.
    for i in 1..features.len() {
        let mut j = i;
        while j > 0 && features[j].payload < features[j - 1].payload {
            features.swap(j, j - 1);
            j -= 1;
        }
    }
    if features.windows(2).any(|p| p[0].payload == p[1].payload) {
        return Err(Error::DuplicateFeature);
    }
    let packages = [0, 1, 2]; // common0xc0,closures0xc1;single-point world1 precedes chainworld2
    if features[0].payload.first() != Some(&0xc0)
        || features[1].payload.get(7) != Some(&1)
        || features[2].payload.get(7) != Some(&2)
    {
        return Err(Error::InvalidMap);
    }
    Ok(Signature {
        universe: n::Universe { features },
        packages,
        role_counts,
        positive_rows: 8,
    })
}
fn family(ordinal: usize, chain: bool) -> Result<usize, Error> {
    let mut start = 0;
    for n in 0..=3 {
        let count = if chain {
            3usize.pow(n as u32)
        } else {
            2usize.pow(n as u32)
        };
        if ordinal < start + count {
            return Ok(n);
        }
        start += count;
    }
    Err(Error::InvalidMap)
}
fn common(point: &[[u64; 24]], chain: &[[u64; 24]]) -> Result<Vec<u8>, Error> {
    let collect =
        |rows: &[[u64; 24]], start: usize, worlds: usize| -> Result<[[u8; 7]; 8], Error> {
            let mut out = [[0u8; 7]; 8];
            let mut seen = [false; 8];
            for row in &rows[start..] {
                for world in 0..worlds {
                    let mut tuple = [0u8; 7];
                    for (i, recipe) in [0, 1, 2, 3, 4, 6, 7].into_iter().enumerate() {
                        tuple[i] = u8::try_from((row[recipe] >> world) & 1)
                            .map_err(|_| Error::InvalidMap)?;
                    }
                    let key = usize::from((tuple[0] << 2) | (tuple[1] << 1) | tuple[2]);
                    if tuple[3] != 0 || tuple[4] != 1 || (seen[key] && out[key] != tuple) {
                        return Err(Error::Framework(f::Error::ProfileContractMismatch));
                    }
                    out[key] = tuple;
                    seen[key] = true;
                }
            }
            if seen.iter().any(|v| !*v) {
                return Err(Error::InvalidMap);
            }
            Ok(out)
        };
    let p = collect(point, 7, 1)?;
    let i = collect(chain, 13, 2)?;
    if p != i {
        return Err(Error::Framework(f::Error::ProfileContractMismatch));
    }
    let mut out = budget::vector(61, 4096)?;
    out.extend_from_slice(&[0xc0, 1, 0x3f, 0, 8]);
    for row in p {
        out.extend_from_slice(&row);
    }
    Ok(out)
}
fn closure(rows: &[[u64; 24]], chain: bool) -> Result<Vec<u8>, Error> {
    let size = if chain { 296 } else { 124 };
    let mut out = budget::vector(size, 4096)?;
    // Same header for BOTH closures. Actual domains/tables cause the difference.
    out.extend_from_slice(&[0xc1, 1, 0xc0, 3, 0, 4]);
    let mut cursor = 0;
    for n in 0..=3 {
        let radix = if chain { 3 } else { 2 };
        let count = (radix as usize).pow(n as u32);
        out.extend_from_slice(&[
            n as u8,
            if chain { 2 } else { 1 },
            radix,
            if chain { 2 } else { 1 },
        ]);
        out.extend_from_slice(if chain { &[3, 2] } else { &[1] });
        out.push(count as u8);
        for ordinal in 0..count {
            let states = f::native::states(ordinal, n, radix).map_err(framework)?;
            out.extend_from_slice(&states);
            let row = rows.get(cursor).ok_or(Error::InvalidMap)?;
            for recipe in [5, 20, 21, 22] {
                let value = u8::try_from(row[recipe]).map_err(|_| Error::InvalidMap)?;
                if value > (if chain { 3 } else { 1 }) {
                    return Err(Error::InvalidMap);
                }
                out.push(value);
            }
            cursor += 1;
        }
    }
    if cursor != rows.len() || out.len() != size {
        return Err(Error::InvalidMap);
    }
    Ok(out)
}
pub(super) fn set(s: &Signature, kernel: f::Kernel) -> Result<n::FeatureSet, Error> {
    let mut out = budget::vector(2, 2)?;
    out.push(s.packages[0]);
    out.push(
        s.packages[if kernel == f::Kernel::ClassicalPointV1 {
            1
        } else {
            2
        }],
    );
    Ok(out)
}
