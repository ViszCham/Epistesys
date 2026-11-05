//! Independent small K6 fixture oracle, not the product implementation.
//! Complete helper source bytes are also Given byte-accounting input only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Node {
    Bool(bool),
    Int(i128),
    Sym(String),
    App(String, Vec<Node>),
}
pub fn sym(value: &str) -> Node { Node::Sym(value.to_owned()) }
pub fn app(name: &str, fields: Vec<Node>) -> Node { Node::App(name.to_owned(), fields) }
pub fn encoded(node: &Node) -> Vec<u8> {
    fn length(mut n: usize, out: &mut Vec<u8>) {
        loop {
            let byte = (n & 127) as u8;
            n >>= 7;
            out.push(byte | if n == 0 { 0 } else { 128 });
            if n == 0 { break; }
        }
    }
    fn write(node: &Node, out: &mut Vec<u8>) {
        match node {
            Node::Bool(v) => out.extend([1, u8::from(*v)]),
            Node::Int(v) => {
                out.extend([2, u8::from(*v < 0)]);
                let mut n = v.unsigned_abs();
                let mut magnitude = Vec::new();
                loop {
                    magnitude.push((n & 255) as u8);
                    n >>= 8;
                    if n == 0 { break; }
                }
                length(magnitude.len(), out);
                out.extend(magnitude);
            }
            Node::Sym(v) => { out.push(3); length(v.len(), out); out.extend(v.as_bytes()); }
            Node::App(name, fields) => {
                out.push(4);
                write(&Node::Sym(name.clone()), out);
                length(fields.len(), out);
                for field in fields { write(field, out); }
            }
        }
    }
    let mut bytes = Vec::new();
    write(node, &mut bytes);
    bytes
}

/// Independent decoder for this fixture's four K6 tags, not an arbitrary VM.
pub fn decoded(bytes: &[u8]) -> Node {
    fn take<'a>(bytes: &'a [u8], at: &mut usize, n: usize) -> &'a [u8] {
        let end = at.checked_add(n).unwrap();
        let result = &bytes[*at..end];
        *at = end;
        result
    }
    fn length(bytes: &[u8], at: &mut usize) -> usize {
        let mut n = 0usize;
        for shift in (0..usize::BITS).step_by(7) {
            let b = take(bytes, at, 1)[0];
            n |= usize::from(b & 127).checked_shl(shift).unwrap();
            if b & 128 == 0 { return n; }
        }
        panic!("invalid fixture length")
    }
    fn read(bytes: &[u8], at: &mut usize) -> Node {
        match take(bytes, at, 1)[0] {
            1 => match take(bytes, at, 1)[0] {
                0 => Node::Bool(false), 1 => Node::Bool(true), _ => panic!("fixture bool"),
            },
            2 => {
                let sign = take(bytes, at, 1)[0];
                let len = length(bytes, at);
                assert!((1..=16).contains(&len));
                let mut magnitude = [0u8; 16];
                magnitude[..len].copy_from_slice(take(bytes, at, len));
                let n = i128::try_from(u128::from_le_bytes(magnitude)).unwrap();
                match sign { 0 => Node::Int(n), 1 => Node::Int(-n), _ => panic!("fixture sign") }
            }
            3 => {
                let len = length(bytes, at);
                Node::Sym(std::str::from_utf8(take(bytes, at, len)).unwrap().to_owned())
            }
            4 => {
                let Node::Sym(name) = read(bytes, at) else { panic!("fixture operator") };
                let n = length(bytes, at);
                let fields = (0..n).map(|_| read(bytes, at)).collect();
                Node::App(name, fields)
            }
            _ => panic!("unsupported fixture tag"),
        }
    }
    let mut at = 0;
    let node = read(bytes, &mut at);
    assert_eq!(at, bytes.len());
    node
}

pub fn field_mut<'a>(node: &'a mut Node, name: &str, index: usize) -> &'a mut Node {
    let Node::App(actual, fields) = node else { panic!("fixture application") };
    assert_eq!(actual, name);
    &mut fields[index]
}
