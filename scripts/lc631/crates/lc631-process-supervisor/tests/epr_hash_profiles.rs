use sha2::{Digest, Sha256};

fn digest(input: &[u8]) -> String {
    Sha256::digest(input)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn optimized_test_backend_preserves_standard_sha256_vectors_and_rejects_mutation() {
    for (input, expected) in [
        (
            Vec::new(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            b"abc".to_vec(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        (
            vec![b'a'; 1_000_000],
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0",
        ),
    ] {
        assert_eq!(digest(&input), expected);
        let mut changed = input;
        changed.push(1);
        assert_ne!(digest(&changed), expected);
    }
}
