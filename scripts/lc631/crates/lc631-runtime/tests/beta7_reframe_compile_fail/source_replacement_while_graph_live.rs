//! Expected E0506 at source assignment; a missing import is not this negative.
#![forbid(unsafe_code)]
#![allow(dead_code)]
use lc631_core::context::{ContextRevision, SourceRevision};
use lc631_runtime::structural_gain_measurement::{ClosedGraph, Edge, Vertex};
use lc631_substrate::source::SourceStore;
use std::sync::Arc;

fn reject() {
    let mut source = SourceStore::new(Arc::<str>::from("Given immutable graph"), 1024).unwrap();
    let context = ContextRevision::original(SourceRevision::of(source.text()));
    let edges = [Edge(Vertex(0), Vertex(1)), Edge(Vertex(1), Vertex(2))];
    let graph = ClosedGraph::checked(&source, context, 3, &edges).unwrap();
    source = SourceStore::new(Arc::<str>::from("replacement"), 1024).unwrap();
    std::hint::black_box(graph.digest());
}
fn main() {}
