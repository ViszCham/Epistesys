//! Checked payload guards are distinct from Copy request construction.
use super::*;
/// Actual private owned measurement;reported bytes never construct this type.
/// ```compile_fail
/// use lc631_runtime::context_novelty_measurement::framework::novelty::Owned;
/// fn inject<'s,'a,'v>(){let _:Owned<'s,'a,'v>=serde_json::from_str("{}").unwrap();}
/// ```
/// ```compile_fail
/// use lc631_runtime::context_novelty_measurement::framework::novelty::Owned;
/// let _=Owned::from_report(());
/// ```
/// ```compile_fail
/// use lc631_runtime::context_novelty_measurement::framework::novelty::Owned;
/// let _=Owned::from_scalar(0);
/// ```
/// ```compile_fail
/// use lc631_runtime::context_novelty_measurement::framework::novelty::Owned;
/// fn clone_owner(o:&Owned<'_,'_,'_>){let _:Owned<'_,'_,'_>=Clone::clone(o);}
/// ```
/// ```compile_fail
/// use lc631_runtime::context_novelty_measurement::framework::novelty::Owned;
/// fn rewrite(o:&mut Owned<'_,'_,'_>){o.reports=[];}
/// ```
/// ```compile_fail
/// use lc631_runtime::context_novelty_measurement::framework::novelty::{Owned,ExpectedPair,Current};
/// use lc631_substrate::resource::ResourceReservationView;
/// fn escape<'s,'a,'v>(o:&Owned<'s,'a,'v>,e:ExpectedPair<'s,'a,'v>,v:&ResourceReservationView<'_>)->Current<'static>{o.with_current(e,v,|c|c).unwrap()}
/// ```
pub struct Owned<'s, 'a, 'v> {
    pub(super) root: &'s Root<'a, 'v>,
    pub(super) expected: ExpectedPair<'s, 'a, 'v>,
    pub(super) reports: [ComponentReport; 2],
    pub(super) union: Rational,
    pub(super) archive: archive::Data<'a, 'v>,
    pub(super) checks: Cell<usize>,
    pub(super) retained: ResourceClaim<'v>,
}
pub struct Current<'scope> {
    reports: &'scope [ComponentReport; 2],
    union: &'scope Rational,
    signature: &'scope signature::Signature,
    known_unions: &'scope [Rational],
    status: ArchiveStatus,
    counts: Counts,
    queries: &'scope [&'scope SmtStatusQuery],
    candidate: &'scope SmtStatusQuery,
    corpus: Digest32,
}
impl<'s, 'a, 'v> Owned<'s, 'a, 'v> {
    fn check(
        &self,
        e: ExpectedPair<'_, 'a, 'v>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        let next = self
            .checks
            .get()
            .checked_add(1)
            .ok_or(Error::CurrentBudget)?;
        if next > self.root.registration.limits.max_current_checks {
            return Err(Error::CurrentBudget);
        }
        self.checks.set(next); // nonrefundable,including mismatches and callback cuts
        self.root.expected(e)?;
        if !plan::same_case(e.source, self.expected.source)
            || !plan::same_case(e.target, self.expected.target)
        {
            return Err(Error::ExpectedInputMismatch);
        }
        budget::current(
            &self.retained,
            view,
            self.root.framework.native.state.source.id().digest(),
        )?;
        self.root.payload_guard(e, view)?;
        let check = || self.root.framework.native.check(view);
        let mut work = Work::new(
            &self.root.framework.native.meter,
            self.root
                .framework
                .native
                .state
                .declaration
                .limits
                .max_call_work,
            &check,
        );
        for held in &self.archive.held {
            held.check(view, &mut work)?;
        }
        Ok(())
    }
    pub fn with_current<R>(
        &self,
        e: ExpectedPair<'_, 'a, 'v>,
        view: &ResourceReservationView<'_>,
        consume: impl for<'scope> FnOnce(Current<'scope>) -> R,
    ) -> Result<R, Error> {
        self.check(e, view)?;
        let result = consume(Current {
            reports: &self.reports,
            union: &self.union,
            signature: &self.root.signature,
            known_unions: &self.archive.unions,
            status: self.archive.status,
            counts: self.archive.counts,
            queries: &self.archive.queries,
            candidate: self.expected.target.expected.subject_query,
            corpus: self.root.registration.corpus_revision,
        });
        self.check(e, view)?;
        Ok(result)
    }
}
impl<'scope> Current<'scope> {
    pub fn report(&self, metric: ComponentMetric) -> Option<&'scope ComponentReport> {
        self.reports.iter().find(|r| r.metric == metric)
    }
    pub fn signature_union_weight(&self) -> &'scope Rational {
        self.union
    }
    pub fn signature_positive_rows(&self) -> usize {
        self.signature.positive_rows
    }
    pub fn signature_role_counts(&self) -> &'scope [[u16; 10]; 2] {
        &self.signature.role_counts
    }
    pub fn signature_payload_lengths(&self) -> [usize; 3] {
        std::array::from_fn(|i| {
            self.signature.universe.features[self.signature.packages[i]]
                .payload
                .len()
        })
    }
    pub fn signature_payload(&self, package: f::Package) -> Option<&'scope [u8]> {
        self.signature
            .universe
            .features
            .get(self.signature.packages[package as usize])
            .map(|p| p.payload.as_slice())
    }
    pub fn knownness_union_weights(&self) -> &'scope [Rational] {
        self.known_unions
    }
    pub fn archive_status(&self) -> ArchiveStatus {
        self.status
    }
    pub fn counts(&self) -> Counts {
        self.counts
    }
    pub fn archive_queries(&self) -> &'scope [&'scope SmtStatusQuery] {
        self.queries
    }
    pub fn candidate_query(&self) -> &'scope SmtStatusQuery {
        self.candidate
    }
    pub fn corpus_revision(&self) -> Digest32 {
        self.corpus
    }
}
