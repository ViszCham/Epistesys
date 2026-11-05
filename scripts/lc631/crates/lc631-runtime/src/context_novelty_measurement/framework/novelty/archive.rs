//! ALL-entry actual max;partial point bounds never imply completed acquisition.
use super::*;
pub(super) struct Data<'a, 'v> {
    pub state: ComponentState,
    pub status: ArchiveStatus,
    pub counts: Counts,
    pub unions: Vec<Rational>,
    pub held: Vec<archive_native::Held<'a, 'v>>,
    pub queries: Vec<&'a SmtStatusQuery>,
}
pub(super) fn measure<'a, 'v>(
    root: &Root<'a, 'v>,
    candidate: &n::FeatureSet,
    view: &'v ResourceReservationView<'_>,
    w: &mut Work<'_>,
) -> Result<Data<'a, 'v>, Error> {
    let input = root.registration.archive;
    let entries = match input {
        ArchiveInput::Read(entries) => entries,
        ArchiveInput::Unread | ArchiveInput::Unavailable => &[],
    };
    let mut data = Data {
        state: ComponentState::Unknown(MeasurementGap::MissingObservation),
        status: ArchiveStatus::Unread,
        counts: Counts {
            archive_entries_seen: 0,
            archive_comparisons: 0,
            native_archive_dispatches: 0,
            native_archive_store_entries: 0,
        },
        unions: budget::vector(entries.len(), 64)?,
        held: budget::vector(entries.len(), 64)?,
        queries: budget::vector(entries.len(), 64)?,
    };
    for e in entries {
        data.queries.push(e.query);
    }
    match input {
        ArchiveInput::Unread => return Ok(data),
        ArchiveInput::Unavailable => {
            data.status = ArchiveStatus::Unavailable;
            return Ok(data);
        }
        ArchiveInput::Read(v) if v.is_empty() => {
            data.status = ArchiveStatus::Empty;
            data.state = ComponentState::Undefined(MeasurementError::EmptyCorpus);
            return Ok(data);
        }
        ArchiveInput::Read(_) => {}
    }
    let mut sets = budget::vector(entries.len(), 64)?;
    let mut unsupported = false;
    let mut no_comparable = false;
    for e in entries {
        if root.comparisons.get() == 0 {
            data.status = ArchiveStatus::Partial;
            break;
        }
        data.counts.archive_entries_seen = data
            .counts
            .archive_entries_seen
            .checked_add(1)
            .ok_or(Error::PayloadBudget)?;
        let mut nodes = 0;
        let mut positive = true;
        for expr in e.query.assertions() {
            positive &= construction::positive(expr, w, 1, &mut nodes)?;
        }
        if !positive {
            unsupported = true;
            continue;
        }
        let plan = archive_native::plan(root, e, w)?;
        if plan.calls > root.dispatches.get() {
            data.status = ArchiveStatus::Partial;
            break;
        }
        // Admission is known BEFORE any lowered/native allocation/dispatch.
        root.comparisons.set(
            root.comparisons
                .get()
                .checked_sub(1)
                .ok_or(Error::WorkBudget)?,
        );
        root.dispatches.set(
            root.dispatches
                .get()
                .checked_sub(plan.calls)
                .ok_or(Error::WorkBudget)?,
        );
        let execution = archive_native::execute(root, e, plan, view, w)?;
        data.counts.native_archive_dispatches = data
            .counts
            .native_archive_dispatches
            .checked_add(execution.calls)
            .ok_or(Error::WorkBudget)?;
        data.counts.native_archive_store_entries = data
            .counts
            .native_archive_store_entries
            .checked_add(execution.entries)
            .ok_or(Error::WorkBudget)?;
        data.held.push(execution.held);
        if !execution.comparable {
            no_comparable = true;
            continue;
        }
        let set = construction::set(root, e.query, e.atoms, w)?;
        let denominator = n::numeric::pair(
            &root.constructions,
            candidate,
            &set,
            budget::native_limits(&root.framework, root.registration.limits),
            w,
        )
        .map_err(native)?;
        data.unions.push(denominator);
        sets.push(set);
        data.counts.archive_comparisons += 1;
        // Deliberately NO early return even if a previous exact match gave1.
    }
    // A failed bridge/unsupported entry cannot be erased by another exact match.
    // Retain every funded native artifact before returning a nonnumeric state.
    if no_comparable {
        data.status = ArchiveStatus::NoComparable;
        data.state = ComponentState::Incomparable { scope: root.digest };
        return Ok(data);
    }
    if unsupported {
        data.status = ArchiveStatus::Unsupported;
        data.state = ComponentState::Unknown(MeasurementGap::Unsupported);
        return Ok(data);
    }
    let actual = if sets.is_empty() {
        ratio(0, 1)?
    } else {
        let universe = n::features::kernel_universe(&root.constructions, w).map_err(native)?;
        let candidate = n::features::keys(candidate, &root.constructions, w).map_err(native)?;
        let mut backing = budget::vector(sets.len(), 64)?;
        for set in &sets {
            backing.push(n::features::keys(set, &root.constructions, w).map_err(native)?);
        }
        let mut slices = budget::vector(backing.len(), 64)?;
        for keys in &backing {
            slices.push(keys.as_slice());
        }
        lc631_analysis::creativity_measurement::knownness(&universe, &candidate, &slices)
            .map_err(Error::Arithmetic)?
    };
    if sets.len() == entries.len() {
        data.status = ArchiveStatus::Complete;
        data.state = ComponentState::Measured(actual);
    } else {
        data.status = ArchiveStatus::Partial;
        data.state = ComponentState::ResourceExhausted {
            partial: Some(
                RationalInterval::new(actual.lower().clone(), Rational::from_integer(1.into()))
                    .map_err(Error::Numeric)?,
            ),
        };
    }
    Ok(data)
}
