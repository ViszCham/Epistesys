//! Stage B templates are serialized generic Expr code, including native lowering.
//! This producer receives only an explicit Given dictionary, never native owners.
mod boolean;
mod cases;
mod dsl;
mod inventory;
mod tuple;
use super::{
    term::{self, app},
    CostError, CostLimits, OwnedInlinePackage, Term,
};
use dsl::*;

pub fn stage_b_copy_pack_package_v1(dictionary: &[u8]) -> Result<OwnedInlinePackage, CostError> {
    let limits = CostLimits::default();
    let d = term::decode(dictionary, limits)?;
    let sections = term::children(&d, "dictionary-v1")?;
    if sections.len() != 4 {
        return Err(CostError::InvalidProgram);
    }
    let declarations = term::children(&sections[0], "declarations-v1")?;
    // Native finite profile: <=2*(spec+reference+4 atoms)+4 time+6 roles.
    if declarations.len() > 32 {
        return Err(CostError::UnsupportedProfile);
    }
    let (case_rows, time_rows) = indices(declarations)?;
    let mut h = inventory::helpers(declarations.len());
    h.extend(boolean::helpers());
    h.extend(cases::helpers(&case_rows));
    h.extend(tuple::helpers(&time_rows));
    let product = mk(
        "tlcx-finite-product-v1",
        vec![
            call("cases", vec![get(get(a(0), 0), 0), get(a(0), 2)]),
            call("temporal", vec![get(get(a(0), 0), 1), get(a(0), 2)]),
            call("spatial", vec![get(get(a(0), 0), 2), get(a(0), 2)]),
        ],
    );
    h.push(function(
        "product",
        2,
        shape(
            a(0),
            "field-stream-v1",
            3,
            req(
                call("check-declarations", vec![get(a(0), 1)]),
                shape(get(a(0), 0), "native-fields-v1", 3, product),
            ),
        ),
    ));
    h.sort_by(|a, b| helper_name(a).cmp(helper_name(b)));
    let helpers = app("helpers-v1", h);
    let adapter = program("adapter", run("native", vec![a(0), a(1)]));
    let projection = program("projection", run("copy", vec![a(0)]));
    let inverse = program("inverse", run("copy", vec![a(0)]));
    let runtime = inventory::runtime();
    let terms = [&adapter, &projection, &inverse, &helpers, &runtime];
    let count = terms.iter().try_fold(0u64, |n, t| {
        n.checked_add(instructions(t)?)
            .ok_or(CostError::ResourceExhausted)
    })?;
    if count > limits.instructions {
        return Err(CostError::ResourceExhausted);
    }
    Ok(OwnedInlinePackage {
        adapter: term::encode(&adapter, limits)?,
        projection: term::encode(&projection, limits)?,
        inverse: term::encode(&inverse, limits)?,
        helpers: term::encode(&helpers, limits)?,
        runtime: term::encode(&runtime, limits)?,
        dictionary: term::copy(dictionary)?,
    })
}

pub(super) fn require_native_adapter_closure(
    package: super::InlinePackage<'_>,
    limits: CostLimits,
    view: &lc631_substrate::resource::ResourceReservationView<'_>,
) -> Result<u64, CostError> {
    // Reuse the declared workspace only after executable evaluation released it.
    let _workspace = super::execution::reserve(view, limits)?;
    let mut meter = super::machine::Meter::new(view, limits.fuel_per_direction);
    // Bound the fixed producer before rebuilding its code; never enlarge caller caps.
    let registration_limits = CostLimits::default();
    meter.chunks(
        registration_limits
            .bundle_bytes
            .checked_add(registration_limits.total_nodes)
            .ok_or(CostError::ResourceExhausted)?,
    )?;
    let gap = || CostError::NeedsEvidence(super::CostGap::NativeCanonicalizationCostClosurePending);
    let registered = match stage_b_copy_pack_package_v1(package.dictionary) {
        Ok(registered) => registered,
        Err(CostError::UnsupportedProfile) => return Err(gap()),
        Err(error) => return Err(error),
    };
    let reference = registered.as_inline();
    meter.chunks(
        (package.adapter.len() as u64)
            .checked_add(reference.adapter.len() as u64)
            .ok_or(CostError::ResourceExhausted)?,
    )?;
    if package.adapter != reference.adapter {
        return Err(gap());
    }
    let encoded = [
        reference.adapter,
        reference.projection,
        reference.inverse,
        reference.helpers,
        reference.runtime,
        reference.dictionary,
    ]
    .iter()
    .try_fold(0u64, |sum, bytes| {
        sum.checked_add(bytes.len() as u64)
            .ok_or(CostError::ResourceExhausted)
    })?;
    let host_budget = limits
        .additional_ram_bytes
        .checked_sub(encoded)
        .ok_or(CostError::ResourceExhausted)?
        / 2;
    // Original P/I were already executed and checked. This gate concerns A -> H/R.
    let actual_input = super::InlinePackage {
        projection: reference.projection,
        inverse: reference.inverse,
        ..package
    };
    let actual = super::codec::Package::parse(actual_input, limits, &mut meter, host_budget, 0)?;
    let expected = super::codec::Package::parse(reference, limits, &mut meter, host_budget, 0)?;
    if !super::machine::matches_registered_closure(&actual, &expected, limits, &mut meter)? {
        return Err(gap());
    }
    view.check()?;
    Ok(meter.used)
}

fn helper_name(t: &Term) -> &str {
    match t {
        Term::Apply(_, v) => match &v[0] {
            Term::Symbol(n) => n,
            _ => "",
        },
        _ => "",
    }
}
// Inspect only explicit dictionary roles. This does not sort or lower native
// cases/atoms/ASTs; those operations are present in the charged template bodies.
type CaseDeclarationIndices = Vec<(usize, usize)>;
type TemporalDeclarationIndices = Vec<usize>;
fn indices(
    rows: &[Term],
) -> Result<(CaseDeclarationIndices, TemporalDeclarationIndices), CostError> {
    let mut cases = Vec::new();
    let mut times = Vec::new();
    for slot in 0..2 {
        let spec = format!("Case/{slot}/Spec");
        let reference = format!("Case/{slot}/Reference");
        let s = find(rows, "CaseSpec", &spec)?;
        let r = find(rows, "CaseReference", &reference)?;
        match (s, r) {
            (Some(s), Some(r)) => cases.push((s, r)),
            (None, None) => {}
            _ => return Err(CostError::InvalidProgram),
        }
    }
    if cases.is_empty() {
        return Err(CostError::UnsupportedProfile);
    }
    for slot in 0..4 {
        match find(rows, "TemporalAtom", &format!("Temporal/Atom/{slot}"))? {
            Some(i) => times.push(i),
            None => break,
        }
    }
    Ok((cases, times))
}
fn find(rows: &[Term], kind: &str, path: &str) -> Result<Option<usize>, CostError> {
    let mut found = None;
    for (i, row) in rows.iter().enumerate() {
        let r = term::children(row, "declaration-v1")?;
        if r.len() != 3 {
            return Err(CostError::InvalidProgram);
        }
        if term::symbol(&r[0])? == kind
            && term::symbol(&r[1])? == path
            && found.replace(i).is_some()
        {
            return Err(CostError::InvalidProgram);
        }
    }
    Ok(found)
}
fn instructions(t: &Term) -> Result<u64, CostError> {
    match t {
        Term::Apply(name, _) if name == "literal" => Ok(1),
        Term::Apply(name, c) => {
            let own = u64::from(matches!(
                name.as_str(),
                "arg"
                    | "child"
                    | "arity"
                    | "operator"
                    | "tag"
                    | "equal"
                    | "less"
                    | "canonical-less"
                    | "symbol-less"
                    | "add"
                    | "subtract"
                    | "multiply"
                    | "if"
                    | "sequence"
                    | "make"
                    | "append"
                    | "dictionary"
                    | "op"
                    | "helper"
                    | "fold"
                    | "require"
                    | "fail"
            ));
            c.iter().try_fold(own, |n, c| {
                n.checked_add(instructions(c)?)
                    .ok_or(CostError::ResourceExhausted)
            })
        }
        _ => Ok(0),
    }
}
