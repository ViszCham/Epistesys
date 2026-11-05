//! One predeclared semantic-role table shared by source/baseline/target products.
//! Raw IDs bind the separate Given envelope; no raw-ID ordering enters K.
use super::{app, decode, encode, fields, int, integer, sym};
use crate::product_correspondence::{
    copy, reserved, IdentityDeclaration, IdentityKind, OwnedIdentity, PlanData, ProductError,
    ProductLimits,
};
use lc631_solvers::temporal::{ClockSpec, TimeScale};
use lc631_substrate::semantic_term::Term;

pub(crate) fn kind_tag(kind: IdentityKind) -> u8 {
    match kind {
        IdentityKind::Clock => 0,
        IdentityKind::Origin => 1,
        IdentityKind::Reference => 2,
        IdentityKind::Frame => 3,
    }
}
fn kind_name(kind: IdentityKind) -> &'static str {
    match kind {
        IdentityKind::Clock => "Clock",
        IdentityKind::Origin => "Origin",
        IdentityKind::Reference => "Reference",
        IdentityKind::Frame => "Frame",
    }
}
fn path(kind: IdentityKind, role: u16) -> Result<&'static str, ProductError> {
    match (kind, role) {
        (IdentityKind::Clock, 0) => Ok("Clock/primary"),
        (IdentityKind::Clock, 1) => Ok("Clock/secondary"),
        (IdentityKind::Origin, 0) => Ok("Origin/primary"),
        (IdentityKind::Reference, 0) => Ok("Reference/primary"),
        (IdentityKind::Frame, 0) => Ok("Frame/primary"),
        (IdentityKind::Frame, 1) => Ok("Frame/secondary"),
        _ => Err(ProductError::IdentityMismatch),
    }
}
pub(crate) fn preflight(
    inputs: &[IdentityDeclaration<'_>],
    limits: ProductLimits,
) -> Result<(), ProductError> {
    if inputs.len() > 6 {
        return Err(ProductError::UnsupportedProjection);
    }
    let mut bytes = 0usize;
    for input in inputs {
        bytes = bytes
            .checked_add(input.meaning.len())
            .ok_or(ProductError::UnsupportedProjection)?;
        if input.meaning.is_empty() {
            return Err(ProductError::MissingMeaningDeclaration);
        }
        let _ = path(input.kind, input.local_role)?;
    }
    if bytes > limits.max_bytes {
        return Err(ProductError::UnsupportedProjection);
    }
    for kind in [
        IdentityKind::Clock,
        IdentityKind::Origin,
        IdentityKind::Reference,
        IdentityKind::Frame,
    ] {
        if !inputs.iter().any(|i| i.kind == kind && i.local_role == 0) {
            return Err(ProductError::MissingMeaningDeclaration);
        }
    }
    Ok(())
}
pub(crate) fn capture(
    inputs: &[IdentityDeclaration<'_>],
    limits: ProductLimits,
) -> Result<Vec<OwnedIdentity>, ProductError> {
    let mut out: Vec<OwnedIdentity> = reserved(inputs.len())?;
    for input in inputs {
        if out.iter().any(|p| {
            p.kind == input.kind
                && (p.local_role == input.local_role || p.native_id == input.native_id)
        }) {
            return Err(ProductError::IdentityMismatch);
        }
        let meaning = decode(input.meaning, limits)?;
        let f = fields(&meaning, "identity-meaning-v1", 2)?;
        if f[0] != sym(path(input.kind, input.local_role)?) {
            return Err(ProductError::IdentityMismatch);
        }
        match input.kind {
            IdentityKind::Clock => {
                let c = fields(&f[1], "clock-meaning-v1", 3)?;
                if !matches!(&c[0],Term::Symbol(s) if matches!(s.as_str(),"Monotonic"|"Utc"|"Tai"))
                    || integer(&c[1])? != 0
                    || !meaning_symbol(&c[2])
                {
                    return Err(ProductError::UnsupportedProjection);
                }
            }
            IdentityKind::Origin | IdentityKind::Reference => {
                if !meaning_symbol(&f[1]) {
                    return Err(ProductError::UnsupportedProjection);
                }
            }
            IdentityKind::Frame => {
                let f = fields(&f[1], "frame-meaning-v1", 6)?;
                if !matches!(&f[0],Term::Symbol(s) if s == "Pixel" || s == "Millimeter")
                    || !meaning_symbol(&f[1])
                    || !meaning_symbol(&f[2])
                    || !meaning_symbol(&f[3])
                {
                    return Err(ProductError::UnsupportedProjection);
                }
                let _ = i64::try_from(integer(&f[4])?)
                    .map_err(|_| ProductError::UnsupportedProjection)?;
                let _ = i64::try_from(integer(&f[5])?)
                    .map_err(|_| ProductError::UnsupportedProjection)?;
            }
        }
        out.push(OwnedIdentity {
            kind: input.kind,
            local_role: input.local_role,
            native_id: input.native_id,
            meaning_bytes: copy(input.meaning)?,
            meaning,
        });
    }
    // Kind and registered semantic position, never raw ID byte order.
    out.sort_by_key(|i| (i.kind, i.local_role));
    Ok(out)
}
fn meaning_symbol(t: &Term) -> bool {
    matches!(t,Term::Symbol(s) if !s.is_empty() && s.len() <= 256)
}
pub(crate) fn native(
    data: &PlanData,
    kind: IdentityKind,
    id: [u8; 32],
) -> Result<&OwnedIdentity, ProductError> {
    data.identities
        .iter()
        .find(|i| i.kind == kind && i.native_id == id)
        .ok_or(ProductError::MissingMeaningDeclaration)
}
pub(crate) fn role<'a>(
    data: &'a PlanData,
    kind: IdentityKind,
    index: &Term,
) -> Result<&'a OwnedIdentity, ProductError> {
    let index = u16::try_from(integer(index)?).map_err(|_| ProductError::IdentityMismatch)?;
    data.identities
        .iter()
        .find(|i| i.kind == kind && i.local_role == index)
        .ok_or(ProductError::IdentityMismatch)
}
pub(crate) fn reference(
    data: &PlanData,
    kind: IdentityKind,
    id: [u8; 32],
    name: &str,
) -> Result<Term, ProductError> {
    let value = native(data, kind, id)?;
    Ok(app(
        name,
        vec![int(i128::from(value.local_role)), value.meaning.clone()],
    ))
}
pub(crate) fn validate_reference<'a>(
    data: &'a PlanData,
    term: &Term,
    kind: IdentityKind,
    name: &str,
) -> Result<&'a OwnedIdentity, ProductError> {
    let f = fields(term, name, 2)?;
    let value = role(data, kind, &f[0])?;
    if f[1] != value.meaning {
        return Err(ProductError::IdentityMismatch);
    }
    Ok(value)
}
pub(crate) fn clock(data: &PlanData, clock: ClockSpec) -> Result<Term, ProductError> {
    let value = native(data, IdentityKind::Clock, *clock.clock().as_bytes())?;
    let meaning = fields(&value.meaning, "identity-meaning-v1", 2)?;
    let payload = fields(&meaning[1], "clock-meaning-v1", 3)?;
    let scale = match clock.scale() {
        TimeScale::Monotonic => "Monotonic",
        TimeScale::Utc => "Utc",
        TimeScale::Tai => "Tai",
    };
    if payload[0] != sym(scale) || clock.precision_nanoseconds() != 0 {
        return Err(ProductError::UnsupportedProjection);
    }
    Ok(app(
        "clock-v1",
        vec![
            int(i128::from(value.local_role)),
            value.meaning.clone(),
            reference(
                data,
                IdentityKind::Origin,
                *clock.origin().as_bytes(),
                "origin-v1",
            )?,
            reference(
                data,
                IdentityKind::Reference,
                *clock.reference().as_bytes(),
                "reference-v1",
            )?,
        ],
    ))
}
pub(crate) fn validate_clock(data: &PlanData, term: &Term) -> Result<u16, ProductError> {
    let f = fields(term, "clock-v1", 4)?;
    let identity = role(data, IdentityKind::Clock, &f[0])?;
    if f[1] != identity.meaning {
        return Err(ProductError::IdentityMismatch);
    }
    validate_reference(data, &f[2], IdentityKind::Origin, "origin-v1")?;
    validate_reference(data, &f[3], IdentityKind::Reference, "reference-v1")?;
    Ok(identity.local_role)
}
pub(crate) fn frame_unit(identity: &OwnedIdentity) -> Result<&str, ProductError> {
    let f = fields(&identity.meaning, "identity-meaning-v1", 2)?;
    let f = fields(&f[1], "frame-meaning-v1", 6)?;
    match &f[0] {
        Term::Symbol(s) => Ok(s),
        _ => Err(ProductError::UnsupportedProjection),
    }
}
pub(crate) fn envelope(data: &PlanData) -> Result<Vec<u8>, ProductError> {
    let mut entries = reserved(data.identities.len())?;
    for value in &data.identities {
        entries.push(app(
            "native-id-v1",
            vec![
                sym(kind_name(value.kind)),
                int(i128::from(value.local_role)),
                sym(&hex(&value.native_id)),
                value.meaning.clone(),
            ],
        ));
    }
    let mut cases = reserved(data.cases.len())?;
    for (slot, case) in data.cases.iter().enumerate() {
        let mut atoms = reserved(case.native_atoms.len())?;
        for atom in &case.native_atoms {
            let meaning = std::str::from_utf8(&atom.meaning)
                .map_err(|_| ProductError::UnsupportedProjection)?;
            atoms.push(app(
                "native-atom-v1",
                vec![int(i128::from(atom.symbol.get())), sym(meaning)],
            ));
        }
        cases.push(app(
            "native-case-v1",
            vec![
                int(slot as i128),
                sym(&hex(&case.native_key)),
                app("native-atoms-v1", atoms),
            ],
        ));
    }
    encode(
        &app(
            "stage-b-native-identities-v1",
            vec![
                sym(&hex(data.binding.as_bytes())),
                app("identities-v1", entries),
                app("native-cases-v1", cases),
            ],
        ),
        data.limits,
    )
}
fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        text.push(char::from(HEX[usize::from(b >> 4)]));
        text.push(char::from(HEX[usize::from(b & 15)]));
    }
    text
}
