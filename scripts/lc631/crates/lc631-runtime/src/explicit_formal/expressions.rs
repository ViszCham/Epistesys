//! Small typed expression fragment. No let/quantifier/function/nonlinear guess.
use super::{commands::Builder, FormalError, FormalErrorKind, FormalSymbol, ResidualReason};
use lc631_solvers::{
    protocol::sexpr::NodeId,
    typed_query::{BoolExpr, IntExpr},
};

impl Builder<'_, '_> {
    pub(super) fn boolean(&mut self, node: NodeId, depth: usize) -> Result<BoolExpr, FormalError> {
        self.visit(node, depth)?;
        if let Some(atom) = self.atom(node) {
            return match atom {
                "true" => Ok(BoolExpr::Constant(true)),
                "false" => Ok(BoolExpr::Constant(false)),
                _ => match self.symbol(node)? {
                    FormalSymbol::Boolean(id) => Ok(BoolExpr::Variable(id)),
                    FormalSymbol::Integer(_) => {
                        Err(self.error(node, FormalErrorKind::SortMismatch))
                    }
                },
            };
        }
        let fields = self.children(node)?;
        let head = fields
            .first()
            .and_then(|n| self.atom(*n))
            .ok_or_else(|| self.error(node, FormalErrorKind::MalformedCommand))?;
        match head {
            "not" if fields.len() == 2 => {
                Ok(BoolExpr::Not(Box::new(self.boolean(fields[1], depth + 1)?)))
            }
            "and" | "or" => {
                if fields.len() < 3 {
                    return Err(self.error(node, FormalErrorKind::MalformedCommand));
                }
                let conjunction = head == "and";
                let mut items = Vec::new();
                items
                    .try_reserve_exact(fields.len() - 1)
                    .map_err(|_| self.error(node, FormalErrorKind::AllocationFailed))?;
                for child in &fields[1..] {
                    items.push(self.boolean(*child, depth + 1)?);
                }
                if conjunction {
                    Ok(BoolExpr::And(items))
                } else {
                    Ok(BoolExpr::Or(items))
                }
            }
            "=" | "<=" | "<" | ">=" | ">" if fields.len() == 3 => {
                let operator = match head {
                    "=" => 0,
                    "<=" => 1,
                    "<" => 2,
                    ">=" => 3,
                    _ => 4,
                };
                // Boolean equality is outside this first typed-core fragment.
                if fields[1..]
                    .iter()
                    .any(|child| matches!(self.symbol(*child), Ok(FormalSymbol::Boolean(_))))
                {
                    return Err(self.error(
                        node,
                        FormalErrorKind::Unsupported(ResidualReason::UnsupportedExpression),
                    ));
                }
                let a = self.integer(fields[1], depth + 1)?;
                let b = self.integer(fields[2], depth + 1)?;
                Ok(match operator {
                    0 => BoolExpr::Equal(a, b),
                    1 => BoolExpr::LessEqual(a, b),
                    2 => BoolExpr::LessThan(a, b),
                    3 => BoolExpr::LessEqual(b, a),
                    _ => BoolExpr::LessThan(b, a),
                })
            }
            "not" | "=" | "<=" | "<" | ">=" | ">" => {
                Err(self.error(node, FormalErrorKind::MalformedCommand))
            }
            _ => Err(self.error(
                node,
                FormalErrorKind::Unsupported(ResidualReason::UnsupportedExpression),
            )),
        }
    }

    pub(super) fn integer(&mut self, node: NodeId, depth: usize) -> Result<IntExpr, FormalError> {
        self.visit(node, depth)?;
        if let Some(atom) = self.atom(node) {
            if decimal(atom) {
                let value = atom
                    .parse::<i64>()
                    .map_err(|_| self.error(node, FormalErrorKind::NumericOutOfRange))?;
                return Ok(IntExpr::Constant(value));
            }
            return match self.symbol(node)? {
                FormalSymbol::Integer(id) => Ok(IntExpr::Variable(id)),
                FormalSymbol::Boolean(_) => Err(self.error(node, FormalErrorKind::SortMismatch)),
            };
        }
        let fields = self.children(node)?;
        let head = fields
            .first()
            .and_then(|n| self.atom(*n))
            .ok_or_else(|| self.error(node, FormalErrorKind::MalformedCommand))?;
        match head {
            "+" => {
                if fields.len() < 3 {
                    return Err(self.error(node, FormalErrorKind::MalformedCommand));
                }
                let mut items = Vec::new();
                items
                    .try_reserve_exact(fields.len() - 1)
                    .map_err(|_| self.error(node, FormalErrorKind::AllocationFailed))?;
                for child in &fields[1..] {
                    items.push(self.integer(*child, depth + 1)?);
                }
                Ok(IntExpr::Add(items))
            }
            "-" if fields.len() == 2 => {
                if let Some(raw) = self.atom(fields[1]).filter(|text| decimal(text)) {
                    let magnitude = raw
                        .parse::<u64>()
                        .map_err(|_| self.error(node, FormalErrorKind::NumericOutOfRange))?;
                    let value = i64::try_from(-i128::from(magnitude))
                        .map_err(|_| self.error(node, FormalErrorKind::NumericOutOfRange))?;
                    return Ok(IntExpr::Constant(value));
                }
                Ok(IntExpr::Negate(Box::new(
                    self.integer(fields[1], depth + 1)?,
                )))
            }
            "-" if fields.len() == 3 => Ok(IntExpr::Subtract(
                Box::new(self.integer(fields[1], depth + 1)?),
                Box::new(self.integer(fields[2], depth + 1)?),
            )),
            "-" => Err(self.error(node, FormalErrorKind::MalformedCommand)),
            _ => Err(self.error(
                node,
                FormalErrorKind::Unsupported(ResidualReason::UnsupportedExpression),
            )),
        }
    }
}
fn decimal(text: &str) -> bool {
    !text.is_empty()
        && (text.len() == 1 || !text.starts_with('0'))
        && text.bytes().all(|b| b.is_ascii_digit())
}
