//! 日本語: 同じ実owner/current leaseへ前後で戻る。読取り予算は返金しない。
//! English: Recheck the same actual owners/current lease before and after; reads are never refunded.
use super::*;

pub fn with_native_current<'p, 'i, 'v, R>(
    root: &'p b::BridgeRoot<'i, 'v>,
    program: &b::BridgeProgram<'p, 'i, 'v>,
    view: &ResourceReservationView<'_>,
    consume: impl for<'s> FnOnce(NativeProgramView<'s, 'p, 'i, 'v>) -> R,
) -> Result<R, Error> {
    // Two nested native visits consume at most the packet's conservative256KiB.
    let scratch = resources::claim(view, 128 * 1024, 0).map_err(Error::Resource)?;
    let result = program
        .inspect_composite_current(root, view, |plan, program, input, forward, inverse| {
            consume(NativeProgramView {
                plan,
                program,
                input,
                forward,
                inverse,
            })
        })
        .map_err(from_bridge)?;
    if !resources::current(&scratch, view).map_err(Error::Resource)? {
        return Err(Error::ForeignReservation);
    }
    Ok(result)
}

pub fn capture_current<'g, 'a, 'b, 'o, 'p, 'i, 'v>(
    coordinate: &'g BoolCoordinateSpec<'i>,
    target: BoolTarget<'g>,
    owners: Owners<'g, 'a, 'b, 'o, 'p, 'i, 'v>,
    view: &'v ResourceReservationView<'_>,
) -> Result<OwnedBoolCorrespondence<'g, 'a, 'b, 'o, 'p, 'i, 'v>, Error> {
    let retained = resources::claim(view, 32 * 1024, 0).map_err(Error::Resource)?;
    let _scratch = resources::claim(view, 64 * 1024, 0).map_err(Error::Resource)?;
    with_native_current(owners.root, owners.program, view, |native| {
        payload::validate(coordinate, target, &native, owners.native)?;
        owners
            .gain
            .report_current(owners.root, owners.program, view)
            .map_err(from_bridge)?;
        Ok::<(), Error>(())
    })??;
    let mut without_bridge = [ComponentMetric::DescriptionBits; 19];
    let mut count = 0usize;
    for &metric in required_metrics(OriginalFormula::F2V1) {
        if metric != ComponentMetric::Bridge {
            *without_bridge
                .get_mut(count)
                .ok_or(Error::InvalidDeclaration)? = metric;
            count += 1;
        }
    }
    if !resources::current(&retained, view).map_err(Error::Resource)? {
        return Err(Error::ForeignReservation);
    }
    Ok(OwnedBoolCorrespondence {
        coordinate,
        target,
        owners,
        without_bridge,
        without_bridge_len: count,
        reads: Cell::new(0),
        retained,
    })
}

impl<'g, 'a, 'b, 'o, 'p, 'i, 'v> OwnedBoolCorrespondence<'g, 'a, 'b, 'o, 'p, 'i, 'v> {
    pub fn with_current<R>(
        &self,
        expected_coordinate: &BoolCoordinateSpec<'i>,
        expected_target: BoolTarget<'_>,
        owners: Owners<'g, 'a, 'b, 'o, 'p, 'i, 'v>,
        view: &ResourceReservationView<'_>,
        consume: impl for<'s> FnOnce(CurrentBoolCorrespondence<'s, 'p, 'i, 'v>) -> R,
    ) -> Result<R, Error> {
        self.check_lease(view)?;
        let used = self.reads.get();
        if used >= 32 {
            return Err(Error::ReadBudget);
        }
        self.reads.set(used + 1); // Bounded <=32, debited even if any later check fails.
        self.check_expected(expected_coordinate, owners)?;
        let _scratch = resources::claim(view, 64 * 1024, 0).map_err(Error::Resource)?;
        let result = with_native_current(owners.root, owners.program, view, |native| {
            payload::validate(self.coordinate, self.target, &native, owners.native)?;
            payload::validate(expected_coordinate, expected_target, &native, owners.native)?;
            let report = owners
                .gain
                .report_current(owners.root, owners.program, view)
                .map_err(from_bridge)?;
            let result = consume(CurrentBoolCorrespondence {
                native,
                report,
                without_bridge: &self.without_bridge[..self.without_bridge_len],
            });
            owners
                .gain
                .report_current(owners.root, owners.program, view)
                .map_err(from_bridge)?;
            Ok::<R, Error>(result)
        })??;
        self.check_lease(view)?;
        self.check_expected(expected_coordinate, owners)?;
        Ok(result)
    }
    fn check_lease(&self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        if !resources::current(&self.retained, view).map_err(Error::Resource)? {
            return Err(Error::ForeignReservation);
        }
        Ok(())
    }
    fn check_expected(
        &self,
        coordinate: &BoolCoordinateSpec<'i>,
        owners: Owners<'g, 'a, 'b, 'o, 'p, 'i, 'v>,
    ) -> Result<(), Error> {
        if !std::ptr::eq(coordinate, self.coordinate) {
            return Err(Error::ForeignCoordinate);
        }
        if !std::ptr::eq(owners.root, self.owners.root)
            || !std::ptr::eq(owners.program, self.owners.program)
            || !std::ptr::eq(owners.gain, self.owners.gain)
        {
            return Err(Error::Bridge(b::BridgeError::BindingMismatch));
        }
        if !std::ptr::eq(owners.native.plan, self.owners.native.plan) {
            return Err(Error::ForeignNativePlan);
        }
        if !std::ptr::eq(owners.native.program, self.owners.native.program) {
            return Err(Error::ForeignNativeProgram);
        }
        Ok(())
    }
}
