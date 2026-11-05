//! Native corpus capture and ALL-entry max kernel; no success/completeness input.
use super::*;
pub(super) struct Archive {
    pub status: ArchiveStatus,
    pub entries: Vec<FeatureSet>,
}
pub(super) fn capture(
    input: &RootInput<'_>,
    d: &Declaration<'_>,
    u: &Universe,
    w: &mut Work<'_>,
) -> Result<Archive, Error> {
    let entries = match d.archive {
        ArchiveInput::Unread => {
            return Ok(Archive {
                status: ArchiveStatus::Unread,
                entries: Vec::new(),
            })
        }
        ArchiveInput::Unavailable => {
            return Ok(Archive {
                status: ArchiveStatus::Unavailable,
                entries: Vec::new(),
            })
        }
        ArchiveInput::Read(v) => v,
    };
    if entries.len() > d.limits.max_archive_entries {
        return Err(Error::PayloadBudget);
    }
    let mut out = resources::vector(entries.len(), 64, w)?;
    let mut unsupported = false;
    for e in entries {
        w.charge(64)?;
        if !std::ptr::eq(e.source, input.source) || e.context != input.original.state().revision() {
            return Err(Error::ExpectedInputMismatch);
        }
        // Raw typed shape admission includes unsupported operators; no unbounded
        // recursive identity walk occurs after an early Unsupported match.
        identity::query_shape(e.query, d.limits, w)?;
        match validate::query(e.query, d.atoms, d.limits, w) {
            Ok(()) => {
                if e.query.boolean_symbols() != input.source_query.boolean_symbols() {
                    return Err(Error::UnsupportedSignature);
                }
                out.push(features::construction_set(e.query, u, d, w)?);
            }
            Err(Error::UnsupportedSignature) => unsupported = true,
            Err(e) => return Err(e),
        }
    }
    let status = if unsupported {
        ArchiveStatus::Unsupported
    } else if entries.is_empty() {
        ArchiveStatus::Empty
    } else {
        ArchiveStatus::Complete
    };
    Ok(Archive {
        status,
        entries: out,
    })
}
pub(super) fn measure(
    root: &RootPlan<'_, '_>,
    candidate: &FeatureSet,
    w: &mut Work<'_>,
) -> Result<(ComponentState, ArchiveStatus, usize), Error> {
    let s = &root.state;
    let a = &s.archive;
    let l = s.declaration.limits;
    match a.status {
        ArchiveStatus::Unread | ArchiveStatus::Unavailable => {
            return Ok((
                ComponentState::Unknown(MeasurementGap::MissingObservation),
                a.status,
                0,
            ))
        }
        ArchiveStatus::Empty => {
            return Ok((
                ComponentState::Undefined(MeasurementError::EmptyCorpus),
                ArchiveStatus::Empty,
                0,
            ))
        }
        ArchiveStatus::Unsupported => {
            return Ok((
                ComponentState::Unknown(MeasurementGap::Unsupported),
                ArchiveStatus::Unsupported,
                0,
            ))
        }
        ArchiveStatus::Complete => {}
        ArchiveStatus::Partial => return Err(Error::ExpectedInputMismatch),
    }
    let count = a.entries.len().min(l.max_archive_comparisons);
    if count == 0 {
        return Ok((
            ComponentState::ResourceExhausted {
                partial: Some(RationalInterval::from_ratio(0, 1, 1, 1).map_err(arithmetic)?),
            },
            ArchiveStatus::Partial,
            0,
        ));
    }
    for entry in &a.entries[..count] {
        numeric::pair(&s.constructions, candidate, entry, l, w)?;
    }
    let universe = features::kernel_universe(&s.constructions, w)?;
    let candidate = features::keys(candidate, &s.constructions, w)?;
    let mut held = resources::vector(count, 64, w)?;
    for entry in &a.entries[..count] {
        held.push(features::keys(entry, &s.constructions, w)?);
    }
    let mut entries = resources::vector(count, 64, w)?;
    for entry in &held {
        entries.push(entry.as_slice());
    }
    // Existing knownness loops every admitted entry even if its first max is1.
    let actual = lc631_analysis::creativity_measurement::knownness(&universe, &candidate, &entries)
        .map_err(Error::Arithmetic)?;
    if count == a.entries.len() {
        Ok((
            ComponentState::Measured(actual),
            ArchiveStatus::Complete,
            count,
        ))
    } else {
        w.charge(64)?;
        let partial =
            RationalInterval::new(actual.lower().clone(), number(1)).map_err(arithmetic)?;
        Ok((
            ComponentState::ResourceExhausted {
                partial: Some(partial),
            },
            ArchiveStatus::Partial,
            count,
        ))
    }
}
