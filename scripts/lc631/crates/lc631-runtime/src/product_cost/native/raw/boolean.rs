//! Structural postorder of actual NORMALIZED AST; never invent discarded raw AST.
use super::*;
use lc631_solvers::typed_query::BoolExpr;
pub(super) fn flatten(expr: &BoolExpr, max: usize) -> Result<Term, CostError> {
    fn walk(
        e: &BoolExpr,
        nodes: &mut Vec<Term>,
        max: usize,
        depth: usize,
    ) -> Result<usize, CostError> {
        if depth > 32 || nodes.len() >= max {
            return Err(CostError::UnsupportedProfile);
        }
        let node = match e {
            BoolExpr::Constant(b) => app("bool-constant-v1", vec![Term::Bool(*b)]),
            BoolExpr::Variable(v) => app("bool-variable-v1", vec![int(v.get() as i128)]),
            BoolExpr::Not(a) => {
                let i = walk(a, nodes, max, depth + 1)?;
                app("bool-not-v1", vec![int(i as i128)])
            }
            BoolExpr::And(parts) | BoolExpr::Or(parts) => {
                let mut children = Vec::new();
                for p in parts {
                    children.push(int(walk(p, nodes, max, depth + 1)? as i128));
                }
                app(
                    if matches!(e, BoolExpr::And(_)) {
                        "bool-and-v1"
                    } else {
                        "bool-or-v1"
                    },
                    vec![app("indices-v1", children)],
                )
            }
            _ => return Err(CostError::UnsupportedProfile),
        };
        if nodes.len() >= max {
            return Err(CostError::UnsupportedProfile);
        }
        let i = nodes.len();
        nodes.push(node);
        Ok(i)
    }
    let mut nodes = Vec::new();
    let root = walk(expr, &mut nodes, max, 0)?;
    Ok(app(
        "bool-ast-v1",
        vec![app("nodes-v1", nodes), int(root as i128)],
    ))
}
