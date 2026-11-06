//! 日本語: current viewは正確なP2 reportと新しいnative検査をともに保持する。
//! English: The current view keeps both exact P2 reports and the new native checks alive.
use super::*;

/// 日本語: report・JSONは実ownerを生成せず、内部fieldは非公開でcurrent借用は寿命内に限る。
/// English: Reports/JSON do not construct this actual owner; fields stay private and current borrows stay within their scope.
/// ```compile_fail,E0599
/// use lc631_runtime::context_novelty_measurement::framework::novelty::semantic::Owned;
/// let _ = Owned::from_report(());
/// ```
/// ```compile_fail,E0277
/// use lc631_runtime::context_novelty_measurement::framework::novelty::semantic::Owned;
/// fn inject<'s, 'a, 'v>() {
///     let _: Owned<'s, 'a, 'v> = serde_json::from_str("{}").unwrap();
/// }
/// ```
/// ```compile_fail,E0616
/// use lc631_runtime::context_novelty_measurement::framework::novelty::semantic::Owned;
/// fn inspect(owner: &Owned<'_, '_, '_>) {
///     let _ = &owner.reports;
/// }
/// ```
/// ```compile_fail
/// use lc631_runtime::context_novelty_measurement::framework::novelty::semantic::{ExpectedPair, Owned};
/// use lc631_runtime::resource::ResourceReservationView;
/// use lc631_analysis::creativity_measurement::{ComponentMetric, ComponentReport};
/// fn escape<'s, 'a, 'v>(
///     owner: &Owned<'s, 'a, 'v>,
///     expected: ExpectedPair<'s, 'a, 'v>,
///     view: &ResourceReservationView<'_>,
/// ) -> &'static ComponentReport {
///     owner.with_current(expected, view, |current| {
///         current.report(ComponentMetric::NoveltyCompatibility).unwrap()
///     }).unwrap()
/// }
/// ```
pub struct Owned<'s, 'a, 'v> {
    root: &'s Root<'a, 'v>,
    expected: ExpectedPair<'s, 'a, 'v>,
    parent: m::Owned<'s, 'a, 'v>,
    reports: [ComponentReport; 3],
    bank: bank::Bank<'v>,
    bridges: Vec<BridgeResult>,
    semantic_total: Rational,
    bridge_total: Rational,
    status: Status,
    checks: Cell<usize>,
    retained: ResourceClaim<'v>,
}

pub struct Current<'scope> {
    parent: m::Current<'scope>,
    reports: &'scope [ComponentReport; 3],
    source_queries: &'scope [Query],
    target_queries: &'scope [Query],
    source: &'scope f::SmtStatusQuery,
    target: &'scope f::SmtStatusQuery,
    bridges: &'scope [BridgeResult],
    semantic_total: &'scope Rational,
    bridge_total: &'scope Rational,
    status: Status,
    counts: Counts,
}

impl<'a, 'v> Root<'a, 'v> {
    pub fn measure_current<'s>(
        &'s self,
        expected: ExpectedPair<'s, 'a, 'v>,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<Owned<'s, 'a, 'v>, Error> {
        m::budget::with_call(&self.parent.framework, || {
            self.own_current(view)?;
            self.expected(expected)?;
            if self.attempts.get() >= self.registration.limits.max_assessments {
                return Err(Error::AssessmentBudget);
            }
            self.attempts.set(self.attempts.get() + 1);
            m::budget::debit(&self.parent.framework, 1024)?;
            let limits = self.registration.limits;
            let retained =
                m::budget::claim(view, limits.assessment_ram, Some(limits.output_bytes))?;
            let scratch = m::budget::claim(view, limits.scratch_ram, None)?;
            // 日本語: P2は、この返却された実owner上のcase検査を完了している。
            // English: P2 finishes with actual case checks on this exact, freshly returned owner.
            let parent = self.parent.measure_current(expected.pair, view)?;
            let check = || self.parent.framework.native.check(view);
            let mut work = n::Work::new(
                &self.parent.framework.native.meter,
                self.parent
                    .framework
                    .native
                    .state
                    .declaration
                    .limits
                    .max_call_work,
                &check,
            );
            let bank = bank::Bank::evaluate(self, expected.pair, view, &mut work)?;
            let dpar = parent
                .reports
                .iter()
                .find(|report| report.metric == ComponentMetric::FrameworkDistance)
                .ok_or(Error::ExpectedInputMismatch)?;
            let values = metrics::evaluate(self, expected.pair, dpar, &bank, &mut work)?;
            let digest = identity::result(
                self,
                expected.pair,
                &parent.reports,
                &bank,
                &values,
                &mut work,
            )?;
            let reports = identity::reports(self, dpar, digest, values.states, &mut work)?;
            bank.check(expected.pair, view, &mut work)?;
            // 日本語: 公開時にP2の保持されたnative/archive実体を一度再検査する。
            // English: Validate P2's retained native/archive backing once at publication.
            parent.check(expected.pair, view)?;
            scratch.check().map_err(Error::Resource)?;
            m::budget::current(
                &retained,
                view,
                self.parent.framework.native.state.source.id().digest(),
            )?;
            Ok(Owned {
                root: self,
                expected,
                parent,
                reports,
                bank,
                bridges: values.bridges,
                semantic_total: values.semantic_total,
                bridge_total: values.bridge_total,
                status: values.status,
                checks: Cell::new(0),
                retained,
            })
        })
    }
}

impl<'s, 'a, 'v> Owned<'s, 'a, 'v> {
    fn guard(
        &self,
        expected: ExpectedPair<'_, 'a, 'v>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        self.root.own_current(view)?;
        let next = self
            .checks
            .get()
            .checked_add(1)
            .ok_or(Error::CurrentBudget)?;
        if next > self.root.registration.limits.max_current_checks {
            return Err(Error::CurrentBudget);
        }
        self.checks.set(next);
        self.root.expected(expected)?;
        if !m::plan::same_case(expected.pair.source, self.expected.pair.source)
            || !m::plan::same_case(expected.pair.target, self.expected.pair.target)
        {
            return Err(Error::ExpectedInputMismatch);
        }
        let native = &self.root.parent.framework.native;
        m::budget::current(&self.retained, view, native.state.source.id().digest())?;
        let check = || native.check(view);
        let mut work = n::Work::new(
            &native.meter,
            native.state.declaration.limits.max_call_work,
            &check,
        );
        self.bank.check(expected.pair, view, &mut work)
    }

    /// 日本語: 一つのP2 current借用でconsumer全体の前後に両caseを検査する。
    /// 取消し時は返却値を破棄し、callbackの副作用はrollbackしない。
    /// English: One P2 current borrow checks both cases before and after the whole consumer.
    /// Cancellation discards the returned value. Callback effects are not rolled back.
    pub fn with_current<R>(
        &self,
        expected: ExpectedPair<'_, 'a, 'v>,
        view: &ResourceReservationView<'_>,
        consume: impl for<'scope> FnOnce(Current<'scope>) -> R,
    ) -> Result<R, Error> {
        m::budget::with_call(&self.root.parent.framework, || {
            self.guard(expected, view)?;
            self.parent
                .with_current(expected.pair, view, |parent| {
                    let value = consume(Current {
                        parent,
                        reports: &self.reports,
                        source_queries: self.bank.queries(Side::Source),
                        target_queries: self.bank.queries(Side::Target),
                        source: expected.pair.source.expected.subject_query,
                        target: expected.pair.target.expected.subject_query,
                        bridges: &self.bridges,
                        semantic_total: &self.semantic_total,
                        bridge_total: &self.bridge_total,
                        status: self.status,
                        counts: self.bank.counts,
                    });
                    self.guard(expected, view)?;
                    Ok(value)
                })
                .map_err(Error::from)?
        })
    }
}

impl<'scope> Current<'scope> {
    pub fn report(&self, metric: ComponentMetric) -> Option<&'scope ComponentReport> {
        self.parent
            .report(metric)
            .or_else(|| self.reports.iter().find(|r| r.metric == metric))
    }
    pub fn tnc(&self) -> Result<&'scope RationalInterval, Error> {
        match self
            .report(ComponentMetric::NoveltyCompatibility)
            .map(|report| &report.state)
        {
            Some(ComponentState::Measured(value)) => Ok(value),
            _ => Err(Error::NoScalar),
        }
    }
    pub fn query(&self, side: Side, role: f::QueryRole) -> Option<&'scope Query> {
        let queries = match side {
            Side::Source => self.source_queries,
            Side::Target => self.target_queries,
        };
        queries.get(role as usize)
    }
    pub fn bridge_results(&self) -> &'scope [BridgeResult] {
        self.bridges
    }
    pub fn source_query(&self) -> &'scope f::SmtStatusQuery {
        self.source
    }
    pub fn target_query(&self) -> &'scope f::SmtStatusQuery {
        self.target
    }
    pub fn semantic_denominator(&self) -> &'scope Rational {
        self.semantic_total
    }
    pub fn bridge_denominator(&self) -> &'scope Rational {
        self.bridge_total
    }
    pub fn counts(&self) -> Counts {
        self.counts
    }
    pub fn status(&self) -> Status {
        self.status
    }
}
