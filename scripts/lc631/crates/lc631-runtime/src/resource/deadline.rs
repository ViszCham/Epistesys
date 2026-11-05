//! Narrow the existing run owner without new identities, quota or reservations.
#![forbid(unsafe_code)]
use super::*;
impl RunResources {
    pub fn narrow_deadline(&self, external: Deadline) -> Result<DeadlineNarrowing, ResourceError> {
        if self.closed {
            return Err(ResourceError::UnknownRun);
        }
        let ledger = self
            .root
            .ledger
            .lock()
            .map_err(|_| ResourceError::Poisoned)?;
        let run = ledger
            .runs
            .iter()
            .find(|run| run.id == self.run_id)
            .ok_or(ResourceError::UnknownRun)?;
        run.cancellation.check()?;
        run.deadline.narrow(external)
    }
}
