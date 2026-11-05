//! Native test fixture support, not a separate Cargo target. Given meanings are NOT source truth.
use super::*;
pub const AND_ROWS: [bool; 4] = [false, false, false, true];
pub const P_ROWS: [bool; 4] = [false, false, true, true];
pub const Q_ROWS: [bool; 4] = [false, true, false, true];

pub fn original_three(
    source: &SourceStore,
    second_mutable: bool,
    third_mutable: bool,
) -> OriginalContext {
    OriginalContext::checked(
        SourceRevision::of(source.text()),
        ContextScope::declared(d(1), d(2), d(3), d(4)),
        AuthorityRevision(7),
        vec![
            ContextPremise::from_source(
                d(5),
                source,
                SourceSpan::checked(source, 0, 7).unwrap(),
                PremiseBasis::Given,
                true,
            )
            .unwrap(),
            ContextPremise::from_source(
                d(6),
                source,
                SourceSpan::checked(source, 8, 9).unwrap(),
                PremiseBasis::Given,
                second_mutable,
            )
            .unwrap(),
            ContextPremise::from_source(
                d(7),
                source,
                SourceSpan::checked(source, 10, 11).unwrap(),
                PremiseBasis::Given,
                third_mutable,
            )
            .unwrap(),
        ],
        ContextLimits {
            max_contexts: 17,
            max_premises: 3,
            max_edits: 1,
            max_statement_bytes: 8192,
        },
    )
    .unwrap()
}

pub struct Native<'a> {
    pub source: &'a SourceStore,
    pub theory: &'a SmtStatusQuery,
    pub goal: &'a SmtStatusQuery,
    pub premises: &'a [GivenPremise<'a>],
    pub atoms: &'a [n::AtomMap],
    pub facts: &'a [n::FactFeature<'a>],
    pub constructions: &'a [n::ConstructionFeature<'a>],
}
impl<'a> Native<'a> {
    pub fn capture<'v>(
        &self,
        original: OriginalContext,
        limits: n::Limits,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<n::RootPlan<'a, 'v>, n::Error> {
        let mut bridge = bridge_limits();
        bridge.max_ast_nodes = 512;
        n::RootPlan::capture_current(
            n::RootInput {
                source: self.source,
                original,
                source_query: self.theory,
                premises: self.premises,
                target: d(5),
                goal: GoalPolicy {
                    revision: d(2),
                    query: self.goal,
                },
                bridge_limits: bridge,
            },
            n::Declaration {
                atoms: self.atoms,
                facts: self.facts,
                constructions: self.constructions,
                archive: n::ArchiveInput::Unread,
                corpus_revision: d(70),
                oracle_revision: d(71),
                policy_revision: d(72),
                limits,
            },
            view,
        )
    }
}

#[macro_export]
macro_rules! policy_native_fixture {
    ($native:ident, $view:ident, $target:expr, $body:block) => {
        let source = SourceStore::new(Arc::from("p and q\np\np"), 256 * 1024).unwrap();
        // The two final identical TEXT occurrences have DIFFERENT Given ASTs.
        // No NLP parsing/semantic truth is claimed by this declared mapping.
        let theory = query(0, 1, vec![$target, var(0), var(1)]);
        let goal = query(0, 1, vec![var(0)]);
        let premises = [
            GivenPremise {
                id: d(5),
                span: SourceSpan::checked(&source, 0, 7).unwrap(),
                formula: &theory.assertions()[0],
            },
            GivenPremise {
                id: d(6),
                span: SourceSpan::checked(&source, 8, 9).unwrap(),
                formula: &theory.assertions()[1],
            },
            GivenPremise {
                id: d(7),
                span: SourceSpan::checked(&source, 10, 11).unwrap(),
                formula: &theory.assertions()[2],
            },
        ];
        let atoms = [
            n::AtomMap {
                local: BoolSymbolId::new(0),
                meaning: 0,
                grounding: SourceSpan::checked(&source, 0, 1).unwrap(),
            },
            n::AtomMap {
                local: BoolSymbolId::new(1),
                meaning: 1,
                grounding: SourceSpan::checked(&source, 6, 7).unwrap(),
            },
        ];
        let one = q(1, 1);
        let weak = BoolExpr::Or(vec![theory.assertions()[0].clone(), var(0)]);
        let facts = [
            n::FactFeature {
                formula: &theory.assertions()[0],
                weight: &one,
            },
            n::FactFeature {
                formula: &theory.assertions()[1],
                weight: &one,
            },
            n::FactFeature {
                formula: &theory.assertions()[2],
                weight: &one,
            },
        ];
        let constructions = [
            n::ConstructionFeature {
                formula: &theory.assertions()[0],
                weight: &one,
            },
            n::ConstructionFeature {
                formula: &theory.assertions()[1],
                weight: &one,
            },
            n::ConstructionFeature {
                formula: &theory.assertions()[2],
                weight: &one,
            },
            n::ConstructionFeature {
                formula: &weak,
                weight: &one,
            },
        ];
        let broker =
            ResourceBroker::new(ResourceLimits::uniform(128 * 1024 * 1024).unwrap()).unwrap();
        let run = broker
            .begin_run(
                1,
                source.id().digest(),
                ResourceLimits::uniform(128 * 1024 * 1024).unwrap(),
                Deadline::after(Duration::from_secs(60)).unwrap(),
                Cancellation::default(),
            )
            .unwrap();
        let reservation = run
            .reserve(&[
                resource(ResourceKind::Ram, 96 * 1024 * 1024),
                resource(ResourceKind::Output, 4 * 1024 * 1024),
            ])
            .unwrap();
        let $view = reservation.lease();
        let $native = Native {
            source: &source,
            theory: &theory,
            goal: &goal,
            premises: &premises,
            atoms: &atoms,
            facts: &facts,
            constructions: &constructions,
        };
        $body
    };
}
