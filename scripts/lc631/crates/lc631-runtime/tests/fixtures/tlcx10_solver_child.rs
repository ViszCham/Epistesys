//! Test-only child for bounded process-adapter integration.
//! This emits a fixed solver-looking status; it is not a solver or proof oracle.

use std::io::{self, Read, Write};

const MAX_INPUT_BYTES: usize = 4096;

fn main() {
    let mut input = Vec::new();
    let mut limited = io::stdin().take((MAX_INPUT_BYTES + 1) as u64);
    if limited.read_to_end(&mut input).is_err() || input.len() > MAX_INPUT_BYTES {
        std::process::exit(2);
    }

    let status = if input == b"(check-sat)\n" {
        b"sat\n".as_slice()
    } else {
        b"unknown\n".as_slice()
    };
    if io::stdout().lock().write_all(status).is_err() {
        std::process::exit(3);
    }
}
