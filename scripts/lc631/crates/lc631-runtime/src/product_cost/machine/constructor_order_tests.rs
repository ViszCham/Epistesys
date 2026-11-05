//! Constructor storage/fuel regression with centrally registered actual hooks.
//! Observe actual constructor buffer capacity, never a caller success report.
use super::*;
use crate::resource::{
    Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
    ResourceRequest,
};
use std::{cell::Cell, time::Duration};

#[derive(Clone, Copy)]
pub(super) enum Constructor {
    Sequence = 0,
    Make = 1,
    Append = 2,
}
#[derive(Clone, Copy)]
pub(super) enum Storage {
    Operands = 0,
    OperatorText = 1,
    Nodes = 2,
}
#[derive(Clone, Copy, Default)]
struct Observation {
    armed: bool,
    events: [usize; 9],
    max_capacity: [usize; 9],
}
thread_local! {static OBSERVATION:Cell<Observation>=const {Cell::new(Observation{
armed:false,events:[0;9],max_capacity:[0;9]})};}

/// Called only immediately after the real Vec/String allocation operation.
/// The observer itself uses fixed TLS storage and allocates no event buffer.
pub(super) fn record(kind: Constructor, storage: Storage, capacity: usize) {
    OBSERVATION.with(|slot| {
        let mut o = slot.get();
        if !o.armed {
            return;
        }
        let i = kind as usize * 3 + storage as usize;
        if capacity > 0 {
            o.events[i] += 1;
            o.max_capacity[i] = o.max_capacity[i].max(capacity);
        }
        slot.set(o);
    });
}
struct Probe;
impl Probe {
    fn start() -> Self {
        OBSERVATION.with(|o| {
            o.set(Observation {
                armed: true,
                ..Observation::default()
            })
        });
        Self
    }
    fn finish(self) -> Observation {
        OBSERVATION.with(|slot| {
            let mut o = slot.get();
            o.armed = false;
            slot.set(o);
            o
        })
    }
}
impl Drop for Probe {
    fn drop(&mut self) {
        OBSERVATION.with(|o| {
            let mut s = o.get();
            s.armed = false;
            o.set(s)
        })
    }
}
fn expr_arg(i: i128) -> Term {
    term::app("arg", vec![term::int(i)])
}
fn fixture(kind: Constructor) -> (Term, [Term; 2]) {
    match kind {
        Constructor::Sequence => (
            term::app("sequence", vec![expr_arg(0), expr_arg(1)]),
            [Term::Bool(false), Term::Bool(true)],
        ),
        Constructor::Make => (
            term::app("make", vec![expr_arg(0), expr_arg(1)]),
            [
                term::sym("box"),
                term::app("seq", vec![Term::Bool(false), Term::Bool(true)]),
            ],
        ),
        Constructor::Append => (
            term::app("append", vec![expr_arg(0), expr_arg(1)]),
            [
                term::app("seq", vec![Term::Bool(false), Term::Bool(true)]),
                term::app("seq", vec![Term::Bool(true), Term::Bool(false)]),
            ],
        ),
    }
}
fn program(name: &str) -> Term {
    term::app(
        "program-v1",
        vec![term::sym(name), term::int(2), expr_arg(0)],
    )
}

/// Execute the real private Vm::eval path with real reservation/current checks.
/// Build fixtures, dictionary and argument Values before arming observations.
fn run_probe(kind: Constructor, fuel: u64) -> (Result<(), CostError>, u64, Observation) {
    let limits = CostLimits::default();
    let cap = ResourceLimits::uniform(320 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(cap).unwrap();
    let run = broker
        .begin_run(
            228,
            lc631_substrate::Digest32::from_bytes([28; 32]),
            cap,
            Deadline::after(Duration::from_secs(60)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let reservation = run
        .reserve(&[
            ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(64 * 1024 * 1024).unwrap(),
            ),
            ResourceRequest::new(
                ResourceKind::Output,
                ResourceAmount::checked(1024 * 1024).unwrap(),
            ),
        ])
        .unwrap();
    let view = reservation.lease();
    let _workspace = crate::product_cost::execution::reserve(&view, limits).unwrap();
    let package = Package {
        adapter: program("adapter"),
        projection: program("projection"),
        inverse: program("inverse"),
        helpers: term::app("helpers-v1", vec![]),
        runtime: term::app("runtime-v1", vec![]),
        dictionary: term::app(
            "dictionary-v1",
            vec![
                term::app("declarations-v1", vec![]),
                term::app("constants-v1", vec![]),
                term::app("helper-data-v1", vec![]),
                term::app("residuals-v1", vec![]),
            ],
        ),
    };
    let mut setup_meter = Meter::new(&view, limits.fuel_per_direction);
    let mut stats = ExecutionStats::default();
    let engine = Engine::new(&package, limits, &mut setup_meter, &mut stats).unwrap();
    let (expr, terms) = fixture(kind);
    let args = terms.map(|t| engine.input(&t, &mut setup_meter, &mut stats).unwrap().0);
    let mut meter = Meter::new(&view, fuel);
    let probe = Probe::start();
    let result = Vm {
        engine: &engine,
        meter: &mut meter,
        stats: &mut stats,
        phase: Phase::Adapter,
    }
    .eval(&expr, &args, 1)
    .map(drop);
    let used = meter.used;
    let observation = probe.finish();
    (result, used, observation)
}
fn check_rejection(kind: Constructor, starved_budgets: &[(u64, u64)], funded_used: u64) {
    let (control, control_used, observed) = run_probe(kind, 128);
    assert!(
        control.is_ok(),
        "funded control must use the actual constructor"
    );
    assert_eq!(
        control_used, funded_used,
        "normal constructor work must be charged exactly once"
    );
    let base = kind as usize * 3;
    assert!(
        observed.events[base..base + 3].iter().any(|n| *n > 0),
        "a missing/disconnected observation hook must fail, never fake zero-allocation success"
    );
    for (fuel, expected_used) in starved_budgets {
        let (result, used, observed) = run_probe(kind, *fuel);
        assert!(matches!(result, Err(CostError::FuelExhausted)));
        assert_eq!(
            used, *expected_used,
            "only successful atomic debits consume fuel; rejection does not burn the cap"
        );
        assert_eq!(
            &observed.events[base..base + 3],
            &[0, 0, 0],
            "constructor storage was allocated before its fuel rejection; observed capacities={:?}",
            observed.max_capacity
        );
    }
}
#[test]
fn tlcx22_cf_n28_sequence_fuel_rejection_precedes_constructor_buffers() {
    // Expr1 then fee3: budgets1/3 both reject at used1, before any buffer.
    // Funded work remains Expr1 + fee3 + two argument reads =6.
    check_rejection(Constructor::Sequence, &[(1, 1), (3, 1)], 6);
}
#[test]
fn tlcx22_cf_n28_make_fuel_rejection_precedes_constructor_storage() {
    // Strict LTR Expr1 + two argument reads =3, then fee3, then name bytes3.
    // Budget5 rejects at3; budget6 rejects at6 before copy. Funded total=9.
    check_rejection(Constructor::Make, &[(5, 3), (6, 6)], 9);
}
#[test]
fn tlcx22_cf_n28_append_fuel_rejection_precedes_constructor_buffer() {
    // Expr1 + two argument reads =3; fee5 rejects at3. Funded total=8.
    check_rejection(Constructor::Append, &[(3, 3)], 8);
}
