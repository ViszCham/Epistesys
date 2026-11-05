//! Admission boundaries not semantic-scoring stubs.
use super::*;
#[test]
fn tlcx22_explain_negative_baseline_shape_role_weights_duplicates_and_expected_source() {
    let f = Fixture::explain();
    let proof = f.projection(false, false);
    let cases = [f.case(b"p", &f.p, &proof)];
    let ri = root_input(&cases);
    let (_br, _run, res) = run(Cancellation::default());
    let view = res.lease();
    assert!(matches!(
        e::capture_explain(root_input(&[]), &view),
        Err(e::Error::InvalidInput(_))
    ));
    let duplicate = [cases[0], cases[0]];
    assert!(matches!(
        e::capture_explain(root_input(&duplicate), &view),
        Err(e::Error::InvalidInput(_))
    ));
    let zero_weight = q(0, 1);
    let bad_weight = [e::ExplainCase {
        weight: &zero_weight,
        ..cases[0]
    }];
    assert!(matches!(
        e::capture_explain(root_input(&bad_weight), &view),
        Err(e::Error::InvalidInput(_))
    ));
    let bad_proof = [e::Step {
        conclusion: &f.p,
        rule: e::Rule::Premise { index: 9 },
    }];
    let bad_base = [f.case(b"p", &f.p, &bad_proof)];
    assert!(matches!(
        e::capture_explain(root_input(&bad_base), &view),
        Err(e::Error::InvalidInput(_))
    ));
    for role in [
        e::DatasetRole::Training,
        e::DatasetRole::Search,
        e::DatasetRole::TransferValidation,
        e::DatasetRole::FinalResearchHoldout,
    ] {
        let mut wrong = ri;
        wrong.binding.role = role;
        assert!(matches!(
            e::capture_explain(wrong, &view),
            Err(e::Error::UnsupportedRole)
        ));
    }
    let mut wrong = ri;
    wrong.binding.selection.source = SourceRevision::of("not this reservation source");
    assert!(matches!(
        e::capture_explain(wrong, &view),
        Err(e::Error::Resource(ResourceError::SourceMismatch))
    ));
    let root = e::capture_explain(ri, &view).unwrap();
    let duplicated = [response(b"p", &proof), response(b"p", &proof)];
    assert!(matches!(
        e::admit_explain(&root, input(1, &duplicated), &view),
        Err(e::Error::InvalidInput(_))
    ));
    let unknown = [response(b"unregistered", &proof)];
    assert!(matches!(
        e::admit_explain(&root, input(1, &unknown), &view),
        Err(e::Error::InvalidInput(_))
    ));
}
