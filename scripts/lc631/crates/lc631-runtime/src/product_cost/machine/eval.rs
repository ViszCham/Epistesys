use super::super::{
    codec::{
        package::{program, table, Function},
        Package,
    },
    term, CostError, CostLimits, ExecutionStats, Term,
};
use super::{
    value::{self, Arena, Kind, Node, Value},
    Meter, Phase,
};
use std::rc::Rc;
#[cfg(test)]
#[path = "constructor_order_tests.rs"]
mod constructor_order_tests;
pub(in crate::product_cost) struct Root(Value);
pub(in crate::product_cost) struct Engine<'p> {
    package: &'p Package,
    helpers: Vec<Function<'p>>,
    runtime: Vec<Function<'p>>,
    arena: Rc<Arena>,
    limits: CostLimits,
    dictionary: Value,
}
impl<'p> Engine<'p> {
    pub fn new(
        package: &'p Package,
        limits: CostLimits,
        m: &mut Meter<'_>,
        stats: &mut ExecutionStats,
    ) -> Result<Self, CostError> {
        let arena = Arena::new(limits);
        m.chunks(term::work(&package.dictionary, limits)?)?;
        stats.copied_bytes = stats
            .copied_bytes
            .checked_add(term::length(&package.dictionary, limits)?)
            .ok_or(CostError::InvalidProgram)?;
        let dictionary = Value::new(
            value::input(&package.dictionary, &arena, limits, false, m.resources)?,
            &arena,
        )?;
        Ok(Self {
            package,
            helpers: table(&package.helpers, "helpers-v1", "function-v1")?,
            runtime: table(&package.runtime, "runtime-v1", "operation-v1")?,
            arena,
            limits,
            dictionary,
        })
    }
    pub fn input(
        &self,
        t: &Term,
        m: &mut Meter<'_>,
        stats: &mut ExecutionStats,
    ) -> Result<Root, CostError> {
        m.chunks(term::work(t, self.limits)?)?;
        stats.copied_bytes = stats
            .copied_bytes
            .checked_add(term::length(t, self.limits)?)
            .ok_or(CostError::InvalidProgram)?;
        Ok(Root(Value::new(
            value::input(t, &self.arena, self.limits, true, m.resources)?,
            &self.arena,
        )?))
    }
    pub fn execute(
        &self,
        phase: Phase,
        input: Root,
        m: &mut Meter<'_>,
        stats: &mut ExecutionStats,
    ) -> Result<Root, CostError> {
        let body = program(
            match phase {
                Phase::Adapter => &self.package.adapter,
                Phase::Projection => &self.package.projection,
                Phase::Inverse => &self.package.inverse,
            },
            match phase {
                Phase::Adapter => "adapter",
                Phase::Projection => "projection",
                Phase::Inverse => "inverse",
            },
        )?;
        m.debit(1)?;
        let args = [
            input.0,
            Value::new(Rc::clone(&self.dictionary.node), &self.arena)?,
        ];
        let mut vm = Vm {
            engine: self,
            meter: m,
            stats,
            phase,
        };
        Ok(Root(vm.eval(body, &args, 1)?))
    }
    pub fn encode(
        &self,
        root: &Root,
        m: &mut Meter<'_>,
        stats: &mut ExecutionStats,
    ) -> Result<Vec<u8>, CostError> {
        let n = &root.0.node;
        if n.tainted {
            return Err(CostError::InvalidProgram);
        }
        if n.bytes > self.limits.semantic_bytes {
            return Err(CostError::ResourceExhausted);
        }
        m.chunks(
            n.bytes
                .checked_add(n.nodes)
                .ok_or(CostError::ResourceExhausted)?,
        )?;
        let bytes = value::encoded(n, self.limits, m.resources)?;
        stats.copied_bytes = stats
            .copied_bytes
            .checked_add(bytes.len() as u64)
            .ok_or(CostError::InvalidProgram)?;
        m.resources.check()?;
        Ok(bytes)
    }
}
pub(in crate::product_cost) fn run(
    engine: &Engine<'_>,
    phase: Phase,
    input: Root,
    meter: &mut Meter<'_>,
    stats: &mut ExecutionStats,
) -> Result<Root, CostError> {
    engine.execute(phase, input, meter, stats)
}
/// One successful atomic constructor-fee debit; consuming it never refunds fuel.
struct PaidBuild;
struct Vm<'e, 'p, 'm, 'r> {
    engine: &'e Engine<'p>,
    meter: &'m mut Meter<'r>,
    stats: &'m mut ExecutionStats,
    phase: Phase,
}
impl Vm<'_, '_, '_, '_> {
    fn val(&self, n: Rc<Node>) -> Result<Value, CostError> {
        Value::new(n, &self.engine.arena)
    }
    fn build(&mut self, kind: Kind, children: u64) -> Result<Value, CostError> {
        let paid = self.charge_build(children)?;
        self.build_paid(paid, kind)
    }
    fn charge_build(&mut self, children: u64) -> Result<PaidBuild, CostError> {
        self.meter
            .debit(children.checked_add(1).ok_or(CostError::InvalidProgram)?)?;
        Ok(PaidBuild)
    }
    fn build_paid(&mut self, _paid: PaidBuild, kind: Kind) -> Result<Value, CostError> {
        self.val(value::make(kind, &self.engine.arena, self.engine.limits)?)
    }
    fn copied(&mut self, t: &Term) -> Result<Value, CostError> {
        let w = term::work(t, self.engine.limits)?;
        self.meter.chunks(w)?;
        self.stats.copied_bytes = self
            .stats
            .copied_bytes
            .checked_add(term::length(t, self.engine.limits)?)
            .ok_or(CostError::InvalidProgram)?;
        self.val(value::input(
            t,
            &self.engine.arena,
            self.engine.limits,
            false,
            self.meter.resources,
        )?)
    }
    fn eval(&mut self, e: &Term, args: &[Value], depth: u64) -> Result<Value, CostError> {
        self.meter.debit(1)?;
        self.phase.step(self.stats)?;
        if depth > self.engine.limits.depth {
            return Err(CostError::ResourceExhausted);
        }
        let (n, c) = match e {
            Term::Apply(n, c) => (n.as_str(), c.as_slice()),
            _ => return Err(CostError::InvalidProgram),
        };
        let out = match n {
            "arg" => self.val(Rc::clone(
                &args
                    .get(term::unsigned(&c[0])? as usize)
                    .ok_or(CostError::InvalidProgram)?
                    .node,
            ))?,
            "literal" => self.copied(&c[0])?,
            "if" => {
                let cond = self.eval(&c[0], args, depth + 1)?;
                let branch = if value::boolean(&cond.node)? { 1 } else { 2 };
                drop(cond);
                self.eval(&c[branch], args, depth + 1)?
            }
            "require" => {
                let cond = self.eval(&c[0], args, depth + 1)?;
                if !value::boolean(&cond.node)? {
                    return Err(CostError::InvalidProgram);
                }
                drop(cond);
                self.eval(&c[1], args, depth + 1)?
            }
            "child" => {
                let base = self.eval(&c[0], args, depth + 1)?;
                let index = self.eval(&c[1], args, depth + 1)?;
                let index = usize::try_from(value::unsigned(&index.node)?)
                    .map_err(|_| CostError::InvalidProgram)?;
                self.meter.debit(1)?;
                self.stats.native_field_reads = self
                    .stats
                    .native_field_reads
                    .checked_add(1)
                    .ok_or(CostError::InvalidProgram)?;
                match &base.node.kind {
                    Kind::Apply(_, xs) => {
                        self.val(Rc::clone(xs.get(index).ok_or(CostError::InvalidProgram)?))?
                    }
                    _ => return Err(CostError::InvalidProgram),
                }
            }
            "tag" | "operator" | "arity" => {
                let base = self.eval(&c[0], args, depth + 1)?;
                self.meter.debit(1)?;
                let t = match n {
                    "tag" => term::sym(value::tag(&base.node)),
                    "operator" => match &base.node.kind {
                        Kind::Apply(op, _) => term::sym(op),
                        _ => return Err(CostError::InvalidProgram),
                    },
                    _ => match &base.node.kind {
                        Kind::Apply(_, xs) => term::int(xs.len() as i128),
                        _ => return Err(CostError::InvalidProgram),
                    },
                };
                self.copied(&t)?
            }
            "sequence" => {
                // Arity is already known: fund constructor storage before even
                // the operand buffer. Partial operand failure keeps this debit.
                let paid = self
                    .charge_build(u64::try_from(c.len()).map_err(|_| CostError::InvalidProgram)?)?;
                let mut values = Vec::new();
                values
                    .try_reserve_exact(c.len())
                    .map_err(|_| CostError::ResourceExhausted)?;
                #[cfg(test)]
                constructor_order_tests::record(
                    constructor_order_tests::Constructor::Sequence,
                    constructor_order_tests::Storage::Operands,
                    values.capacity(),
                );
                for x in c {
                    values.push(self.eval(x, args, depth + 1)?)
                }
                let mut nodes = Vec::new();
                nodes
                    .try_reserve_exact(values.len())
                    .map_err(|_| CostError::ResourceExhausted)?;
                #[cfg(test)]
                constructor_order_tests::record(
                    constructor_order_tests::Constructor::Sequence,
                    constructor_order_tests::Storage::Nodes,
                    nodes.capacity(),
                );
                for x in &values {
                    nodes.push(Rc::clone(&x.node))
                }
                self.build_paid(paid, Kind::Apply("seq".into(), nodes))?
            }
            "make" => {
                let name = self.eval(&c[0], args, depth + 1)?;
                let seq = self.eval(&c[1], args, depth + 1)?;
                let name = value::symbol(&name.node)?;
                if name == "opaque-v1" || name == "external-v1" {
                    return Err(CostError::InvalidProgram);
                }
                let xs = value::seq(&seq.node)?;
                let paid = self.charge_build(
                    u64::try_from(xs.len()).map_err(|_| CostError::InvalidProgram)?,
                )?;
                self.meter.chunks(name.len() as u64)?;
                let name = value::copy_text(name, self.meter.resources)?;
                #[cfg(test)]
                constructor_order_tests::record(
                    constructor_order_tests::Constructor::Make,
                    constructor_order_tests::Storage::OperatorText,
                    name.capacity(),
                );
                let mut nodes = Vec::new();
                nodes
                    .try_reserve_exact(xs.len())
                    .map_err(|_| CostError::ResourceExhausted)?;
                #[cfg(test)]
                constructor_order_tests::record(
                    constructor_order_tests::Constructor::Make,
                    constructor_order_tests::Storage::Nodes,
                    nodes.capacity(),
                );
                for x in xs {
                    self.meter.resources.check()?;
                    nodes.push(Rc::clone(x))
                }
                self.build_paid(paid, Kind::Apply(name, nodes))?
            }
            "append" => {
                let a = self.eval(&c[0], args, depth + 1)?;
                let b = self.eval(&c[1], args, depth + 1)?;
                let a = value::seq(&a.node)?;
                let b = value::seq(&b.node)?;
                let count = a
                    .len()
                    .checked_add(b.len())
                    .ok_or(CostError::ResourceExhausted)?;
                if count as u64 > self.engine.limits.total_nodes {
                    return Err(CostError::ResourceExhausted);
                }
                let paid = self
                    .charge_build(u64::try_from(count).map_err(|_| CostError::InvalidProgram)?)?;
                let mut xs = Vec::new();
                xs.try_reserve_exact(count)
                    .map_err(|_| CostError::ResourceExhausted)?;
                #[cfg(test)]
                constructor_order_tests::record(
                    constructor_order_tests::Constructor::Append,
                    constructor_order_tests::Storage::Nodes,
                    xs.capacity(),
                );
                xs.extend(a.iter().cloned());
                xs.extend(b.iter().cloned());
                self.build_paid(paid, Kind::Apply("seq".into(), xs))?
            }
            "dictionary" => {
                let index = self.eval(&c[1], args, depth + 1)?;
                let index = usize::try_from(value::unsigned(&index.node)?)
                    .map_err(|_| CostError::InvalidProgram)?;
                let section = term::unsigned(&c[0])? as usize;
                self.meter.debit(2)?;
                self.stats.dictionary_reads = self
                    .stats
                    .dictionary_reads
                    .checked_add(1)
                    .ok_or(CostError::InvalidProgram)?;
                match &self.engine.dictionary.node.kind {
                    Kind::Apply(_, sections) => {
                        match &sections.get(section).ok_or(CostError::InvalidProgram)?.kind {
                            Kind::Apply(_, rows) => self.val(Rc::clone(
                                rows.get(index).ok_or(CostError::InvalidProgram)?,
                            ))?,
                            _ => return Err(CostError::InvalidProgram),
                        }
                    }
                    _ => return Err(CostError::InvalidProgram),
                }
            }
            "op" | "helper" => {
                let name = term::symbol(&c[0])?;
                let mut operands = Vec::new();
                operands
                    .try_reserve_exact(c.len() - 1)
                    .map_err(|_| CostError::ResourceExhausted)?;
                for x in &c[1..] {
                    operands.push(self.eval(x, args, depth + 1)?)
                }
                self.meter.debit(2)?;
                let table = if n == "op" {
                    &self.engine.runtime
                } else {
                    &self.engine.helpers
                };
                let f = table
                    .iter()
                    .find(|f| f.name == name)
                    .ok_or(CostError::MissingDependency)?;
                let body = f.body;
                if n == "op" {
                    self.stats.runtime_steps = self
                        .stats
                        .runtime_steps
                        .checked_add(1)
                        .ok_or(CostError::InvalidProgram)?;
                }
                self.eval(body, &operands, depth + 1)?
            }
            "fold" => {
                let max = term::unsigned(&c[0])?;
                let items = self.eval(&c[1], args, depth + 1)?;
                let xs = value::seq(&items.node)?;
                if xs.len() as u64 > max || max > self.engine.limits.iteration_bound {
                    return Err(CostError::InvalidProgram);
                }
                let mut acc = self.eval(&c[2], args, depth + 1)?;
                for (i, item) in xs.iter().enumerate() {
                    self.meter.debit(1)?;
                    let item = self.val(Rc::clone(item))?;
                    let idx = self.build(Kind::Atom(term::int(i as i128)), 0)?;
                    let frame = [acc, item, idx];
                    acc = self.eval(&c[3], &frame, depth + 1)?;
                }
                acc
            }
            "equal" | "less" | "canonical-less" | "symbol-less" | "add" | "subtract"
            | "multiply" => {
                let a = self.eval(&c[0], args, depth + 1)?;
                let b = self.eval(&c[1], args, depth + 1)?;
                let result = match n {
                    "equal" => Term::Bool(equal(&a.node, &b.node, self.meter)?),
                    "symbol-less" => {
                        let a = value::symbol(&a.node)?;
                        let b = value::symbol(&b.node)?;
                        Term::Bool(less_bytes(a.as_bytes(), b.as_bytes(), self.meter)?)
                    }
                    "canonical-less" => {
                        if a.node.tainted || b.node.tainted {
                            return Err(CostError::InvalidProgram);
                        }
                        self.meter
                            .chunks(a.node.bytes + a.node.nodes + b.node.bytes + b.node.nodes)?;
                        let aa = value::encoded(&a.node, self.engine.limits, self.meter.resources)?;
                        let bb = value::encoded(&b.node, self.engine.limits, self.meter.resources)?;
                        Term::Bool(less_bytes(&aa, &bb, self.meter)?)
                    }
                    _ => {
                        let a = value::unsigned(&a.node)?;
                        let b = value::unsigned(&b.node)?;
                        match n {
                            "less" => Term::Bool(a < b),
                            "add" => {
                                term::int(a.checked_add(b).ok_or(CostError::InvalidProgram)? as i128)
                            }
                            "subtract" => {
                                term::int(a.checked_sub(b).ok_or(CostError::InvalidProgram)? as i128)
                            }
                            _ => {
                                term::int(a.checked_mul(b).ok_or(CostError::InvalidProgram)? as i128)
                            }
                        }
                    }
                };
                self.build(Kind::Atom(result), 0)?
            }
            "fail" => return Err(CostError::InvalidProgram),
            _ => return Err(CostError::InvalidProgram),
        };
        self.meter.resources.check()?;
        Ok(out)
    }
}
fn equal(a: &Node, b: &Node, m: &mut Meter<'_>) -> Result<bool, CostError> {
    m.debit(2)?;
    Ok(match (&a.kind, &b.kind) {
        (Kind::Opaque(ka, a), Kind::Opaque(kb, b)) => {
            if ka != kb {
                return Err(CostError::InvalidProgram);
            }
            a == b
        }
        (Kind::Opaque(..), _) | (_, Kind::Opaque(..)) => return Err(CostError::InvalidProgram),
        (Kind::Atom(a), Kind::Atom(b)) => {
            m.chunks(a_bytes(a) + a_bytes(b))?;
            a == b
        }
        (Kind::Apply(na, a), Kind::Apply(nb, b)) => {
            m.chunks((na.len() + nb.len()) as u64)?;
            let mut same = na == nb && a.len() == b.len();
            for (a, b) in a.iter().zip(b) {
                same &= equal(a, b, m)?
            }
            same
        }
        (Kind::Binder(ka, sa, a), Kind::Binder(kb, sb, b)) => {
            m.chunks((sa.len() + sb.len()) as u64)?;
            ka == kb && sa == sb && equal(a, b, m)?
        }
        _ => false,
    })
}
fn a_bytes(t: &Term) -> u64 {
    match t {
        Term::Symbol(s) => s.len() as u64,
        Term::Integer(n) => n.bits().div_ceil(8),
        _ => 1,
    }
}
fn less_bytes(a: &[u8], b: &[u8], m: &mut Meter<'_>) -> Result<bool, CostError> {
    for (a, b) in a.iter().zip(b) {
        m.debit(2)?;
        if a != b {
            return Ok(a < b);
        }
    }
    Ok(a.len() < b.len())
}
