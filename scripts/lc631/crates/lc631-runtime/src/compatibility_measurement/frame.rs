//! Actual generated model syntax and existing checker, never status/error-as-false.
use super::*;
use lc631_solvers::{
    protocol::sexpr::{parse, SExprLimits},
    smt::{validate_model, CheckedModel},
};
use std::fmt::{self, Write};

pub(super) struct ModelFrame<'i> {
    pub bytes: Vec<u8>,
    pub model: Option<CheckedModel<'i>>,
    pub hard: Check,
    pub value: Option<bool>,
}
pub(super) fn guard<'i>(
    frame: &Frame,
    query: &'i SmtStatusQuery,
    cap: usize,
) -> Result<ModelFrame<'i>, Error> {
    let bytes = encode(frame, cap)?;
    let limits = SExprLimits::checked(cap, 64, 8).map_err(Error::Syntax)?;
    let document = parse(&bytes, limits).map_err(Error::Syntax)?;
    let (model, hard) = match validate_model(query, &document) {
        Ok(model) => {
            for assignment in frame {
                if model.boolean(assignment.symbol) != Some(assignment.value) {
                    return Err(Error::BindingMismatch);
                }
            }
            (Some(model), Check::Pass)
        }
        Err(ModelValidationError::AssertionFalse { index: 0 }) => (None, Check::Fail),
        Err(error) => return Err(Error::Model(error)),
    };
    // CheckedModel borrows the original query only, never the parser document.
    Ok(ModelFrame {
        bytes,
        model,
        hard,
        value: None,
    })
}
pub(super) fn use_stage<'i>(
    frame: &Frame,
    guard_query: &'i SmtStatusQuery,
    use_query: &SmtStatusQuery,
    symbol: BoolSymbolId,
    cap: usize,
) -> Result<ModelFrame<'i>, Error> {
    let mut result = guard(frame, guard_query, cap)?;
    let Some(model) = result.model.as_ref() else {
        // A confirmed guard failure is not a false use value or a successful model.
        return Ok(result);
    };
    let actual = model
        .boolean(symbol)
        .ok_or(Error::Model(ModelValidationError::MissingAssignment))?;
    let limits = SExprLimits::checked(cap, 64, 8).map_err(Error::Syntax)?;
    let document = parse(&result.bytes, limits).map_err(Error::Syntax)?;
    match validate_model(use_query, &document) {
        Ok(_) if actual => {}
        Err(ModelValidationError::AssertionFalse { index: 0 }) if !actual => {}
        Ok(_) | Err(ModelValidationError::AssertionFalse { index: 0 }) => {
            return Err(Error::BindingMismatch)
        }
        Err(error) => return Err(Error::Model(error)),
    }
    result.value = Some(actual);
    Ok(result)
}
fn encode(frame: &Frame, cap: usize) -> Result<Vec<u8>, Error> {
    let mut text = String::new();
    text.try_reserve_exact(cap)
        .map_err(|_| Error::AllocationFailed)?;
    let mut out = Bounded {
        text: &mut text,
        cap,
    };
    out.write_str("(model").map_err(|_| Error::ProgramBudget)?;
    for assignment in frame {
        // Only typed u32 names and literal booleans enter this generated frame.
        write!(
            out,
            " (define-fun b{} () Bool {})",
            assignment.symbol.get(),
            if assignment.value { "true" } else { "false" }
        )
        .map_err(|_| Error::ProgramBudget)?;
    }
    out.write_char(')').map_err(|_| Error::ProgramBudget)?;
    Ok(text.into_bytes())
}
struct Bounded<'a> {
    text: &'a mut String,
    cap: usize,
}
impl Write for Bounded<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let len = self.text.len().checked_add(s.len()).ok_or(fmt::Error)?;
        if len > self.cap {
            return Err(fmt::Error);
        }
        self.text.push_str(s);
        Ok(())
    }
}
