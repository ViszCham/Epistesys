//! The single bounded actual BoolExpr-to-native-Modal adapter; no text parser.
use super::*;
pub(super) struct Lowered {
    pub nodes: Vec<Formula>,
    pub root: u16,
    pub trace: Vec<NodeMap>,
}
pub(super) struct NodeMap {
    pub ordinal: u16,
    pub native: u16,
    pub operator: GrammarOperator,
    pub meaning: Option<u8>,
}
impl Lowered {
    pub fn check_map(&self) -> Result<(), Error> {
        let mut covered = [false; 128];
        if self.trace.is_empty() || self.trace.len() > 128 {
            return Err(Error::InvalidMap);
        }
        for t in &self.trace {
            let slot = usize::from(t.ordinal);
            if slot >= self.trace.len()
                || covered[slot]
                || usize::from(t.native) >= self.nodes.len()
            {
                return Err(Error::InvalidMap);
            }
            covered[slot] = true;
            match (t.operator, t.meaning) {
                (GrammarOperator::Variable, Some(_))
                | (
                    GrammarOperator::Constant
                    | GrammarOperator::Not
                    | GrammarOperator::And
                    | GrammarOperator::Or,
                    None,
                ) => {}
                _ => return Err(Error::InvalidMap),
            }
        }
        Ok(())
    }
}
/// Source occurrences and binary-expanded native nodes are distinct resources.
#[derive(Clone, Copy)]
pub(super) struct Dimensions {
    pub source: usize,
    pub native: usize,
}
pub(super) fn dimensions(e: &BoolExpr, depth: usize) -> Result<Dimensions, Error> {
    if depth > 32 {
        return Err(Error::LoweringBudget);
    }
    let result = match e {
        BoolExpr::Constant(_) | BoolExpr::Variable(_) => Dimensions {
            source: 1,
            native: 1,
        },
        BoolExpr::Not(c) => {
            let child = dimensions(c, depth + 1)?;
            Dimensions {
                source: child.source.checked_add(1).ok_or(Error::LoweringBudget)?,
                native: child.native.checked_add(1).ok_or(Error::LoweringBudget)?,
            }
        }
        BoolExpr::And(cs) | BoolExpr::Or(cs) => {
            if cs.is_empty() || cs.len() > 128 {
                return Err(Error::UnsupportedOperator);
            }
            // LTR binary fold emits one native connective per child AFTER the first.
            let mut total = Dimensions {
                source: 1,
                native: cs.len().checked_sub(1).ok_or(Error::LoweringBudget)?,
            };
            for c in cs {
                let child = dimensions(c, depth + 1)?;
                total.source = total
                    .source
                    .checked_add(child.source)
                    .ok_or(Error::LoweringBudget)?;
                total.native = total
                    .native
                    .checked_add(child.native)
                    .ok_or(Error::LoweringBudget)?;
                if total.source > 128 || total.native > 128 {
                    return Err(Error::LoweringBudget);
                }
            }
            total
        }
        BoolExpr::Equal(_, _) | BoolExpr::LessEqual(_, _) | BoolExpr::LessThan(_, _) => {
            return Err(Error::UnsupportedOperator)
        }
    };
    if result.source > 128 || result.native > 128 {
        return Err(Error::LoweringBudget);
    }
    Ok(result)
}
pub(super) fn lower(
    e: &BoolExpr,
    atoms: &[super::super::AtomMap],
    map: &[AtomCorrespondence],
) -> Result<Lowered, Error> {
    // Both exact dimensions are admitted BEFORE either payload Vec allocation.
    let size = dimensions(e, 1)?;
    let mut nodes = bounds::reserve(size.native, 128)?;
    let mut trace = bounds::reserve(size.source, 128)?;
    let mut next = 0;
    let root = append(e, atoms, map, &mut nodes, &mut trace, &mut next)?;
    let lowered = Lowered { nodes, root, trace };
    if lowered.nodes.len() != size.native || lowered.trace.len() != size.source {
        return Err(Error::InvalidMap);
    }
    lowered.check_map()?;
    Ok(lowered)
}
fn push(nodes: &mut Vec<Formula>, node: Formula) -> Result<u16, Error> {
    if nodes.len() >= nodes.capacity() || nodes.len() >= 128 {
        return Err(Error::LoweringBudget);
    }
    let id = u16::try_from(nodes.len()).map_err(|_| Error::LoweringBudget)?;
    nodes.push(node);
    Ok(id)
}
fn append(
    e: &BoolExpr,
    atoms: &[super::super::AtomMap],
    map: &[AtomCorrespondence],
    nodes: &mut Vec<Formula>,
    trace: &mut Vec<NodeMap>,
    next: &mut u16,
) -> Result<u16, Error> {
    let ordinal = *next;
    *next = next.checked_add(1).ok_or(Error::LoweringBudget)?;
    let (operator, meaning) = match e {
        BoolExpr::Constant(_) => (GrammarOperator::Constant, None),
        BoolExpr::Variable(id) => (
            GrammarOperator::Variable,
            Some(
                atoms
                    .iter()
                    .find(|a| a.local == *id)
                    .ok_or(Error::InvalidMap)?
                    .meaning,
            ),
        ),
        BoolExpr::Not(_) => (GrammarOperator::Not, None),
        BoolExpr::And(_) => (GrammarOperator::And, None),
        BoolExpr::Or(_) => (GrammarOperator::Or, None),
        BoolExpr::Equal(_, _) | BoolExpr::LessEqual(_, _) | BoolExpr::LessThan(_, _) => {
            return Err(Error::UnsupportedOperator)
        }
    };
    let node = match e {
        BoolExpr::Constant(false) => Formula::Bottom,
        BoolExpr::Constant(true) => Formula::Top,
        BoolExpr::Variable(id) => {
            let m = atoms
                .iter()
                .find(|m| m.local == *id)
                .ok_or(Error::InvalidMap)?;
            let slot = map
                .iter()
                .find(|v| v.meaning == m.meaning)
                .ok_or(Error::InvalidMap)?
                .chain_slot;
            Formula::Atom(slot)
        }
        BoolExpr::Not(c) => Formula::Not(append(c, atoms, map, nodes, trace, next)?),
        BoolExpr::And(cs) | BoolExpr::Or(cs) => {
            let mut values = cs.iter();
            let mut acc = append(
                values.next().ok_or(Error::UnsupportedOperator)?,
                atoms,
                map,
                nodes,
                trace,
                next,
            )?;
            for child in values {
                let right = append(child, atoms, map, nodes, trace, next)?;
                acc = push(
                    nodes,
                    if matches!(e, BoolExpr::And(_)) {
                        Formula::And(acc, right)
                    } else {
                        Formula::Or(acc, right)
                    },
                )?;
            }
            trace.push(NodeMap {
                ordinal,
                native: acc,
                operator,
                meaning,
            });
            return Ok(acc);
        }
        BoolExpr::Equal(_, _) | BoolExpr::LessEqual(_, _) | BoolExpr::LessThan(_, _) => {
            return Err(Error::UnsupportedOperator)
        }
    };
    let native = push(nodes, node)?;
    trace.push(NodeMap {
        ordinal,
        native,
        operator,
        meaning,
    });
    Ok(native)
}
