use super::{AspBinding, AspError, AspProgram, AtomId};
use crate::protocol::QueryResultKind;
use crate::registry::{BackendId, LogicProfile};
use crate::runner::{QuerySourceIdentity, SolverQuery};

#[derive(Debug)]
pub struct PreparedAspQuery {
    binding: AspBinding,
    payload: Vec<u8>,
}

impl PreparedAspQuery {
    pub const fn binding(&self) -> AspBinding {
        self.binding
    }
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
    pub fn solver_query(&self) -> Result<SolverQuery<'_>, AspError> {
        SolverQuery::checked(
            BackendId::Clingo,
            QuerySourceIdentity::from_bytes(*self.binding.context().source().digest().as_bytes()),
            LogicProfile::AspFiniteGroundNormal,
            QueryResultKind::StableModelExistence,
            &self.payload,
        )
        .map_err(|_| AspError::Query)
    }
}

impl AspProgram {
    /// Emit only generated symbols; solver options must request all models (`--models=0`).
    /// Costs remain separately evaluated finite objectives, not Clingo optimization.
    pub fn prepare(&self) -> Result<PreparedAspQuery, AspError> {
        let mut out = Writer {
            bytes: Vec::new(),
            max: self.limits.output,
        };
        out.push("% epistesys-ground-normal-v1 closed-world-default-negation\n% context ")?;
        out.hex(self.binding.context().interpretation_digest().as_bytes())?;
        out.push("\n% domain ")?;
        out.hex(self.binding.domain().as_bytes())?;
        out.push("\n% grounding-basis-declared ")?;
        out.hex(self.binding.grounding_basis().as_bytes())?;
        out.push("\n% finite-universe")?;
        for atom in &self.atoms {
            out.push(" ")?;
            out.atom(*atom)?;
        }
        out.push("\n% costs-enumeration-only")?;
        for cost in &self.costs {
            out.push(" ")?;
            out.atom(cost.atom)?;
            out.push(":")?;
            out.push(&cost.weight.to_string())?;
        }
        out.push("\n")?;
        for rule in &self.rules {
            if let Some(head) = rule.head {
                out.atom(head)?;
            }
            if rule.head.is_none() || !rule.positive.is_empty() || !rule.negative.is_empty() {
                out.push(" :- ")?;
                let mut separator = "";
                for atom in &rule.positive {
                    out.push(separator)?;
                    out.atom(*atom)?;
                    separator = ", ";
                }
                for atom in &rule.negative {
                    out.push(separator)?;
                    out.push("not ")?;
                    out.atom(*atom)?;
                    separator = ", ";
                }
            }
            out.push(".\n")?;
        }
        Ok(PreparedAspQuery {
            binding: self.binding,
            payload: out.bytes,
        })
    }
}

struct Writer {
    bytes: Vec<u8>,
    max: usize,
}

impl Writer {
    fn push(&mut self, value: &str) -> Result<(), AspError> {
        let length = self
            .bytes
            .len()
            .checked_add(value.len())
            .ok_or(AspError::ArithmeticOverflow)?;
        if length > self.max {
            return Err(AspError::OutputLimit);
        }
        self.bytes
            .try_reserve(value.len())
            .map_err(|_| AspError::AllocationFailed)?;
        self.bytes.extend_from_slice(value.as_bytes());
        Ok(())
    }
    fn atom(&mut self, atom: AtomId) -> Result<(), AspError> {
        self.push("a")?;
        self.push(&atom.get().to_string())
    }
    fn hex(&mut self, bytes: &[u8; 32]) -> Result<(), AspError> {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for byte in bytes {
            let pair = [HEX[usize::from(byte >> 4)], HEX[usize::from(byte & 15)]];
            let text = std::str::from_utf8(&pair).map_err(|_| AspError::OutputSyntax)?;
            self.push(text)?;
        }
        Ok(())
    }
}
