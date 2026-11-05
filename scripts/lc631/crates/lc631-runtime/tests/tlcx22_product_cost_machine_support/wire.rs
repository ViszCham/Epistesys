//! Independent finite K6 oracle. No production codec or machine is called.
//! These authored trees/bytes are Given test inputs, never native owner evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Node {
    Bool(bool),
    Int(i128),
    Symbol(String),
    Apply(String, Vec<Node>),
}
pub fn sym(s: &str) -> Node {
    Node::Symbol(s.into())
}
pub fn int(n: i128) -> Node {
    Node::Int(n)
}
pub fn app(s: &str, xs: Vec<Node>) -> Node {
    Node::Apply(s.into(), xs)
}
pub fn literal(x: Node) -> Node {
    app("literal", vec![x])
}
pub fn arg(n: i128) -> Node {
    app("arg", vec![int(n)])
}
pub fn child(x: Node, n: i128) -> Node {
    app("child", vec![x, literal(int(n))])
}
pub fn op(name: &str, xs: Vec<Node>) -> Node {
    let mut fields = vec![sym(name)];
    fields.extend(xs);
    app("op", fields)
}
fn leb(mut n: usize, out: &mut Vec<u8>) {
    loop {
        let low = (n & 127) as u8;
        n >>= 7;
        out.push(low | if n == 0 { 0 } else { 128 });
        if n == 0 {
            break;
        }
    }
}
fn write(node: &Node, out: &mut Vec<u8>) {
    match node {
        Node::Bool(b) => out.extend([1, u8::from(*b)]),
        Node::Int(n) => {
            out.extend([2, u8::from(*n < 0)]);
            let mut bytes = n.unsigned_abs().to_le_bytes().to_vec();
            while bytes.len() > 1 && bytes.last() == Some(&0) {
                bytes.pop();
            }
            leb(bytes.len(), out);
            out.extend(bytes);
        }
        Node::Symbol(s) => {
            out.push(3);
            leb(s.len(), out);
            out.extend(s.as_bytes());
        }
        Node::Apply(name, xs) => {
            out.push(4);
            write(&sym(name), out);
            leb(xs.len(), out);
            for x in xs {
                write(x, out);
            }
        }
    }
}
pub fn encoded(n: &Node) -> Vec<u8> {
    let mut out = Vec::new();
    write(n, &mut out);
    out
}
/// Independent algebraic size count. Does not call encoded or production code.
pub fn counted(n: &Node) -> usize {
    fn width(mut n: usize) -> usize {
        let mut w = 1;
        while n >= 128 {
            n >>= 7;
            w += 1;
        }
        w
    }
    match n {
        Node::Bool(_) => 2,
        Node::Int(n) => {
            let mut q = n.unsigned_abs();
            let mut bytes = 1;
            while q >= 256 {
                q >>= 8;
                bytes += 1;
            }
            2 + width(bytes) + bytes
        }
        Node::Symbol(s) => 1 + width(s.len()) + s.len(),
        Node::Apply(s, xs) => {
            1 + counted(&sym(s)) + width(xs.len()) + xs.iter().map(counted).sum::<usize>()
        }
    }
}
pub fn fields_mut(n: &mut Node) -> &mut Vec<Node> {
    match n {
        Node::Apply(_, xs) => xs,
        _ => panic!("authored Apply required"),
    }
}
