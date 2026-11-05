//! Private native creation-stage observation, never inferred from error text/kind.
use std::io;

pub(crate) enum SpawnFailure {
    BeforeChild(io::Error),
    MayHaveStarted(io::Error),
}
impl SpawnFailure {
    pub(crate) fn into_io(self) -> io::Error {
        match self {
            Self::BeforeChild(error) | Self::MayHaveStarted(error) => error,
        }
    }
}
