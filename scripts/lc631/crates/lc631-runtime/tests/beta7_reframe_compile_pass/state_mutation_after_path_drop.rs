//! Positive type control: state may advance after the checked path is dropped.
#![forbid(unsafe_code)]
#![allow(dead_code)]
use lc631_runtime::contextual_distillation::{ContextualDistillation, NextRelationQuery};
use lc631_runtime::contextual_distillation::consumer::{consume_dependency_path, revalidate_next_query};

fn control(state: &mut ContextualDistillation, request: NextRelationQuery) {
    let token = revalidate_next_query(state, request, 1000).unwrap();
    let path = consume_dependency_path(token).unwrap();
    std::hint::black_box(path.nodes());
    drop(path);
    let _ = state.epoch(&[]);
}
fn main() {}
