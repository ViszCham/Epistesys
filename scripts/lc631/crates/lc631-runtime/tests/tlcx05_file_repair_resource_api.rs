//! Compile-level cut: the repair driver must accept an injected owner rather
//! than forcing its caller back onto the legacy snapshot producer.
use lc631_analysis::{
    DgclFileEffectOwner, DgclFileEffectPlan, DgclFileRepairConfig, DgclFileRepairDriver,
    DgclRepairDriverError, ToolProcessRunner,
};

#[test]
fn tlcx05_file_repair_exposes_an_explicit_no_fallback_resource_constructor() {
    fn construct<'a>(
        config: DgclFileRepairConfig<'a>,
        runner: &'a mut dyn ToolProcessRunner,
        effects: Option<&'a mut dyn DgclFileEffectOwner>,
    ) -> Result<DgclFileRepairDriver<'a>, DgclRepairDriverError> {
        DgclFileRepairDriver::new_with_runner(config, runner, effects)
    }
    let _constructor = construct;
    fn borrow_only(_plan: DgclFileEffectPlan<'_>) {}
    let _plan_type = borrow_only;
}
