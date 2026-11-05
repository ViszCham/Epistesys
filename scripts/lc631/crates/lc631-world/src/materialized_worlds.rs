//! Preparatory bounded worlds over admitted contexts and actual borrowed payloads.
//! Byte distinctness is structural only. All execution reports remain unobserved
//! until a runtime-owned observation bridge consumes authenticated native results.
use crate::context_graph::ContextGraph;
use lc631_core::context::{ContextRevision, ContextState};
use std::mem::size_of;
use std::sync::atomic::{AtomicU64, Ordering};

pub const MATERIALIZED_BASE_SLOTS: usize = 256;
pub const MATERIALIZED_LIVE_LIMIT: usize = 2_048;
const MAX_REFERENCED_BYTES: usize = 128 * 1024 * 1024;
static NEXT_ARENA: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaterializedLimits {
    pub max_live: usize,
    pub max_attempts: u64,
    pub max_payload_bytes: usize,
    pub max_referenced_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaterializedError {
    InvalidLimits,
    AllocationFailed,
    DomainExhausted,
    AttemptBudget,
    UnknownContext,
    InvalidPayload,
    PayloadBudget,
    LiveCapacity,
    ForeignArena,
    StaleKey,
    QueryMismatch,
    ResultAlreadyPresent,
}

/// Report container: exact payload bytes do not certify their semantic meaning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundWorldQuery<'a> {
    context: ContextRevision,
    profile: &'a [u8],
    bytes: &'a [u8],
}

impl<'a> BoundWorldQuery<'a> {
    pub const fn reported(context: ContextRevision, profile: &'a [u8], bytes: &'a [u8]) -> Self {
        Self {
            context,
            profile,
            bytes,
        }
    }
    pub const fn context(self) -> ContextRevision {
        self.context
    }
    pub const fn profile_bytes(self) -> &'a [u8] {
        self.profile
    }
    pub const fn query_bytes(self) -> &'a [u8] {
        self.bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportedRunState {
    Completed,
    Timeout,
    Unsupported,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReportedWorldResult<'a> {
    query: BoundWorldQuery<'a>,
    state: ReportedRunState,
    model: Option<&'a [u8]>,
    result: &'a [u8],
}

impl<'a> ReportedWorldResult<'a> {
    pub const fn reported(
        query: BoundWorldQuery<'a>,
        state: ReportedRunState,
        model: Option<&'a [u8]>,
        result: &'a [u8],
    ) -> Self {
        Self {
            query,
            state,
            model,
            result,
        }
    }
    pub const fn state(self) -> ReportedRunState {
        self.state
    }
    pub const fn model_bytes(self) -> Option<&'a [u8]> {
        self.model
    }
    pub const fn result_bytes(self) -> &'a [u8] {
        self.result
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaterializedKey {
    arena: u64,
    slot: u16,
    generation: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaterializedAdmission {
    key: MaterializedKey,
    duplicate: bool,
}

impl MaterializedAdmission {
    pub const fn key(self) -> MaterializedKey {
        self.key
    }
    pub const fn is_duplicate(self) -> bool {
        self.duplicate
    }
}

pub struct MaterializedBranch<'a> {
    context: &'a ContextState,
    query: BoundWorldQuery<'a>,
    result: Option<ReportedWorldResult<'a>>,
    referenced_bytes: usize,
}

impl<'a> MaterializedBranch<'a> {
    pub const fn context(&self) -> &'a ContextState {
        self.context
    }
    pub const fn query(&self) -> BoundWorldQuery<'a> {
        self.query
    }
    pub const fn reported_result(&self) -> Option<ReportedWorldResult<'a>> {
        self.result
    }
}

struct Slot<'a> {
    generation: u32,
    retired: bool,
    branch: Option<MaterializedBranch<'a>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionCoverage {
    /// Caller reports never increment an observed-execution count.
    Unobserved { reported_completed: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BaseCoverage {
    Insufficient { required: usize, actual: usize },
    StructurallyMaterialized { actual: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaterializedCoverage {
    pub attempted: u64,
    pub aliases: u64,
    pub structural_live: usize,
    pub semantic_distinct_unconfirmed: usize,
    pub execution: ExecutionCoverage,
    pub base: BaseCoverage,
    pub referenced_bytes: usize,
    pub reserved_slot_bytes: usize,
}

/// Single owner; graph and all payloads outlive the store. No persistent handles,
/// mutation permits, solver receipt issuers, truth or metric scores are exposed.
pub struct MaterializedWorlds<'a> {
    graph: &'a ContextGraph,
    limits: MaterializedLimits,
    arena: u64,
    slots: Vec<Slot<'a>>,
    attempted: u64,
    aliases: u64,
    referenced_bytes: usize,
}

impl<'a> MaterializedWorlds<'a> {
    pub fn new(
        graph: &'a ContextGraph,
        limits: MaterializedLimits,
    ) -> Result<Self, MaterializedError> {
        if limits.max_live == 0
            || limits.max_live > MATERIALIZED_LIVE_LIMIT
            || limits.max_attempts == 0
            || limits.max_payload_bytes == 0
            || limits.max_payload_bytes > 16 * 1024 * 1024
            || limits.max_referenced_bytes == 0
            || limits.max_referenced_bytes > MAX_REFERENCED_BYTES
        {
            return Err(MaterializedError::InvalidLimits);
        }
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(limits.max_live)
            .map_err(|_| MaterializedError::AllocationFailed)?;
        let arena = NEXT_ARENA
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                next.checked_add(1)
            })
            .map_err(|_| MaterializedError::DomainExhausted)?;
        Ok(Self {
            graph,
            limits,
            arena,
            slots,
            attempted: 0,
            aliases: 0,
            referenced_bytes: 0,
        })
    }

    pub fn admit(
        &mut self,
        query: BoundWorldQuery<'a>,
    ) -> Result<MaterializedAdmission, MaterializedError> {
        if self.attempted >= self.limits.max_attempts {
            return Err(MaterializedError::AttemptBudget);
        }
        self.attempted += 1; // guarded by a finite u64 upper bound above
        let context = self
            .graph
            .get(query.context)
            .ok_or(MaterializedError::UnknownContext)?;
        self.payload_len(query.profile)?;
        self.payload_len(query.bytes)?;
        for (index, slot) in self.slots.iter().enumerate() {
            if slot
                .branch
                .as_ref()
                .is_some_and(|branch| branch.query == query)
            {
                self.aliases += 1; // aliases <= attempted <= max_attempts
                return Ok(MaterializedAdmission {
                    key: MaterializedKey {
                        arena: self.arena,
                        slot: index as u16,
                        generation: slot.generation,
                    },
                    duplicate: true,
                });
            }
        }
        let bytes = query
            .profile
            .len()
            .checked_add(query.bytes.len())
            .ok_or(MaterializedError::PayloadBudget)?;
        let total = self.admit_bytes(bytes)?;
        let index = match self
            .slots
            .iter()
            .position(|slot| slot.branch.is_none() && !slot.retired)
        {
            Some(index) => index,
            None if self.slots.len() < self.limits.max_live => {
                self.slots.push(Slot {
                    generation: 0,
                    retired: false,
                    branch: None,
                });
                self.slots.len() - 1
            }
            None => return Err(MaterializedError::LiveCapacity),
        };
        let slot = &mut self.slots[index];
        slot.branch = Some(MaterializedBranch {
            context,
            query,
            result: None,
            referenced_bytes: bytes,
        });
        self.referenced_bytes = total;
        Ok(MaterializedAdmission {
            key: MaterializedKey {
                arena: self.arena,
                slot: index as u16,
                generation: slot.generation,
            },
            duplicate: false,
        })
    }

    pub fn get(&self, key: MaterializedKey) -> Result<&MaterializedBranch<'a>, MaterializedError> {
        self.slots[self.index(key)?]
            .branch
            .as_ref()
            .ok_or(MaterializedError::StaleKey)
    }

    pub fn attach_reported(
        &mut self,
        key: MaterializedKey,
        result: ReportedWorldResult<'a>,
    ) -> Result<(), MaterializedError> {
        let index = self.index(key)?;
        let branch = self.get(key)?;
        if branch.query != result.query {
            return Err(MaterializedError::QueryMismatch);
        }
        if branch.result.is_some() {
            return Err(MaterializedError::ResultAlreadyPresent);
        }
        let result_bytes = self.payload_len(result.result)?;
        let model_bytes = match result.model {
            Some(model) => self.payload_len(model)?,
            None => 0,
        };
        let bytes = result_bytes
            .checked_add(model_bytes)
            .ok_or(MaterializedError::PayloadBudget)?;
        let total = self.admit_bytes(bytes)?;
        let new_branch_bytes = branch
            .referenced_bytes
            .checked_add(bytes)
            .ok_or(MaterializedError::PayloadBudget)?;
        let branch = self.slots[index]
            .branch
            .as_mut()
            .ok_or(MaterializedError::StaleKey)?;
        branch.result = Some(result);
        branch.referenced_bytes = new_branch_bytes;
        self.referenced_bytes = total;
        Ok(())
    }

    pub fn remove(&mut self, key: MaterializedKey) -> Result<(), MaterializedError> {
        let index = self.index(key)?;
        let bytes = self.get(key)?.referenced_bytes;
        let total = self
            .referenced_bytes
            .checked_sub(bytes)
            .ok_or(MaterializedError::PayloadBudget)?;
        let slot = &mut self.slots[index];
        slot.branch = None;
        match slot.generation.checked_add(1) {
            Some(next) => slot.generation = next,
            None => slot.retired = true,
        }
        self.referenced_bytes = total;
        Ok(())
    }

    pub fn coverage(&self) -> MaterializedCoverage {
        let live = self
            .slots
            .iter()
            .filter(|slot| slot.branch.is_some())
            .count();
        let reported_completed = self
            .slots
            .iter()
            .filter(|slot| {
                slot.branch
                    .as_ref()
                    .and_then(|branch| branch.result)
                    .is_some_and(|result| result.state == ReportedRunState::Completed)
            })
            .count();
        MaterializedCoverage {
            attempted: self.attempted,
            aliases: self.aliases,
            structural_live: live,
            semantic_distinct_unconfirmed: live,
            execution: ExecutionCoverage::Unobserved { reported_completed },
            base: if live < MATERIALIZED_BASE_SLOTS {
                BaseCoverage::Insufficient {
                    required: MATERIALIZED_BASE_SLOTS,
                    actual: live,
                }
            } else {
                BaseCoverage::StructurallyMaterialized { actual: live }
            },
            referenced_bytes: self.referenced_bytes,
            reserved_slot_bytes: self.slots.capacity() * size_of::<Slot<'a>>(),
        }
    }

    fn index(&self, key: MaterializedKey) -> Result<usize, MaterializedError> {
        if key.arena != self.arena {
            return Err(MaterializedError::ForeignArena);
        }
        let index = usize::from(key.slot);
        let slot = self.slots.get(index).ok_or(MaterializedError::StaleKey)?;
        if slot.retired || slot.generation != key.generation || slot.branch.is_none() {
            return Err(MaterializedError::StaleKey);
        }
        Ok(index)
    }

    fn payload_len(&self, bytes: &[u8]) -> Result<usize, MaterializedError> {
        if bytes.is_empty() || bytes.len() > self.limits.max_payload_bytes {
            return Err(MaterializedError::InvalidPayload);
        }
        Ok(bytes.len())
    }

    fn admit_bytes(&self, bytes: usize) -> Result<usize, MaterializedError> {
        let total = self
            .referenced_bytes
            .checked_add(bytes)
            .ok_or(MaterializedError::PayloadBudget)?;
        if total > self.limits.max_referenced_bytes {
            return Err(MaterializedError::PayloadBudget);
        }
        Ok(total)
    }
}
