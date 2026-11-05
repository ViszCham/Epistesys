//! Actual dispatch history owns immutable lowering/model backing and private store IDs.
use super::*;
pub(super) struct NativeHistory<'v> {
    pub lowered: Vec<lowering::Lowered>,
    pub models: Vec<Vec<u64>>,
    pub records: Vec<Record>,
    pub store: ModalObservationStore<'v>,
    pub context: ContextRevision,
    pub scope: ContextScope,
}
pub(super) struct Record {
    pub model: usize,
    pub formula: usize,
    pub id: ModalObservationId,
    pub input: [u8; 32],
    pub bits: u64,
}
impl<'v> NativeHistory<'v> {
    pub fn new(
        capacity: usize,
        context: ContextRevision,
        scope: ContextScope,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        Ok(Self {
            lowered: Vec::new(),
            models: Vec::new(),
            records: bounds::reserve(capacity, 4096)?,
            store: ModalObservationStore::with_resources(capacity, view).map_err(Error::Modal)?,
            context,
            scope,
        })
    }
    pub fn evaluate(
        &mut self,
        source: &str,
        model: usize,
        formula: usize,
        view: &ResourceReservationView<'_>,
    ) -> Result<u64, Error> {
        if self.records.len() >= self.records.capacity() {
            return Err(Error::NativeBudget);
        }
        let lower = self.lowered.get(formula).ok_or(Error::InvalidMap)?;
        let valuation = self.models.get(model).ok_or(Error::InvalidMap)?;
        let frame = Frame::new(Profile::IntuitionisticPreorder, &[3, 2])
            .map_err(|e| Error::Modal(ModalError::Evaluation(e)))?;
        let model_input =
            Model::new(frame, valuation).map_err(|e| Error::Modal(ModalError::Evaluation(e)))?;
        let id = self
            .store
            .evaluate(
                ModalRequest {
                    source,
                    context: self.context,
                    scope: self.scope,
                    model: model_input,
                    nodes: &lower.nodes,
                    root: lower.root,
                },
                view,
            )
            .map_err(Error::Modal)?;
        let report = self
            .store
            .lookup(id, self.context, self.scope)
            .map_err(Error::Modal)?;
        let bits = report.evaluation().true_worlds();
        self.records.push(Record {
            model,
            formula,
            id,
            input: report.input_digest(),
            bits,
        });
        Ok(bits)
    }
    pub fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        // Central-only minimal shared seam; historical lookup is insufficient.
        self.store.check_current(view).map_err(Error::Modal)?;
        for lower in &self.lowered {
            lower.check_map()?;
        }
        if self.records.len() != self.store.len() {
            return Err(Error::ExpectedInputMismatch);
        }
        for r in &self.records {
            if r.model >= self.models.len() || r.formula >= self.lowered.len() {
                return Err(Error::ExpectedInputMismatch);
            }
            let report = self
                .store
                .lookup(r.id, self.context, self.scope)
                .map_err(Error::Modal)?;
            if report.input_digest() != r.input
                || report.evaluation().true_worlds() != r.bits
                || report.finite_world_count() != 2
            {
                return Err(Error::ExpectedInputMismatch);
            }
        }
        self.store.check_current(view).map_err(Error::Modal)
    }
}
pub(super) fn states(mut ordinal: usize, atoms: usize, radix: u8) -> Result<Vec<u8>, Error> {
    let mut values = bounds::reserve(atoms, 3)?;
    values.resize(atoms, 0);
    for value in values.iter_mut().rev() {
        *value = u8::try_from(ordinal % usize::from(radix)).map_err(|_| Error::ModelBudget)?;
        ordinal /= usize::from(radix);
    }
    if ordinal != 0 {
        return Err(Error::ModelBudget);
    }
    Ok(values)
}
pub(super) fn chain_model(states: &[u8], map: &[AtomCorrespondence]) -> Result<Vec<u64>, Error> {
    let mut values = bounds::reserve(states.len(), 3)?;
    values.resize(states.len(), 0);
    for m in map {
        let s = *states
            .get(usize::from(m.meaning))
            .ok_or(Error::InvalidMap)?;
        let bits = match s {
            0 => 0,
            1 => 2,
            2 => 3,
            _ => return Err(Error::InvalidMap),
        };
        *values
            .get_mut(usize::from(m.chain_slot))
            .ok_or(Error::InvalidMap)? = bits;
    }
    Ok(values)
}
pub(super) fn point_assignment(states: &[u8], map: &[AtomCorrespondence]) -> Result<usize, Error> {
    let mut value = 0;
    for m in map {
        let shift = states
            .len()
            .checked_sub(usize::from(m.point_slot) + 1)
            .ok_or(Error::InvalidMap)?;
        let state = *states
            .get(usize::from(m.meaning))
            .ok_or(Error::InvalidMap)?;
        if state > 1 {
            return Err(Error::InvalidMap);
        }
        value |= usize::from(state) << shift;
    }
    Ok(value)
}
