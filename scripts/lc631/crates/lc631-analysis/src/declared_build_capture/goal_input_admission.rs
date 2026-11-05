//! Goal-only bounds admission, never identity/meaning/authority validation.
//!
//! PRIVATE CALL CONTRACT: the only production caller is the narrow
//! SourceReadBinding::check_goal_profile_inputs forwarding seam. It holds its
//! actual 64KiB ResourceClaim before this function reserves any cursor storage,
//! and checks the same claim afterwards. The test-only funded entry mirrors
//! that call graph. Do not reexport this function or add an unfunded public
//! entry; no duplicate128KiB claim is acquired here. Existing wide capture is
//! unchanged and independently verifies the actual pipeline after admission.
#![forbid(unsafe_code)]
#[path = "goal_input_admission/budget.rs"]
mod budget;
#[path = "goal_input_admission/completion.rs"]
mod completion;
#[path = "goal_input_admission/distillation.rs"]
mod distillation;
#[path = "goal_input_admission/expression.rs"]
mod expression;
#[path = "goal_input_admission/geometry.rs"]
mod geometry;
#[path = "goal_input_admission/grammar.rs"]
mod grammar;
#[path = "goal_input_admission/language.rs"]
mod language;
#[path = "goal_input_admission/objects.rs"]
mod objects;
#[path = "goal_input_admission/pipeline.rs"]
mod pipeline;
#[path = "goal_input_admission/program.rs"]
mod program;
#[path = "goal_input_admission/sequences.rs"]
mod sequences;
#[path = "goal_input_admission/tags_completion.rs"]
mod tags_completion;
#[path = "goal_input_admission/tags_grammar.rs"]
mod tags_grammar;
#[path = "goal_input_admission/tags_pipeline.rs"]
mod tags_pipeline;
#[path = "goal_input_admission/tags_state.rs"]
mod tags_state;
#[path = "goal_input_admission/tl.rs"]
mod tl;
use crate::declared_build_capture::SnapshotCaptureError as Error;
use lc631_substrate::resource::{ResourceError, ResourceReservationView};
use objects::Object;
use sequences::Sequence;

#[derive(Clone, Copy)]
enum Node<'a> {
    Str(&'a str),
    Number,
    Boolean,
    Null,
    Object(Object<'a>),
    Seq(Sequence<'a>),
}
struct Field<'a> {
    key: &'static str,
    value: Node<'a>,
}
#[derive(Clone, Copy)]
struct Frame<'a> {
    node: Node<'a>,
    depth: usize,
    next: usize,
}
const FRAMES: usize = 256;
const SCRATCH: usize = 64 * 1024;

// Closed private numeric family, NOT a serialization or visitor hook. A field
// changing into String/BigInt/another type fails compilation rather than being
// silently charged as a primitive. Every present representation is <=32 JSON
// token bytes. This trait is not exposed outside the private worker subtree.
trait FixedNumber {}
impl FixedNumber for u8 {}
impl FixedNumber for u16 {}
impl FixedNumber for u32 {}
impl FixedNumber for u64 {}
impl FixedNumber for usize {}
impl FixedNumber for i32 {}
impl FixedNumber for lc631_core::ObligationId {}
impl FixedNumber for lc631_tldg::SyntaxNodeId {}
impl FixedNumber for lc631_tl::SourceLedgerId {}
impl FixedNumber for lc631_tl::TargetClaimId {}
impl FixedNumber for lc631_tl::ProjectionEdgeId {}
fn number<T: FixedNumber>(_: &T) -> Node<'_> {
    Node::Number
}
fn boolean(_: &bool) -> Node<'_> {
    Node::Boolean
}

pub(crate) fn check(
    pipeline: &crate::DgclPipelineReport,
    resources: &ResourceReservationView<'_>,
) -> Result<(), Error> {
    resources.check()?;
    // The two root serde(skip) fields are explicitly bounded, not authenticated
    // or serialized. The full all-field pattern lives in pipeline::pipeline.
    if pipeline.immutable_core_digest.len() > 71
        || pipeline.source_store.text().len() > budget::LEAF
    {
        return Err(Error::InventoryLimit);
    }
    let bytes = std::mem::size_of::<Frame<'_>>()
        .checked_mul(FRAMES)
        .ok_or(Error::InventoryLimit)?;
    if bytes > SCRATCH {
        return Err(Error::InventoryLimit);
    }
    let mut stack = Vec::<Frame<'_>>::new();
    stack
        .try_reserve_exact(FRAMES)
        .map_err(|_| Error::Resource(ResourceError::AllocationFailed))?;
    if stack.capacity() > FRAMES {
        return Err(Error::Resource(ResourceError::AllocationFailed));
    }
    let root = Node::Object(Object::Pipeline(pipeline));
    let mut budget = budget::Budget::new();
    budget.enter(root, 0, resources)?;
    stack.push(Frame {
        node: root,
        depth: 0,
        next: 0,
    });
    while let Some(mut frame) = stack.pop() {
        let length = match frame.node {
            Node::Object(v) => v.len(),
            Node::Seq(v) => v.len(),
            Node::Str(_) | Node::Number | Node::Boolean | Node::Null => 0,
        };
        if frame.next == length {
            continue;
        }
        if frame.next > length {
            return Err(Error::InventoryLimit);
        }
        let depth = frame.depth.checked_add(1).ok_or(Error::InventoryLimit)?;
        budget.pre_child(depth)?;
        let child = child(frame.node, frame.next, depth, &mut budget, resources)?;
        frame.next = frame.next.checked_add(1).ok_or(Error::InventoryLimit)?;
        // Funded fixed capacity is checked BEFORE either enqueue; only one
        // child and its suspended parent are live, never all wide-array items.
        if stack.len().checked_add(2).ok_or(Error::InventoryLimit)? > FRAMES {
            return Err(Error::InventoryLimit);
        }
        budget.enter(child, depth, resources)?;
        stack.push(frame);
        stack.push(Frame {
            node: child,
            depth,
            next: 0,
        });
    }
    resources.check()?;
    Ok(())
}
fn child<'a>(
    node: Node<'a>,
    index: usize,
    depth: usize,
    budget: &mut budget::Budget,
    resources: &ResourceReservationView<'_>,
) -> Result<Node<'a>, Error> {
    match node {
        Node::Object(v) => {
            let field = v.field(index).ok_or(Error::InventoryLimit)?;
            budget.enter(Node::Str(field.key), depth, resources)?;
            budget.pre_child(depth)?;
            Ok(field.value)
        }
        Node::Seq(v) => v.item(index).ok_or(Error::InventoryLimit),
        Node::Str(_) | Node::Number | Node::Boolean | Node::Null => Err(Error::InventoryLimit),
    }
}

#[cfg(test)]
#[path = "goal_input_admission/tests.rs"]
mod tests;
