//! Given complete fixture-source accounting ONLY, never a current cost owner.
//! These are actual complete helper bytes, not labels/digests/future paths.
//! Assigning roles is Given; no executable-code completeness is certified.
use super::{models, wire::{app, decoded, encoded, sym}};
use lc631_runtime::product_correspondence::{
    CodeRole, DeferredReason, ProductError, ProductPlan, ProvidedBundleInput,
    ProvidedCodeUnit, ProvidedPayload,
};
use lc631_substrate::resource::ResourceReservationView;

pub fn exercise(plan: &ProductPlan<'_>, view: &ResourceReservationView<'_>) {
    let semantic = encoded(&models::golden(0));
    let dictionary_term = app("semantic-dictionaries-v1", vec![
        sym("p"), sym("sensor"), models::meaning(lc631_runtime::product_correspondence::IdentityKind::Clock, 0),
        models::meaning(lc631_runtime::product_correspondence::IdentityKind::Origin, 0),
        models::meaning(lc631_runtime::product_correspondence::IdentityKind::Reference, 0),
        models::meaning(lc631_runtime::product_correspondence::IdentityKind::Frame, 0),
    ]);
    let dictionary = encoded(&dictionary_term);
    let sources: [&[u8]; 3] = [include_bytes!("wire.rs"), include_bytes!("models.rs"), include_bytes!("cost_contract.rs")];
    let roles = [CodeRole::Projection, CodeRole::Unprojection, CodeRole::CorrespondenceAdapter];
    let units: Vec<_> = roles.iter().copied().zip(sources).map(|(role, bytes)| {
        ProvidedCodeUnit { role, payload: ProvidedPayload::Inline(bytes) }
    }).collect();
    let input = || ProvidedBundleInput {
        semantic: &semantic, code_units: &units,
        semantic_dictionary: ProvidedPayload::Inline(&dictionary),
    };
    let accounting = plan.account_provided_bundle(input(), view).unwrap();
    let names = ["Projection", "Unprojection", "CorrespondenceAdapter"];
    let code = app("provided-code-closure-v1", names.into_iter().zip(sources).map(|(role, bytes)| {
        app("code-unit-v1", vec![sym(role), sym(std::str::from_utf8(bytes).unwrap())])
    }).collect());
    let expected = encoded(&app("bundle-v1", vec![models::golden(0), code, dictionary_term]));
    assert_eq!(accounting.bundle_bytes(), expected);
    assert_eq!(accounting.bits(), 8 * u64::try_from(expected.len()).unwrap());
    assert_eq!(accounting.scalar_status(), DeferredReason::FullCostOwnerMissing);
    // Independent K6 extraction consumes target bundle bytes only, never a
    // SourceStore/context/owner or raw external-ID envelope lookup capability.
    let bundle = decoded(accounting.bundle_bytes());
    let super::wire::Node::App(name, fields) = bundle else { panic!("K6 bundle") };
    assert_eq!(name, "bundle-v1");
    assert_eq!(fields[0], models::golden(0));

    let cut = ProvidedBundleInput {
        semantic: &semantic, code_units: &units[..2],
        semantic_dictionary: ProvidedPayload::Inline(&dictionary),
    };
    assert!(matches!(plan.account_provided_bundle(cut, view), Err(ProductError::Deferred(DeferredReason::MissingInlineCode))));
    let missing_dictionary = ProvidedBundleInput {
        semantic: &semantic, code_units: &units,
        semantic_dictionary: ProvidedPayload::Inline(&[]),
    };
    assert!(matches!(plan.account_provided_bundle(missing_dictionary, view), Err(ProductError::Deferred(DeferredReason::MissingSemanticDictionary))));
    let external_dictionary = ProvidedBundleInput {
        semantic: &semantic, code_units: &units,
        semantic_dictionary: ProvidedPayload::ExternalReference(b"SourceStore/context/owner-sidecar"),
    };
    assert!(matches!(plan.account_provided_bundle(external_dictionary, view), Err(ProductError::Deferred(DeferredReason::ExternalLookupRequired))));
    let external_code = [
        ProvidedCodeUnit { role: CodeRole::Projection, payload: ProvidedPayload::ExternalReference(b"digest-or-path-is-not-code") },
        ProvidedCodeUnit { role: CodeRole::Unprojection, payload: ProvidedPayload::Inline(sources[1]) },
        ProvidedCodeUnit { role: CodeRole::CorrespondenceAdapter, payload: ProvidedPayload::Inline(sources[2]) },
    ];
    assert!(matches!(plan.account_provided_bundle(ProvidedBundleInput {
        semantic: &semantic, code_units: &external_code,
        semantic_dictionary: ProvidedPayload::Inline(&dictionary),
    }, view), Err(ProductError::Deferred(DeferredReason::ExternalLookupRequired))));

    // Equal byte cost is not equal code/data lineage. Mutated Given bytes are
    // not executed, compiled, or promoted to a complete current code closure.
    let mut changed = sources[0].to_vec();
    let first_letter = changed.iter().position(u8::is_ascii_alphabetic).unwrap();
    changed[first_letter] = if changed[first_letter] == b'X' { b'Y' } else { b'X' };
    let changed_units = [
        ProvidedCodeUnit { role: CodeRole::Projection, payload: ProvidedPayload::Inline(&changed) },
        ProvidedCodeUnit { role: CodeRole::Unprojection, payload: ProvidedPayload::Inline(sources[1]) },
        ProvidedCodeUnit { role: CodeRole::CorrespondenceAdapter, payload: ProvidedPayload::Inline(sources[2]) },
    ];
    let changed_accounting = plan.account_provided_bundle(ProvidedBundleInput {
        semantic: &semantic, code_units: &changed_units,
        semantic_dictionary: ProvidedPayload::Inline(&dictionary),
    }, view).unwrap();
    assert_eq!(changed_accounting.bits(), accounting.bits());
    assert_ne!(changed_accounting.binding(), accounting.binding());
    assert_eq!(changed_accounting.scalar_status(), DeferredReason::FullCostOwnerMissing);
}
