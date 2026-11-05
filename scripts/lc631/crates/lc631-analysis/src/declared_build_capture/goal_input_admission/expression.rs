//! ConditionExpression is the actual recursive type. Recursion is represented
//! solely by borrowed child nodes for the parent's iterative cursor stack.
use super::{Field, Node, Object, Sequence};
use crate::ConditionExpression;
pub(super) fn field(value: &ConditionExpression, index: usize) -> Option<Field<'_>> {
    let (tag, operand) = match value {
        ConditionExpression::Predicate(v) => ("predicate", Node::Object(Object::Predicate(v))),
        ConditionExpression::All(v) => ("all", Node::Seq(Sequence::Expression(v))),
        ConditionExpression::Any(v) => ("any", Node::Seq(Sequence::Expression(v))),
        ConditionExpression::Not(v) => ("not", Node::Object(Object::Expression(v))),
        ConditionExpression::Unresolved(v) => ("unresolved", Node::Object(Object::Predicate(v))),
    };
    match index {
        0 => Some(Field {
            key: "kind",
            value: Node::Str(tag),
        }),
        1 => Some(Field {
            key: "operands",
            value: operand,
        }),
        _ => None,
    }
}
