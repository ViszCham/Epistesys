//! Explicit, one-shot persistence of a fully verified legacy JSON chain.

use super::legacy::{LegacyHoldReason, LegacyJsonMigration};
use super::persistence::{persist_segment, JournalPersistenceError, PersistedJournalSegment};
use super::types::{JournalCursor, JournalError};
use lc631_host::HostPersistentStoreOwner;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LegacyChainPersistenceDecision {
    Hold {
        reason: LegacyHoldReason,
    },
    Persisted {
        legacy_final_digest: String,
        record_count: usize,
        storage: PersistedJournalSegment,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LegacyChainPersistenceError {
    Migration(JournalError),
    Persistence(JournalPersistenceError),
}

/// Verify the entire caller-supplied legacy chain before one binary write.
///
/// This function does not read the legacy files, infer their ordering, recheck
/// current source/authority, or establish an external head. The caller keeps
/// ownership of input acquisition and fresh-resume validation.
pub fn verify_and_persist_legacy_chain(
    owner: &mut HostPersistentStoreOwner<'_>,
    records: &[&[u8]],
    expected_predecessor: JournalCursor,
) -> Result<LegacyChainPersistenceDecision, LegacyChainPersistenceError> {
    let migration = LegacyJsonMigration::verify_and_migrate(records)
        .map_err(LegacyChainPersistenceError::Migration)?;
    match migration {
        LegacyJsonMigration::Hold { reason } => Ok(LegacyChainPersistenceDecision::Hold { reason }),
        LegacyJsonMigration::MigratedUnanchored {
            binary_segment,
            record_count,
            final_legacy_digest,
        } => {
            let storage = persist_segment(owner, &binary_segment, expected_predecessor)
                .map_err(LegacyChainPersistenceError::Persistence)?;
            Ok(LegacyChainPersistenceDecision::Persisted {
                legacy_final_digest: final_legacy_digest,
                record_count,
                storage,
            })
        }
    }
}
