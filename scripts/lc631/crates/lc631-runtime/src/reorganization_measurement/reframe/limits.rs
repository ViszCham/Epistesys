//! Logical registered limits; no cross-plan/OS/RSS enforcement is asserted.
use super::Error;
#[derive(Clone, Copy)]
pub struct ExactLimits {
    pub max_root_work: u64,
    pub max_query_work: u64,
    pub max_map_work: u64,
    pub max_total_work: u64,
    pub retained_ram: u64,
    pub scratch_ram: u64,
    pub selector_ram: u64,
}
impl ExactLimits {
    pub fn finite_path_v2() -> Self {
        Self {
            max_root_work: 4096,
            max_query_work: 4096,
            max_map_work: 4096,
            max_total_work: 262144,
            retained_ram: 65536,
            scratch_ram: 262144,
            selector_ram: 262144,
        }
    }
    pub(super) fn checked(self) -> Result<Self, Error> {
        if self.max_root_work == 0
            || self.max_root_work > 4096
            || self.max_query_work == 0
            || self.max_query_work > 4096
            || self.max_map_work == 0
            || self.max_map_work > 4096
            || self.max_total_work == 0
            || self.max_total_work > 262144
            || self.retained_ram != 65536
            || self.scratch_ram != 262144
            || self.selector_ram != 262144
        {
            return Err(Error::InvalidLimits);
        }
        Ok(self)
    }
}
