use super::{pin, Mode};
use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_runtime::{spatial_preservation::*, temporal_preservation::*};
use lc631_solvers::{causal_spatial::{FrameId, Rectangle, SpatialRelation}, temporal::*};
use lc631_substrate::{semantic_term::{self, Term, TermLimits}, Digest32};

pub fn context() -> ContextRevision {
    ContextRevision::original(SourceRevision::of("Stage B Given finite three-part product"))
}
pub fn scope() -> ContextScope { ContextScope::declared(pin(1),pin(2),pin(3),pin(4)) }
pub fn identity(mode: Mode, n: u8) -> [u8;32] {
    [if matches!(mode,Mode::Renamed) {n+100}else{n};32]
}
pub fn clock(mode: Mode, secondary: bool) -> ClockSpec {
    ClockSpec::exact(Digest32::from_bytes(identity(mode,if secondary {44}else{40})),
        TimeScale::Monotonic,Digest32::from_bytes(identity(mode,41)),Digest32::from_bytes(identity(mode,42)))
}
pub fn query() -> TemporalQuery {
    TemporalQuery::checked(context(),vec![TemporalNode::Atom(0)],0,TemporalProfile::ClosedFiniteKleeneV1).unwrap()
}
pub fn trace(mode: Mode, secondary: bool, truth: Truth) -> FiniteTrace {
    let c=clock(mode,secondary);
    FiniteTrace::checked((0..2).map(|t| TraceStep::new(
        Instant::absolute(c,t,TimeUnit::Second).unwrap(),vec![truth])).collect(),TraceClosure::Closed).unwrap()
}
pub fn temporal<'a>(query:&'a TemporalQuery,trace:&'a FiniteTrace)->TemporalModelInput<'a> {
    TemporalModelInput{query,trace,scope:scope()}
}
pub fn spatial(mode:Mode,swap:bool)->SpatialModelInput {
    let frame=FrameId(identity(mode,60));
    let a=Rectangle::new(frame,[0,0],[2,2]).unwrap();
    let b=Rectangle::new(frame,[4,0],[6,2]).unwrap();
    SpatialModelInput{left:Some(if swap {b}else{a}),right:Some(if swap {a}else{b}),frame,
        unit:SpatialUnit::Pixel,context:context(),scope:scope()}
}
pub fn time_policy(a:TemporalModelInput<'_>,b:TemporalModelInput<'_>)->TemporalPreservationPolicy {
    TemporalPreservationPolicy::declared(a,b,vec![ProtectedTemporalProbe{id:pin(50),weight:1,
        relation:ProtectedTemporalRelation::MappedPredicateTruth}],Some(DeclaredTimeTranslation{
            source_clock:a.trace.steps()[0].instant().clock(),target_clock:b.trace.steps()[0].instant().clock(),offset_nanoseconds:0})).unwrap()
}
pub fn space_policy(a:SpatialModelInput,b:SpatialModelInput)->SpatialPreservationPolicy {
    SpatialPreservationPolicy::declared(a,b,vec![ProtectedSpatialProbe{id:pin(70),weight:1,
        relation:ProtectedSpatialRelation::Kernel(SpatialRelation::StrictlyLeftOf)}],Some(DeclaredSpatialMap{
            source_frame:a.frame,target_frame:b.frame,source_unit:a.unit,target_unit:b.unit,
            signs:(AxisSign::Positive,AxisSign::Positive),translation:(0,0)})).unwrap()
}
pub fn encoded(term:Term)->Vec<u8> { semantic_term::encode(&term,TermLimits::default()).unwrap() }
pub fn symbol(value:&str)->Term {Term::Symbol(value.into())}
pub fn meaning(path:&str,payload:Term)->Vec<u8> {
    encoded(Term::Apply("identity-meaning-v1".into(),vec![symbol(path),payload]))
}
pub fn clock_meaning(path:&str)->Vec<u8> {
    meaning(&format!("Clock/{path}"),Term::Apply("clock-meaning-v1".into(),vec![symbol("Monotonic"),Term::Integer(0.into()),symbol("logical-clock")]))
}
pub fn frame_meaning()->Vec<u8> {
    meaning("Frame/primary",Term::Apply("frame-meaning-v1".into(),vec![symbol("Pixel"),symbol("x-right"),symbol("y-up"),
        symbol("right-handed"),Term::Integer(0.into()),Term::Integer(0.into())]))
}
pub fn case_spec()->Vec<u8> {
    encoded(Term::Apply("stage-b-case-spec-v1".into(),vec![symbol("BooleanReferenceAgreement"),
        Term::Apply("atoms-v1".into(),vec![]),Term::Apply("theory-v1".into(),vec![]),
        symbol("SoftFiniteValidation"),symbol("AllAdmissibleAssignments")]))
}
