//! Actual endpoint Dijkstra plus separate path/Bellman/reachability validation.
//! Given graph costs stay Given; these checks establish finite-model arithmetic only.
use super::*;
use registry::{Edge, FrozenModel};
pub struct PathWitness {
    states: Vec<StateId>,
    canonical: Vec<u8>,
    cost: Rational,
}
impl PathWitness {
    pub fn states(&self) -> &[StateId] {
        &self.states
    }
    pub fn canonical_payload(&self) -> &[u8] {
        &self.canonical
    }
    pub fn cost(&self) -> &Rational {
        &self.cost
    }
}
pub(super) struct Endpoint {
    pub distances: Vec<Option<Rational>>,
    pub witness: Option<PathWitness>,
}
fn weight(edge: &Edge, upper: bool) -> &Rational {
    if upper {
        &edge.upper
    } else {
        &edge.lower
    }
}
fn same(a: &Rational, b: &Rational) -> Result<bool, Error> {
    Ok(numeric::checked_compare(a, b)?.is_eq())
}
pub(super) fn compute(
    model: &FrozenModel,
    upper: bool,
    work: &mut Work<'_, '_>,
) -> Result<Endpoint, Error> {
    let n = model.states.len();
    let mut distances = reserved(n)?;
    distances.resize_with(n, || None);
    distances[model.start] = Some(integer(0));
    let mut settled = [false; 16];
    for _ in 0..n {
        let mut best: Option<usize> = None;
        for node in 0..n {
            work.step(1)?;
            if settled[node] {
                continue;
            }
            if let Some(value) = &distances[node] {
                let replace = match best {
                    None => true,
                    Some(old) => match &distances[old] {
                        Some(prior) => numeric::checked_compare(value, prior)?.is_lt(),
                        None => return Err(Error::Witness),
                    },
                };
                // Nodes are content-sorted. IDs/order never break a distance tie.
                if replace {
                    best = Some(node);
                }
            }
        }
        let Some(node) = best else {
            break;
        };
        settled[node] = true;
        let current = distances[node].as_ref().ok_or(Error::Witness)?.clone();
        for edge in model.edges.iter().filter(|e| e.from == node) {
            work.step(1)?;
            let proposed = numeric::checked_add(&current, weight(edge, upper))?;
            let improve = match &distances[edge.to] {
                None => true,
                Some(prior) => numeric::checked_compare(&proposed, prior)?.is_lt(),
            };
            if improve {
                distances[edge.to] = Some(proposed);
            }
        }
    }
    let witness = if distances[model.goal].is_some() {
        Some(canonical_path(model, &distances, upper, work)?)
    } else {
        None
    };
    let endpoint = Endpoint { distances, witness };
    validate(model, &endpoint, upper, work)?;
    Ok(endpoint)
}
/// Lexicographically first SIMPLE sequence of canonical edge keys,
/// not lexical order of the length-prefixed whole-witness encoding. Reachability excluding
/// visited nodes makes zero-cost cycles terminate without ID-based predecessors.
fn canonical_path(
    model: &FrozenModel,
    distances: &[Option<Rational>],
    upper: bool,
    work: &mut Work<'_, '_>,
) -> Result<PathWitness, Error> {
    let n = model.states.len();
    let mut visited = [false; 16];
    let mut indices = reserved(n)?;
    let mut edge_indices = reserved(n)?;
    let mut at = model.start;
    visited[at] = true;
    indices.push(at);
    while at != model.goal {
        if indices.len() >= n {
            return Err(Error::Witness);
        }
        let mut candidates = reserved(model.edges.len())?;
        for (index, edge) in model.edges.iter().enumerate() {
            work.step(1)?;
            if edge.from == at && !visited[edge.to] && optimal(edge, distances, upper)? {
                candidates.push(index);
            }
        }
        candidates.sort_unstable_by(|a, b| model.edges[*a].key.cmp(&model.edges[*b].key));
        let mut selected = None;
        for index in candidates {
            let edge = &model.edges[index];
            if reachable(
                model,
                edge.to,
                model.goal,
                &visited,
                Some((distances, upper)),
                work,
            )? {
                selected = Some(index);
                break;
            }
        }
        let index = selected.ok_or(Error::Witness)?;
        at = model.edges[index].to;
        visited[at] = true;
        edge_indices.push(index);
        indices.push(at);
    }
    let mut states = reserved(indices.len())?;
    let mut canonical = Vec::new();
    let mut cost = integer(0);
    registry::field(&mut canonical, b"lc631.bmem.simple-optimal-path.v1")?;
    // Endpoint/model content belongs in the result even for a zero-edge self path.
    registry::field(&mut canonical, &model.states[model.start].payload)?;
    registry::field(&mut canonical, &model.states[model.goal].payload)?;
    for index in indices {
        states.push(model.states[index].id);
    }
    for index in edge_indices {
        let edge = &model.edges[index];
        registry::field(&mut canonical, &edge.key)?;
        cost = numeric::checked_add(&cost, weight(edge, upper))?;
    }
    Ok(PathWitness {
        states,
        canonical,
        cost,
    })
}
fn optimal(edge: &Edge, distances: &[Option<Rational>], upper: bool) -> Result<bool, Error> {
    match (&distances[edge.from], &distances[edge.to]) {
        (Some(a), Some(b)) => same(&numeric::checked_add(a, weight(edge, upper))?, b),
        (None, _) | (_, None) => Ok(false),
    }
}
/// Separate bounded BFS, not a second Dijkstra. Optional reduced optimal graph.
fn reachable(
    model: &FrozenModel,
    from: usize,
    to: usize,
    blocked: &[bool; 16],
    optimal_graph: Option<(&[Option<Rational>], bool)>,
    work: &mut Work<'_, '_>,
) -> Result<bool, Error> {
    if blocked[from] {
        return Ok(false);
    }
    let mut seen = *blocked;
    let mut queue = [0usize; 16];
    let mut head = 0;
    let mut tail = 1;
    queue[0] = from;
    seen[from] = true;
    while head < tail {
        work.step(1)?;
        let node = queue[head];
        head += 1;
        if node == to {
            return Ok(true);
        }
        for edge in model.edges.iter().filter(|e| e.from == node) {
            work.step(1)?;
            if seen[edge.to] {
                continue;
            }
            let admitted = match optimal_graph {
                None => true,
                Some((d, upper)) => optimal(edge, d, upper)?,
            };
            if admitted {
                if tail >= model.states.len() {
                    return Err(Error::Witness);
                }
                seen[edge.to] = true;
                queue[tail] = edge.to;
                tail += 1;
            }
        }
    }
    Ok(false)
}
/// Source0 + all-edge Bellman inequalities + an attaining simple path validate
/// optimality independently of the producer's relaxation order. BFS checks gaps.
fn validate(
    model: &FrozenModel,
    endpoint: &Endpoint,
    upper: bool,
    work: &mut Work<'_, '_>,
) -> Result<(), Error> {
    if endpoint.distances.len() != model.states.len() {
        return Err(Error::Witness);
    }
    if !same(
        endpoint.distances[model.start]
            .as_ref()
            .ok_or(Error::Witness)?,
        &integer(0),
    )? {
        return Err(Error::Witness);
    }
    for (index, distance) in endpoint.distances.iter().enumerate() {
        let reachable = reachable(model, model.start, index, &[false; 16], None, work)?;
        if reachable != distance.is_some() {
            return Err(Error::Witness);
        }
        if let Some(value) = distance {
            numeric::check(value)?;
            if numeric::checked_compare(value, &integer(0))?.is_lt() {
                return Err(Error::Witness);
            }
        }
    }
    for edge in &model.edges {
        work.step(1)?;
        if let Some(from) = &endpoint.distances[edge.from] {
            let to = endpoint.distances[edge.to].as_ref().ok_or(Error::Witness)?;
            if numeric::checked_compare(to, &numeric::checked_add(from, weight(edge, upper))?)?
                .is_gt()
            {
                return Err(Error::Witness);
            }
        }
    }
    match (&endpoint.witness, &endpoint.distances[model.goal]) {
        (None, None) => Ok(()),
        (Some(path), Some(goal)) => {
            if path.states.is_empty()
                || path.states.len() > model.states.len()
                || path.states.first() != Some(&model.states[model.start].id)
                || path.states.last() != Some(&model.states[model.goal].id)
            {
                return Err(Error::Witness);
            }
            let mut total = integer(0);
            let mut seen = [false; 16];
            let mut canonical = Vec::new();
            registry::field(&mut canonical, b"lc631.bmem.simple-optimal-path.v1")?;
            registry::field(&mut canonical, &model.states[model.start].payload)?;
            registry::field(&mut canonical, &model.states[model.goal].payload)?;
            for (i, id) in path.states.iter().enumerate() {
                work.step(1)?;
                let index = model
                    .states
                    .iter()
                    .position(|s| s.id == *id)
                    .ok_or(Error::Witness)?;
                if seen[index] {
                    return Err(Error::Witness);
                }
                seen[index] = true;
                if let Some(next) = path.states.get(i + 1) {
                    let edge = model
                        .edges
                        .iter()
                        .find(|e| e.from == index && model.states[e.to].id == *next)
                        .ok_or(Error::Witness)?;
                    total = numeric::checked_add(&total, weight(edge, upper))?;
                    registry::field(&mut canonical, &edge.key)?;
                }
            }
            if !same(&total, goal)? || !same(&path.cost, goal)? || canonical != path.canonical {
                return Err(Error::Witness);
            }
            Ok(())
        }
        (None, Some(_)) | (Some(_), None) => Err(Error::Witness),
    }
}
