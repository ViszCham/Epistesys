//! Hand-authored executable K6 package. No production package generator/gold.
//! Tiny generic Given fields exercise A/P/I/R; this is NOT native Stage B closure.
use super::wire::{app, arg, child, encoded, int, literal, op, sym, Node};
use lc631_runtime::product_cost::{InlinePackage, ReportedFieldInput};

pub struct Fixture {
    pub adapter: Node,
    pub projection: Node,
    pub inverse: Node,
    pub helpers: Node,
    pub runtime: Node,
    pub dictionary: Node,
    pub fields: Node,
    pub declarations: Node,
}
pub struct Bytes {
    pub adapter: Vec<u8>,
    pub projection: Vec<u8>,
    pub inverse: Vec<u8>,
    pub helpers: Vec<u8>,
    pub runtime: Vec<u8>,
    pub dictionary: Vec<u8>,
    pub fields: Vec<u8>,
    pub declarations: Vec<u8>,
}
pub fn program(entry: &str, body: Node) -> Node {
    app("program-v1", vec![sym(entry), int(2), body])
}
pub fn operation(name: &str, arity: i128, body: Node) -> Node {
    app("operation-v1", vec![sym(name), int(arity), body])
}
fn make(name: &str, xs: Vec<Node>) -> Node {
    app("make", vec![literal(sym(name)), app("sequence", xs)])
}
impl Fixture {
    pub fn identity(flag: bool) -> Self {
        let declaration = app(
            "declaration-v1",
            vec![sym("BooleanAtom"), sym("Case/0/Atom/0"), sym("p")],
        );
        let declarations = app("declarations-v1", vec![declaration]);
        let dictionary = app(
            "dictionary-v1",
            vec![
                declarations.clone(),
                app("constants-v1", vec![]),
                app("helper-data-v1", vec![]),
                app("residuals-v1", vec![]),
            ],
        );
        // Native/raw generic field and declaration lowering is actually exercised.
        // R controls flag semantics; no matching Rust intrinsic supplies it.
        let runtime = app(
            "runtime-v1",
            vec![
                operation("copy", 1, arg(0)),
                operation("field", 2, app("child", vec![arg(0), arg(1)])),
                operation("flag", 0, literal(Node::Bool(flag))),
                operation(
                    "meaning",
                    1,
                    child(app("dictionary", vec![int(0), arg(0)]), 2),
                ),
            ],
        );
        Self {
            adapter: program(
                "adapter",
                make(
                    "sample-semantic-v1",
                    vec![
                        op("field", vec![arg(0), literal(int(0))]),
                        op("flag", vec![]),
                        op("meaning", vec![literal(int(0))]),
                    ],
                ),
            ),
            projection: program("projection", op("copy", vec![arg(0)])),
            inverse: program("inverse", op("copy", vec![arg(0)])),
            helpers: app("helpers-v1", vec![]),
            runtime,
            dictionary,
            fields: app("reported-fields-v1", vec![Node::Bool(false)]),
            declarations,
        }
    }
    pub fn semantic(flag: bool) -> Node {
        app(
            "sample-semantic-v1",
            vec![Node::Bool(false), Node::Bool(flag), sym("p")],
        )
    }
    pub fn packed() -> Self {
        let mut f = Self::identity(false);
        f.adapter = program(
            "adapter",
            make(
                "pair-v1",
                vec![
                    op("field", vec![arg(0), literal(int(0))]),
                    op("field", vec![arg(0), literal(int(0))]),
                ],
            ),
        );
        f.projection = program("projection", literal(app("packed-v1", vec![])));
        f.inverse = program(
            "inverse",
            make(
                "pair-v1",
                vec![
                    app("dictionary", vec![int(1), literal(int(0))]),
                    app("dictionary", vec![int(1), literal(int(0))]),
                ],
            ),
        );
        let Node::Apply(_, xs) = &mut f.dictionary else {
            unreachable!()
        };
        xs[1] = app("constants-v1", vec![Node::Bool(false)]);
        f
    }
    pub fn bytes(&self) -> Bytes {
        Bytes {
            adapter: encoded(&self.adapter),
            projection: encoded(&self.projection),
            inverse: encoded(&self.inverse),
            helpers: encoded(&self.helpers),
            runtime: encoded(&self.runtime),
            dictionary: encoded(&self.dictionary),
            fields: encoded(&self.fields),
            declarations: encoded(&self.declarations),
        }
    }
    pub fn bundle(&self, target: Node) -> Node {
        app(
            "bundle-v1",
            vec![
                target,
                app(
                    "decoder-v1",
                    vec![
                        self.adapter.clone(),
                        self.projection.clone(),
                        self.inverse.clone(),
                        self.helpers.clone(),
                        self.runtime.clone(),
                    ],
                ),
                self.dictionary.clone(),
            ],
        )
    }
}
impl Bytes {
    pub fn package(&self) -> InlinePackage<'_> {
        InlinePackage {
            adapter: &self.adapter,
            projection: &self.projection,
            inverse: &self.inverse,
            helpers: &self.helpers,
            runtime: &self.runtime,
            dictionary: &self.dictionary,
        }
    }
    pub fn input(&self) -> ReportedFieldInput<'_> {
        ReportedFieldInput {
            fields: &self.fields,
            declarations: &self.declarations,
        }
    }
}
