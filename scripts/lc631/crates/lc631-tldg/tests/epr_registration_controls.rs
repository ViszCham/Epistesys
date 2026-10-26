use lc631_tldg::{default_backend_registry, default_profile_registry};

#[test]
fn epr_05_existing_profile_registration_is_transactional() {
    let mut registry = default_profile_registry();
    let before = registry.clone();
    let mut duplicate = registry.iter().next().unwrap().clone();
    duplicate.name = "rejected replacement".into();
    duplicate.revision = "unaccepted-v2".into();
    assert!(registry.insert(duplicate).is_err());
    assert_eq!(registry, before);
}

#[test]
fn epr_05_existing_backend_registration_is_transactional() {
    let mut registry = default_backend_registry();
    let before = registry.clone();
    let mut duplicate = registry.iter().next().unwrap().clone();
    duplicate.name = "rejected replacement".into();
    duplicate.revision = "unaccepted-v2".into();
    assert!(registry.insert(duplicate).is_err());
    assert_eq!(registry, before);
}
