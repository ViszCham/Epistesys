//! Expected E0502 at epoch: DependencyPath retains the actual state borrow.
#![forbid(unsafe_code)]
#![allow(dead_code)]
use lc631_runtime::contextual_distillation::{ContextualDistillation, NextRelationQuery};
use lc631_runtime::contextual_distillation::consumer::{consume_dependency_path, revalidate_next_query};

fn reject(state: &mut ContextualDistillation, request: NextRelationQuery) {
    let token = revalidate_next_query(state, request, 1000).unwrap();
    let path = consume_dependency_path(token).unwrap();
    let _ = state.epoch(&[]);
    std::hint::black_box(path.nodes());
}
fn main() {}
