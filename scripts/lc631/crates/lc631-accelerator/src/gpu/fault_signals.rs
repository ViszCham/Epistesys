//! Sticky blocking signals shared by the existing GPU client.
//! Callback invocation, uncaptured error and test injection have separate origins.
//! A callback invocation is not evidence of involuntary physical GPU failure.
use std::sync::atomic::{AtomicU8, Ordering};

const DEVICE_LOST_CALLBACK: u8 = 1;
const UNCAPTURED_ERROR: u8 = 1 << 1;
#[cfg(test)]
const TEST_INJECTED_FAULT: u8 = 1 << 2;

pub(super) struct FaultSignals {
    events: AtomicU8,
}

impl FaultSignals {
    pub(super) fn new() -> Self {
        Self {
            events: AtomicU8::new(0),
        }
    }

    // Keep every existing gate's Acquire read and any-fault blocking behavior.
    pub(super) fn load(&self, ordering: Ordering) -> bool {
        self.events.load(ordering) != 0
    }

    pub(super) fn record_device_lost_callback(&self) {
        // One atomic update publishes the origin and closes admission together.
        // fetch_or preserves other origins even when handlers run concurrently.
        self.events
            .fetch_or(DEVICE_LOST_CALLBACK, Ordering::Release);
    }

    pub(super) fn record_uncaptured_error(&self) {
        self.events.fetch_or(UNCAPTURED_ERROR, Ordering::Release);
    }

    pub(super) fn callback_observed(&self) -> bool {
        self.events.load(Ordering::Acquire) & DEVICE_LOST_CALLBACK != 0
    }

    #[cfg(test)]
    fn inject_test_fault(&self) {
        // No origin argument: a test control cannot claim a native callback.
        self.events.fetch_or(TEST_INJECTED_FAULT, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tlcx33_fault_signals_clean_state_is_unblocked_and_unobserved() {
        let signals = FaultSignals::new();
        assert!(!signals.load(Ordering::Acquire));
        assert!(!signals.callback_observed());
    }

    #[test]
    fn tlcx33_fault_signals_synthetic_callback_state_blocks_and_projects_callback() {
        let signals = FaultSignals::new();
        // Synthetic private-state coverage, not an actual wgpu callback or GPU fault.
        signals.record_device_lost_callback();
        assert!(signals.load(Ordering::Acquire));
        assert!(signals.callback_observed());
    }

    #[test]
    fn tlcx33_fault_signals_uncaptured_error_blocks_without_callback_attribution() {
        let signals = FaultSignals::new();
        signals.record_uncaptured_error();
        assert!(signals.load(Ordering::Acquire));
        assert!(
            !signals.callback_observed(),
            "uncaptured error must not be reported as a device-loss callback"
        );
    }

    #[test]
    fn tlcx33_fault_signals_test_injection_blocks_without_callback_attribution() {
        let signals = FaultSignals::new();
        signals.inject_test_fault();
        assert!(signals.load(Ordering::Acquire));
        assert!(
            !signals.callback_observed(),
            "test injection must not be reported as a device-loss callback"
        );
    }

    #[test]
    fn tlcx33_fault_signals_synthetic_callback_stays_sticky_after_other_events() {
        let events: [fn(&FaultSignals); 3] = [
            FaultSignals::record_device_lost_callback,
            FaultSignals::record_uncaptured_error,
            FaultSignals::inject_test_fault,
        ];
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let signals = FaultSignals::new();
            let mut callback_seen = false;
            for event in order {
                events[event](&signals);
                callback_seen |= event == 0;
                assert!(signals.load(Ordering::Acquire));
                assert_eq!(signals.callback_observed(), callback_seen);
            }
            assert_eq!(
                signals.events.load(Ordering::Acquire),
                DEVICE_LOST_CALLBACK | UNCAPTURED_ERROR | TEST_INJECTED_FAULT
            );
        }
        // Simultaneous handlers must retain every origin, not overwrite one another.
        let signals = FaultSignals::new();
        std::thread::scope(|scope| {
            for event in events {
                let signals = &signals;
                scope.spawn(move || event(signals));
            }
        });
        assert_eq!(
            signals.events.load(Ordering::Acquire),
            DEVICE_LOST_CALLBACK | UNCAPTURED_ERROR | TEST_INJECTED_FAULT
        );
    }

    fn gpu_packet() -> crate::semantic_pareto::CheckedSemanticGridBatch {
        use crate::semantic_pareto::{CheckedSemanticGridBatch, GridAxis, GridDirection};
        CheckedSemanticGridBatch::checked_numeric(
            [33; 32],
            lc631_core::EpochId(33),
            vec![
                GridAxis {
                    denominator: 1,
                    minimum: 0,
                    maximum: 3,
                    direction: GridDirection::Minimize,
                };
                2
            ],
            vec![[1; 32], [2; 32]],
            vec![1, 3, 3, 1],
            vec![crate::NumericMask::Known; 2],
        )
        .unwrap()
    }

    #[test]
    #[ignore = "requires explicit actual GPU optin; sticky injected state needs an isolated process"]
    fn tlcx33_same_client_test_injection_invalidates_current_and_blocks_admission() {
        use crate::semantic_pareto::{
            cpu_grid_frontier, ScopedSemanticGpuBudget, SemanticDeviceClass, SemanticGpuError,
        };
        use crate::{AcceleratorBroker, CubeClWgpuBackend};
        assert_eq!(
            std::env::var("EPISTESYS_TEST_SEMANTIC_GPU").as_deref(),
            Ok("1"),
            "explicit optin required"
        );
        let packet = gpu_packet();
        let budget = ScopedSemanticGpuBudget {
            max_buffers: 4,
            max_requested_bytes: 4096,
            max_inflight_requested_bytes: 4096,
            max_buffer_bytes: 4096,
        };
        let mut broker = AcceleratorBroker::new(CubeClWgpuBackend::default());
        let observation = broker
            .execute_semantic_pareto(&packet, budget, &mut || true)
            .unwrap();
        assert!(matches!(
            observation.adapter().device_class(),
            SemanticDeviceClass::DiscreteGpu | SemanticDeviceClass::IntegratedGpu
        ));
        assert_eq!(
            broker
                .check_semantic_current(&observation)
                .unwrap()
                .frontier_words(),
            cpu_grid_frontier(&packet)
        );
        let state = super::super::gpu_state(); // Reuse the production OnceLock/client.
        assert!(super::super::semantic_pareto::device_current());
        assert!(!state.device_lost.callback_observed());
        assert_eq!(state.device_lost.events.load(Ordering::Acquire), 0);
        state.device_lost.inject_test_fault(); // Control injection, never native loss.
        assert_eq!(
            state.device_lost.events.load(Ordering::Acquire),
            TEST_INJECTED_FAULT
        );
        assert!(!state.device_lost.callback_observed());
        assert!(!super::super::semantic_pareto::device_current());
        assert!(matches!(
            broker.check_semantic_current(&observation),
            Err(SemanticGpuError::StaleObservation)
        ));
        assert!(matches!(
            broker.execute_semantic_pareto(&packet, budget, &mut || true),
            Err(SemanticGpuError::Quarantined)
        ));
        // A fresh logical broker still uses the same blocked production client.
        let mut fresh_broker = AcceleratorBroker::new(CubeClWgpuBackend::default());
        assert!(matches!(
            fresh_broker.execute_semantic_pareto(&packet, budget, &mut || true),
            Err(SemanticGpuError::Quarantined)
        ));
        eprintln!(
            "tlcx33 same-client control injection only: adapter={} backend={} native_callback_observed=false; old_current=stale; new_admission=blocked",
            observation.adapter().name(), observation.adapter().backend()
        );
    }
}
