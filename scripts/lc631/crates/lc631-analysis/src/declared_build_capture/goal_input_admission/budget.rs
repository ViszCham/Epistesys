//! Pre-visit quotas for the fixed typed cursor. No scanning or formatting.
use super::{Error, Node};
use lc631_substrate::resource::ResourceReservationView;
pub(super) const NODES: usize = 32_768;
pub(super) const CONTAINER: usize = 16_384;
pub(super) const DEPTH: usize = 64;
pub(super) const LEAF: usize = 262_144;
pub(super) const AGGREGATE: usize = 1_048_576;
pub(super) const ENCODED: usize = 8_388_608;
pub(super) struct Budget {
    pub(super) nodes: usize,
    pub(super) strings: usize,
    pub(super) encoded: usize,
}
fn add(total: usize, amount: usize, cap: usize) -> Result<usize, Error> {
    let next = total.checked_add(amount).ok_or(Error::InventoryLimit)?;
    if next > cap {
        return Err(Error::InventoryLimit);
    }
    Ok(next)
}
impl Budget {
    pub(super) const fn new() -> Self {
        Self {
            nodes: 0,
            strings: 0,
            encoded: 0,
        }
    }
    pub(super) fn pre_child(&self, depth: usize) -> Result<(), Error> {
        if depth > DEPTH || self.nodes >= NODES {
            return Err(Error::InventoryLimit);
        }
        Ok(())
    }
    pub(super) fn enter(
        &mut self,
        node: Node<'_>,
        depth: usize,
        resources: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        self.pre_child(depth)?;
        let nodes = add(self.nodes, 1, NODES)?;
        let (string_bytes, encoded_bytes) = match node {
            Node::Str(v) => {
                let length = v.len();
                if length > LEAF {
                    return Err(Error::InventoryLimit);
                }
                let escaped = length
                    .checked_mul(6)
                    .and_then(|n| n.checked_add(2))
                    .ok_or(Error::InventoryLimit)?;
                (length, escaped)
            }
            Node::Number => (0, 32),
            Node::Boolean => (0, 5),
            Node::Null => (0, 4),
            Node::Object(v) => {
                let len = v.len();
                self.container(len, depth, nodes, 2)?;
                // Conservative one comma and colon per field, with braces.
                let overhead = len
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(2))
                    .ok_or(Error::InventoryLimit)?;
                (0, overhead)
            }
            Node::Seq(v) => {
                let len = v.len();
                self.container(len, depth, nodes, 1)?;
                (0, len.checked_add(2).ok_or(Error::InventoryLimit)?)
            }
        };
        let strings = add(self.strings, string_bytes, AGGREGATE)?;
        let encoded = add(self.encoded, encoded_bytes, ENCODED)?;
        // All quotas were checked before this node is visited/enqueued. The
        // counters are local to an invocation, not a success/report producer.
        self.nodes = nodes;
        self.strings = strings;
        self.encoded = encoded;
        if nodes % 64 == 0 {
            resources.check()?;
        }
        Ok(())
    }
    fn container(
        &self,
        length: usize,
        depth: usize,
        nodes: usize,
        per_child: usize,
    ) -> Result<(), Error> {
        if length > CONTAINER || (length != 0 && depth >= DEPTH) {
            return Err(Error::InventoryLimit);
        }
        let children = length.checked_mul(per_child).ok_or(Error::InventoryLimit)?;
        add(nodes, children, NODES)?;
        Ok(())
    }
}
