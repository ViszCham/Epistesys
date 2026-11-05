use lc631_process_supervisor::filesystem::DirectoryAnchor;
use std::path::Path;

#[test]
fn tlcx05_held_directory_identity_can_be_revalidated_without_mutation() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let anchor = DirectoryAnchor::open_existing(root).unwrap();
    anchor.validate_current().unwrap();
    assert_eq!(anchor.path(), root);
}

#[test]
fn tlcx05_held_file_identity_can_be_revalidated_without_write_or_repair() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let anchor = DirectoryAnchor::open_existing(root).unwrap();
    let file = anchor.open_read_write(Path::new("Cargo.toml")).unwrap();
    let before = file.identity();
    let bytes = file.metadata().unwrap().len();
    file.validate_current().unwrap();
    assert_eq!(file.identity(), before);
    assert_eq!(file.metadata().unwrap().len(), bytes);
}
