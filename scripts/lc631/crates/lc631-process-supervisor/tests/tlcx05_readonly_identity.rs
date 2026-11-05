use lc631_process_supervisor::filesystem::{DirectoryAnchor, ReadOnlyAnchoredFile};
use std::path::Path;

fn read_only_type(_: &ReadOnlyAnchoredFile) {}

#[test]
fn tlcx05_readonly_existing_source_has_read_and_identity_without_repair() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let anchor = DirectoryAnchor::open_existing(root).unwrap();
    let mut file = anchor.open_read_only(Path::new("Cargo.toml")).unwrap();
    read_only_type(&file);
    file.validate_current().unwrap();
    let expected = std::fs::read(root.join("Cargo.toml")).unwrap();
    assert_eq!(file.read_bounded(4096).unwrap(), expected);
}
