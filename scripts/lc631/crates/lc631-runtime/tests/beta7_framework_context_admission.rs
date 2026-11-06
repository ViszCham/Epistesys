//! Exercise P3's source-feasibility boundary using independently compilable P1/P2 APIs.
#[path = "tlcx22_framework_p2_support/mod.rs"]
mod support;
use support::*;
#[path = "beta7_framework_inconsistent_support/mod.rs"]
mod inconsistent_fixture;
#[path = "beta7_framework_fixture_support/mod.rs"]
mod native_fixture;

#[test]
fn contradictory_gamma_is_rejected_before_a_framework_owner_exists() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        inconsistent_fixture::assert_inconsistent_native_admission(h, view);
    });
}
