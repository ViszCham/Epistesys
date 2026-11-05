//! Boundary and current-reservation cuts for the explicit numeric payload profile.
//! Numeric rows here are caller-reported; this test does not establish world provenance.
#[path = "tlcx23_formal_support/mod.rs"]
mod support;

use lc631_core::context::ContextRevision;
use lc631_core::measurement_numeric::RationalInterval as Interval;
use lc631_core::CandidateId;
use lc631_runtime::joint_selection::*;
use lc631_runtime::semantic_accelerator::selection::*;
use lc631_substrate::resource::{Cancellation, ResourceError};
use support::*;

fn epoch() -> EpochStamp {
    EpochStamp {
        epoch: 1,
        generation: 1,
        fence: 1,
    }
}

fn row<'a>(
    key: &'a [u8],
    values: &'a [AxisValue<'a>],
    one: &'a Interval,
    context: ContextRevision,
    pins: SelectionPins,
) -> JointCandidate<'a> {
    JointCandidate {
        id: CandidateId(1),
        context,
        pins,
        hard_gate: ReportedGate::Pass,
        map_back: ReportedGate::Pass,
        axes: values,
        total_loss: AxisValue::Exact(one),
        canonical_key: key,
        measurement_binding: d(1),
    }
}

#[test]
fn tlcx33_payload_profile_exact_cap_token_and_reservation_cuts() {
    cases(|c| {
        let cancel = Cancellation::default();
        let b = broker();
        let run = run(&b, c.plan.scope().source.digest(), cancel.clone());
        let reservation = reserve(&run, 32 * 1024 * 1024, 1024 * 1024);
        let view = reservation.lease();
        let foreign = reserve(&run, 8 * 1024 * 1024, 1024 * 1024);
        let foreign_view = foreign.lease();
        let insufficient = reserve(&run, 2 * 1024 * 1024 + 256 * 1024, 1024 * 1024);
        let small_view = insufficient.lease();
        let pins = pins(c.plan);
        let one = Interval::exact_ratio(1, 1).unwrap();
        let axes = [AxisKind::Goal(d(40))];
        let values = [AxisValue::Exact(&one)];
        let policy = SelectionPolicy {
            pins,
            axes: &axes,
            representative_order: None,
            declared_goal_solve_overlap: false,
            max_candidates: 1,
            max_canonical_bytes: 4 * 1024 * 1024 + 1,
        };
        let small = [row(b"same-input", &values, &one, c.good.context(), pins)];
        let legacy = prepare_numeric_batch(
            &policy,
            &small,
            SearchCoverage::EvaluatedSet,
            epoch(),
            &view,
            1,
        )
        .unwrap();
        let extended = prepare_numeric_batch_with_profile(
            &policy,
            &small,
            SearchCoverage::EvaluatedSet,
            epoch(),
            &view,
            1,
            NumericPayloadProfile::TiledWorld2048V1,
        )
        .unwrap();
        let explicit_legacy = prepare_numeric_batch_with_profile(
            &policy,
            &small,
            SearchCoverage::EvaluatedSet,
            epoch(),
            &view,
            1,
            NumericPayloadProfile::LegacyV1,
        )
        .unwrap();
        assert_eq!(legacy.token(), explicit_legacy.token());
        assert_eq!(legacy.payload_profile(), NumericPayloadProfile::LegacyV1);
        assert_eq!(
            extended.payload_profile(),
            NumericPayloadProfile::TiledWorld2048V1
        );
        assert_ne!(legacy.token(), extended.token());
        assert!(matches!(
            finish_selection(&extended, None, &foreign_view),
            Err(ConsumerError::ForeignReservation)
        ));
        let legacy_in_small_reservation = prepare_numeric_batch(
            &policy,
            &small,
            SearchCoverage::EvaluatedSet,
            epoch(),
            &small_view,
            1,
        )
        .unwrap();
        drop(legacy_in_small_reservation);
        assert!(matches!(
            prepare_numeric_batch_with_profile(
                &policy,
                &small,
                SearchCoverage::EvaluatedSet,
                epoch(),
                &small_view,
                1,
                NumericPayloadProfile::TiledWorld2048V1,
            ),
            Err(ConsumerError::Resource(_))
        ));

        // Exact 1/1 has two endpoints of 19 bytes each, for the axis and
        // total: 76 accounted bytes, plus the fixed per-row 256 bytes.
        const CAP: usize = 4 * 1024 * 1024;
        const FIXED_ACCOUNTED: usize = 256 + 76;
        let key_at_cap = vec![b'x'; CAP - FIXED_ACCOUNTED];
        let at_cap = [row(&key_at_cap, &values, &one, c.good.context(), pins)];
        assert!(matches!(
            prepare_numeric_batch(
                &policy,
                &at_cap,
                SearchCoverage::EvaluatedSet,
                epoch(),
                &view,
                1,
            ),
            Err(ConsumerError::PayloadLimit)
        ));
        let exact = prepare_numeric_batch_with_profile(
            &policy,
            &at_cap,
            SearchCoverage::EvaluatedSet,
            epoch(),
            &view,
            1,
            NumericPayloadProfile::TiledWorld2048V1,
        )
        .unwrap();
        drop(exact);
        let key_over_cap = vec![b'x'; CAP - FIXED_ACCOUNTED + 1];
        let over_cap = [row(&key_over_cap, &values, &one, c.good.context(), pins)];
        assert!(matches!(
            prepare_numeric_batch_with_profile(
                &policy,
                &over_cap,
                SearchCoverage::EvaluatedSet,
                epoch(),
                &view,
                1,
                NumericPayloadProfile::TiledWorld2048V1,
            ),
            Err(ConsumerError::PayloadLimit)
        ));
        assert!(matches!(
            prepare_numeric_batch_with_profile(
                &policy,
                &small,
                SearchCoverage::EvaluatedSet,
                epoch(),
                &view,
                8_388_609,
                NumericPayloadProfile::TiledWorld2048V1,
            ),
            Err(ConsumerError::WorkLimit)
        ));
        let three_values = [
            AxisValue::Exact(&one),
            AxisValue::Exact(&one),
            AxisValue::Exact(&one),
        ];
        let mut too_many: Vec<_> = (0..2049)
            .map(|i| {
                let mut item = row(b"same-input", &values, &one, c.good.context(), pins);
                item.id = CandidateId(i + 1);
                item
            })
            .collect();
        let many_policy = SelectionPolicy {
            max_candidates: 2049,
            ..policy
        };
        assert!(matches!(
            prepare_numeric_batch_with_profile(
                &many_policy,
                &too_many,
                SearchCoverage::EvaluatedSet,
                epoch(),
                &view,
                8_388_608,
                NumericPayloadProfile::TiledWorld2048V1,
            ),
            Err(ConsumerError::WorkLimit)
        ));
        too_many.truncate(2048);
        let three_kinds = [
            AxisKind::Goal(d(40)),
            AxisKind::SemanticLoss,
            AxisKind::TemporalLoss,
        ];
        for item in &mut too_many {
            item.axes = &three_values;
        }
        let three_policy = SelectionPolicy {
            axes: &three_kinds,
            max_candidates: 2048,
            ..policy
        };
        assert!(matches!(
            prepare_numeric_batch_with_profile(
                &three_policy,
                &too_many,
                SearchCoverage::EvaluatedSet,
                epoch(),
                &view,
                8_388_608,
                NumericPayloadProfile::TiledWorld2048V1,
            ),
            Err(ConsumerError::WorkLimit)
        ));
        // Enough RAM to prepare the batch does not imply enough to retain a
        // separate result claim while that batch remains alive.
        let batch_only = reserve(
            &run,
            2 * 1024 * 1024 + 256 * 1024 + 4 * 1024 * 1024,
            1024 * 1024,
        );
        let batch_only_view = batch_only.lease();
        let unfinishable = prepare_numeric_batch_with_profile(
            &policy,
            &small,
            SearchCoverage::EvaluatedSet,
            epoch(),
            &batch_only_view,
            1,
            NumericPayloadProfile::TiledWorld2048V1,
        )
        .unwrap();
        assert!(matches!(
            finish_selection(&unfinishable, None, &batch_only_view),
            Err(ConsumerError::Resource(_))
        ));
        drop(unfinishable);

        // Batch and selection own separate 4 MiB profile charges. The report
        // remains charged after the batch is dropped and releases on drop.
        let tight = reserve(&run, 2 * (256 * 1024 + 4 * 1024 * 1024), 1024 * 1024);
        let tight_view = tight.lease();
        let batch = prepare_numeric_batch_with_profile(
            &policy,
            &small,
            SearchCoverage::EvaluatedSet,
            epoch(),
            &tight_view,
            1,
            NumericPayloadProfile::TiledWorld2048V1,
        )
        .unwrap();
        let selection = finish_selection(&batch, None, &tight_view).unwrap();
        drop(batch);
        assert_eq!(
            selection.project_current(&tight_view).unwrap().frontier,
            [0]
        );
        assert!(matches!(
            prepare_numeric_batch_with_profile(
                &policy,
                &small,
                SearchCoverage::EvaluatedSet,
                epoch(),
                &tight_view,
                1,
                NumericPayloadProfile::TiledWorld2048V1,
            ),
            Err(ConsumerError::Resource(_))
        ));
        drop(selection);
        let after_release = prepare_numeric_batch_with_profile(
            &policy,
            &small,
            SearchCoverage::EvaluatedSet,
            epoch(),
            &tight_view,
            1,
            NumericPayloadProfile::TiledWorld2048V1,
        )
        .unwrap();
        drop(after_release);
        cancel.cancel();
        assert!(matches!(
            finish_selection(&extended, None, &view),
            Err(ConsumerError::Resource(ResourceError::Cancelled))
        ));
        assert!(matches!(
            prepare_numeric_batch_with_profile(
                &policy,
                &small,
                SearchCoverage::EvaluatedSet,
                epoch(),
                &view,
                1,
                NumericPayloadProfile::TiledWorld2048V1,
            ),
            Err(ConsumerError::Resource(ResourceError::Cancelled))
        ));
    });
}
