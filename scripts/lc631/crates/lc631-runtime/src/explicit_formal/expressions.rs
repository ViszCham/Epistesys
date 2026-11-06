//! Small typed expression fragment. No let/quantifier/function/nonlinear guess.
use super::{
    commands::Builder, DeclaredLogic, FormalError, FormalErrorKind, FormalSymbol, ResidualReason,
};
use lc631_solvers::{
    protocol::sexpr::NodeId,
    typed_query::{BoolExpr, IntExpr},
};

// Equality between Boolean operands is outside this profile. Validate those
// operands without retaining a Boolean AST for an unsupported comparison.
enum ComparisonOperand {
    Boolean,
    Integer(IntExpr),
}

impl Builder<'_, '_> {
    pub(super) fn boolean(&mut self, node: NodeId, depth: usize) -> Result<BoolExpr, FormalError> {
        self.visit(node, depth)?;
        if let Some(atom) = self.atom(node) {
            return match atom {
                "true" => Ok(BoolExpr::Constant(true)),
                "false" => Ok(BoolExpr::Constant(false)),
                number if decimal(number) => Err(self.error(node, FormalErrorKind::SortMismatch)),
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
                let left = self.comparison_operand(fields[1], depth + 1)?;
                let right = self.comparison_operand(fields[2], depth + 1)?;
                let (a, b) = match (left, right) {
                    (ComparisonOperand::Integer(a), ComparisonOperand::Integer(b)) => (a, b),
                    (ComparisonOperand::Boolean, ComparisonOperand::Boolean) if operator == 0 => {
                        return Err(self.error(
                            node,
                            FormalErrorKind::Unsupported(ResidualReason::UnsupportedExpression),
                        ));
                    }
                    _ => return Err(self.error(node, FormalErrorKind::SortMismatch)),
                };
                // The emitter uses QF_LIA for the conservative Boolean subset.
                // An integer comparison cannot cross that source-logic boundary.
                if self.logic != Some(DeclaredLogic::QfLia) {
                    return Err(self.error(
                        node,
                        FormalErrorKind::Unsupported(ResidualReason::UnsupportedExpression),
                    ));
                }
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

    fn comparison_operand(
        &mut self,
        node: NodeId,
        depth: usize,
    ) -> Result<ComparisonOperand, FormalError> {
        match self.atom(node) {
            Some("true" | "false") => {
                self.visit(node, depth)?;
                return Ok(ComparisonOperand::Boolean);
            }
            Some(number) if decimal(number) => {
                return self.integer(node, depth).map(ComparisonOperand::Integer);
            }
            Some(_) => {
                self.visit(node, depth)?;
                return Ok(match self.symbol(node)? {
                    FormalSymbol::Boolean(_) => ComparisonOperand::Boolean,
                    FormalSymbol::Integer(id) => ComparisonOperand::Integer(IntExpr::Variable(id)),
                });
            }
            None => {}
        }
        let head = self
            .doc
            .children(node)
            .and_then(|mut children| children.next())
            .and_then(|head| self.atom(head));
        match head {
            Some("+" | "-") => self.integer(node, depth).map(ComparisonOperand::Integer),
            _ => {
                self.boolean(node, depth)?;
                Ok(ComparisonOperand::Boolean)
            }
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
            if matches!(atom, "true" | "false") {
                return Err(self.error(node, FormalErrorKind::SortMismatch));
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
