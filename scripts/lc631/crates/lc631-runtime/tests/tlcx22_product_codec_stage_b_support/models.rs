//! Complete bounded native fixture and hand-declared product schema expectation.
//! Context/meaning declarations are Given; no actual-owner correspondence here.
// Shared fixture surface: individual integration targets use different subsets.
#![allow(dead_code)]
use super::wire::{app, encoded, sym, Node};
use lc631_core::{
    context::{ContextRevision, ContextScope, SourceRevision},
    measurement_numeric::Rational,
};
use lc631_runtime::{
    adversarial_measurement::{Response, SemanticAtom},
    product_correspondence::{
        CaseDeclaration, IdentityDeclaration, IdentityKind, PlanSpec, ProductInput, ProductLimits,
        TlProfile,
    },
    spatial_preservation::{SpatialModelInput, SpatialUnit},
    temporal_preservation::TemporalModelInput,
};
use lc631_solvers::{
    causal_spatial::{FrameId, Rectangle},
    temporal::{
        ClockSpec, FiniteTrace, Instant, TemporalNode, TemporalProfile, TemporalQuery, TimeScale,
        TimeUnit, TraceClosure, TraceStep, Truth,
    },
    typed_query::{BoolExpr, BoolSymbolId, SmtEmitLimits, SmtStatusQuery},
};
use lc631_substrate::Digest32;

pub fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
pub fn case_spec(atom: bool) -> Node {
    app(
        "stage-b-case-spec-v1",
        vec![
            sym("BooleanReferenceAgreement"),
            app("atoms-v1", if atom { vec![sym("p")] } else { vec![] }),
            app(
                "theory-v1",
                if atom {
                    vec![app("atom-v1", vec![sym("p")])]
                } else {
                    vec![]
                },
            ),
            sym("SoftFiniteValidation"),
            sym("AllAdmissibleAssignments"),
        ],
    )
}
pub fn meaning(kind: IdentityKind, role: u16) -> Node {
    let (path, payload) = match kind {
        IdentityKind::Clock => (
            if role == 0 {
                "Clock/primary"
            } else {
                "Clock/secondary"
            },
            app(
                "clock-meaning-v1",
                vec![sym("Monotonic"), Node::Int(0), sym("logical-clock")],
            ),
        ),
        IdentityKind::Origin => ("Origin/primary", sym("epoch-zero")),
        IdentityKind::Reference => ("Reference/primary", sym("first-event")),
        IdentityKind::Frame => (
            if role == 0 {
                "Frame/primary"
            } else {
                "Frame/secondary"
            },
            app(
                "frame-meaning-v1",
                vec![
                    sym("Pixel"),
                    sym("x-right"),
                    sym("y-up"),
                    sym("right-handed"),
                    Node::Int(0),
                    Node::Int(0),
                ],
            ),
        ),
    };
    app("identity-meaning-v1", vec![sym(path), payload])
}

pub struct Fixture {
    pub context: ContextRevision,
    pub scope: ContextScope,
    pub clock: ClockSpec,
    pub secondary_clock: ClockSpec,
    pub frame: FrameId,
    pub query: TemporalQuery,
    pub trace: FiniteTrace,
    pub spatial: SpatialModelInput,
    pub empty_response: BoolExpr,
    pub atom_response: BoolExpr,
    pub atom: [SemanticAtom<'static>; 1],
    pub specs: [Vec<u8>; 2],
    pub references: [Vec<u8>; 2],
    pub meanings: [Vec<u8>; 5],
    pub temporal_meaning: Vec<u8>,
    pub weights: [Rational; 2],
    pub offset: u8,
}
impl Fixture {
    pub fn new(offset: u8) -> Self {
        let context =
            ContextRevision::original(SourceRevision::of("stage-b codec Given finite models"));
        let scope = ContextScope::declared(d(1), d(2), d(3), d(4));
        let clock = ClockSpec::exact(
            d(10 + offset),
            TimeScale::Monotonic,
            d(11 + offset),
            d(12 + offset),
        );
        let secondary_clock = ClockSpec::exact(
            d(14 + offset),
            TimeScale::Monotonic,
            d(11 + offset),
            d(12 + offset),
        );
        let frame = FrameId([13 + offset; 32]);
        let query = TemporalQuery::checked(
            context,
            vec![TemporalNode::Atom(0)],
            0,
            TemporalProfile::ClosedFiniteKleeneV1,
        )
        .unwrap();
        let trace = trace(clock, &[Truth::Unknown, Truth::True]);
        let spatial = SpatialModelInput {
            left: Some(Rectangle::new(frame, [0, 0], [1, 1]).unwrap()),
            right: Some(Rectangle::new(frame, [2, 0], [3, 1]).unwrap()),
            frame,
            unit: SpatialUnit::Pixel,
            context,
            scope,
        };
        Self {
            context,
            scope,
            clock,
            secondary_clock,
            frame,
            query,
            trace,
            spatial,
            empty_response: BoolExpr::Constant(false),
            atom_response: BoolExpr::Variable(BoolSymbolId::new(17)),
            atom: [SemanticAtom {
                symbol: BoolSymbolId::new(17),
                meaning: b"p",
            }],
            specs: [encoded(&case_spec(false)), encoded(&case_spec(true))],
            references: [
                encoded(&Node::Bool(true)),
                encoded(&app("atom-v1", vec![sym("p")])),
            ],
            meanings: [
                encoded(&meaning(IdentityKind::Clock, 0)),
                encoded(&meaning(IdentityKind::Origin, 0)),
                encoded(&meaning(IdentityKind::Reference, 0)),
                encoded(&meaning(IdentityKind::Frame, 0)),
                encoded(&meaning(IdentityKind::Clock, 1)),
            ],
            temporal_meaning: encoded(&sym("sensor")),
            weights: [
                Rational::from_integer(2.into()),
                Rational::from_integer(3.into()),
            ],
            offset,
        }
    }
    pub fn declarations(&self) -> Declarations<'_> {
        Declarations {
            // Opposite native-key order: the slot order must come from specs.
            cases: [
                CaseDeclaration {
                    native_key: b"a-atom",
                    native_atoms: &self.atom,
                    semantic_spec: &self.specs[1],
                    reference_contract: &self.references[1],
                    oracle: d(20),
                    weight: &self.weights[1],
                },
                CaseDeclaration {
                    native_key: b"z-empty",
                    native_atoms: &[],
                    semantic_spec: &self.specs[0],
                    reference_contract: &self.references[0],
                    oracle: d(21),
                    weight: &self.weights[0],
                },
            ],
            identities: vec![
                IdentityDeclaration {
                    kind: IdentityKind::Clock,
                    local_role: 0,
                    native_id: [10 + self.offset; 32],
                    meaning: &self.meanings[0],
                },
                IdentityDeclaration {
                    kind: IdentityKind::Origin,
                    local_role: 0,
                    native_id: [11 + self.offset; 32],
                    meaning: &self.meanings[1],
                },
                IdentityDeclaration {
                    kind: IdentityKind::Reference,
                    local_role: 0,
                    native_id: [12 + self.offset; 32],
                    meaning: &self.meanings[2],
                },
                IdentityDeclaration {
                    kind: IdentityKind::Frame,
                    local_role: 0,
                    native_id: [13 + self.offset; 32],
                    meaning: &self.meanings[3],
                },
                IdentityDeclaration {
                    kind: IdentityKind::Clock,
                    local_role: 1,
                    native_id: [14 + self.offset; 32],
                    meaning: &self.meanings[4],
                },
            ],
            temporal: [&self.temporal_meaning],
        }
    }
    pub fn responses(&self) -> [Response<'_>; 2] {
        [
            Response {
                case: b"a-atom",
                term: &self.atom_response,
            },
            Response {
                case: b"z-empty",
                term: &self.empty_response,
            },
        ]
    }
    pub fn input<'a>(&'a self, responses: &'a [Response<'a>]) -> ProductInput<'a> {
        ProductInput {
            responses,
            temporal: TemporalModelInput {
                query: &self.query,
                trace: &self.trace,
                scope: self.scope,
            },
            spatial: self.spatial,
        }
    }
}
pub struct Declarations<'a> {
    pub cases: [CaseDeclaration<'a>; 2],
    pub identities: Vec<IdentityDeclaration<'a>>,
    pub temporal: [&'a [u8]; 1],
}
impl Declarations<'_> {
    pub fn spec<'a>(&'a self, fixture: &Fixture) -> PlanSpec<'a> {
        PlanSpec {
            context: fixture.context,
            scope: fixture.scope,
            goal_profile: None,
            tl_profile: TlProfile::TemporalSpatial1To3,
            cases: &self.cases,
            identities: &self.identities,
            temporal_atom_meanings: &self.temporal,
            limits: ProductLimits::default(),
        }
    }
}
pub fn trace(clock: ClockSpec, values: &[Truth]) -> FiniteTrace {
    FiniteTrace::checked(
        values
            .iter()
            .enumerate()
            .map(|(i, truth)| {
                TraceStep::new(
                    Instant::absolute(clock, (i * 10) as i64, TimeUnit::Nanosecond).unwrap(),
                    vec![*truth],
                )
            })
            .collect(),
        TraceClosure::Closed,
    )
    .unwrap()
}

/// Hand-authored schema tree, independent from all product codec functions.
pub fn golden(clock_role: u16) -> Node {
    let role = i128::from(clock_role);
    app(
        "tlcx-finite-product-v1",
        vec![
            app(
                "response-cases-v1",
                vec![
                    app(
                        "case-slot-v1",
                        vec![Node::Int(0), case_spec(false), Node::Bool(false)],
                    ),
                    app(
                        "case-slot-v1",
                        vec![
                            Node::Int(1),
                            case_spec(true),
                            app("atom-v1", vec![sym("p")]),
                        ],
                    ),
                ],
            ),
            app(
                "temporal-model-v1",
                vec![
                    sym("ClosedFiniteKleeneV1"),
                    app(
                        "clock-v1",
                        vec![
                            Node::Int(role),
                            meaning(IdentityKind::Clock, clock_role),
                            app(
                                "origin-v1",
                                vec![Node::Int(0), meaning(IdentityKind::Origin, 0)],
                            ),
                            app(
                                "reference-v1",
                                vec![Node::Int(0), meaning(IdentityKind::Reference, 0)],
                            ),
                        ],
                    ),
                    app("atoms-v1", vec![sym("sensor")]),
                    app("atom-v1", vec![sym("sensor")]),
                    app(
                        "closed-trace-v1",
                        vec![
                            app(
                                "step-v1",
                                vec![
                                    Node::Int(0),
                                    Node::Int(role),
                                    app("truths-v1", vec![sym("Unknown")]),
                                ],
                            ),
                            app(
                                "step-v1",
                                vec![
                                    Node::Int(10),
                                    Node::Int(role),
                                    app("truths-v1", vec![sym("True")]),
                                ],
                            ),
                        ],
                    ),
                ],
            ),
            app(
                "spatial-pair-v1",
                vec![
                    app(
                        "frame-v1",
                        vec![Node::Int(0), meaning(IdentityKind::Frame, 0)],
                    ),
                    sym("Pixel"),
                    app(
                        "rect-v1",
                        vec![
                            Node::Int(0),
                            Node::Int(0),
                            Node::Int(0),
                            Node::Int(1),
                            Node::Int(1),
                        ],
                    ),
                    app(
                        "rect-v1",
                        vec![
                            Node::Int(0),
                            Node::Int(2),
                            Node::Int(0),
                            Node::Int(3),
                            Node::Int(1),
                        ],
                    ),
                ],
            ),
        ],
    )
}

#[allow(dead_code)]
pub fn inhabited_theory(atom: bool) -> SmtStatusQuery {
    SmtStatusQuery::checked(
        if atom {
            vec![BoolSymbolId::new(17)]
        } else {
            vec![]
        },
        vec![],
        if atom {
            vec![BoolExpr::Variable(BoolSymbolId::new(17))]
        } else {
            vec![]
        },
        SmtEmitLimits::checked(4096, 4, 0, 16, 64, 8).unwrap(),
    )
    .unwrap()
}
