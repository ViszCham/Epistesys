//! Real frame-specific parsing; unperformed parsing stays Unknown.
use super::{
    pipeline::Data, syntax, CheckedFrameMap, Error, FrameProfile, QueryText, ReframePlan, Stage,
};
use lc631_core::CandidateId;
use lc631_substrate::resource::ResourceReservationView;
pub struct ParsedFrameProgram<'p, 'i, 's, 'v> {
    pub(super) data: Data<'p, 'i, 's, 'v>,
}
impl<'p, 'i, 's, 'v> ParsedFrameProgram<'p, 'i, 's, 'v> {
    pub fn parse_exact(
        plan: &'p ReframePlan<'i, 's, 'v>,
        binding: &'p CheckedFrameMap<'p, 'i, 's, 'v>,
        texts: &'i [QueryText<'i>],
        id: CandidateId,
        r: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        let mut data = Data::new(plan, binding, texts, id, r)?;
        for i in 0..data.rows.len() {
            let text = texts
                .iter()
                .find(|t| t.query == data.rows[i].id)
                .ok_or(Error::InvalidProbeSet)?
                .text;
            if text.len() > 16384 {
                return Err(Error::PayloadLimit);
            }
            if !data.query_charge(i, (text.len() + 1) as u64, Stage::Parse)? {
                continue;
            }
            let parsed = match data.profile() {
                FrameProfile::DirectEdgeV1 => {
                    syntax::parse_direct_v1(text, plan.input.base.node_count())?
                }
                FrameProfile::CheckedPathMacroV1 => {
                    syntax::parse_checked_path_v1(text, plan.input.base.node_count())?
                }
                FrameProfile::Unregistered(_) => {
                    return Err(Error::Unsupported(super::Cause::UnsupportedProfile))
                }
            };
            let expected = plan.input.perspectives[i].edge;
            if parsed.edge != expected {
                return Err(Error::FramePayloadMismatch);
            }
            data.rows[i].term = Some(parsed);
        }
        data.check(r)?;
        Ok(Self { data })
    }
}
