//! Lower actual parsed terms to direct or selected checked-path references.
use super::{pipeline::Data, Error, LowerRule, ParseRule, ParsedFrameProgram, Stage};
use lc631_substrate::resource::ResourceReservationView;
pub struct LoweredFrameProgram<'p, 'i, 's, 'v> {
    pub(super) data: Data<'p, 'i, 's, 'v>,
}
impl<'p, 'i, 's, 'v> LoweredFrameProgram<'p, 'i, 's, 'v> {
    pub fn lower_current(
        parsed: ParsedFrameProgram<'p, 'i, 's, 'v>,
        r: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        let mut data = parsed.data;
        data.next_claim(r)?;
        for i in 0..data.rows.len() {
            let Some(term) = data.rows[i].term else {
                continue;
            };
            if !data.query_charge(i, 1, Stage::Lower)? {
                continue;
            }
            match term.rule {
                ParseRule::DirectEdgeAtomV1 => {
                    data.rows[i].lower = Some(LowerRule::DirectEdgeReferenceV1)
                }
                ParseRule::CheckedMacroCallV1 => {
                    data.rows[i].lower = Some(LowerRule::CheckedPathReferenceV1);
                    for j in 0..data.binding.frame.macros.len() {
                        if !data.query_charge(i, 1, Stage::Lower)? {
                            break;
                        }
                        if data.binding.frame.macros[j].edge == term.edge {
                            data.rows[i].selected = Some(j);
                            break;
                        }
                    }
                }
            }
        }
        data.check(r)?;
        Ok(Self { data })
    }
}
