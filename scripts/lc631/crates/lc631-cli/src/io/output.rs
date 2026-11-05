//! Encode completely under a quota before exposing a single public byte.
use lc631_runtime::resource::{
    ResourceAmount, ResourceError, ResourceKind, ResourceRequest, ResourceReservationView,
};
use serde::Serialize;
use std::io::{self, Write};

pub fn write_pretty_json<T: Serialize, W: Write>(
    value: &T,
    view: &ResourceReservationView<'_>,
    sink: &mut W,
) -> Result<(), ResourceError> {
    write_pretty_json_observed(value, view, sink).map_err(|error| match error {
        JsonWriteFailure::BeforeWrite(error) => error,
        JsonWriteFailure::SinkWrite(kind) => ResourceError::Io(kind),
    })
}

/// Keep encoding/admission separate from a possibly partial external write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum JsonWriteFailure {
    BeforeWrite(ResourceError),
    SinkWrite(io::ErrorKind),
}

pub(crate) fn write_pretty_json_observed<T: Serialize, W: Write>(
    value: &T,
    view: &ResourceReservationView<'_>,
    sink: &mut W,
) -> Result<(), JsonWriteFailure> {
    let before = JsonWriteFailure::BeforeWrite;
    let cap = view.amount(ResourceKind::Output).map_err(before)?;
    let amount = ResourceAmount::checked(cap).map_err(before)?;
    let _claim = view
        .claim_resources(&[
            ResourceRequest::new(ResourceKind::Ram, amount),
            ResourceRequest::new(ResourceKind::Output, amount),
        ])
        .map_err(before)?;
    let cap = usize::try_from(cap).map_err(|_| before(ResourceError::ArithmeticOverflow))?;
    let bytes = encode_pretty_bounded(value, cap, Some(view)).map_err(before)?;
    view.check().map_err(before)?;
    // Quota/encoding failures above write nothing. A sink I/O failure can still
    // be a partial external delivery; it is not rollback or host-sink evidence.
    sink.write_all(&bytes)
        .map_err(|error| JsonWriteFailure::SinkWrite(error.kind()))
}

pub(crate) fn encode_pretty_bounded<T: Serialize>(
    value: &T,
    cap: usize,
    view: Option<&ResourceReservationView<'_>>,
) -> Result<Vec<u8>, ResourceError> {
    if cap == 0 {
        return Err(ResourceError::InvalidAmount);
    }
    let mut output = JsonBuffer {
        bytes: Vec::new(),
        cap,
        failure: None,
        view,
    };
    if serde_json::to_writer_pretty(&mut output, value).is_err() {
        return Err(output
            .failure
            .unwrap_or(ResourceError::Io(io::ErrorKind::InvalidData)));
    }
    if output.write_all(b"\n").is_err() {
        return Err(output
            .failure
            .unwrap_or(ResourceError::Io(io::ErrorKind::InvalidData)));
    }
    Ok(output.bytes)
}

struct JsonBuffer<'a> {
    bytes: Vec<u8>,
    cap: usize,
    failure: Option<ResourceError>,
    view: Option<&'a ResourceReservationView<'a>>,
}
impl Write for JsonBuffer<'_> {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        let result = self.append(input);
        match result {
            Ok(()) => Ok(input.len()),
            Err(error) => {
                self.failure = Some(error);
                Err(io::Error::other("bounded JSON encoding failed"))
            }
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl JsonBuffer<'_> {
    fn append(&mut self, input: &[u8]) -> Result<(), ResourceError> {
        if let Some(view) = self.view {
            view.check()?;
        }
        let required = self
            .bytes
            .len()
            .checked_add(input.len())
            .ok_or(ResourceError::ArithmeticOverflow)?;
        if required > self.cap {
            return Err(ResourceError::Exhausted(ResourceKind::Output));
        }
        if required > self.bytes.capacity() {
            let grown = self
                .bytes
                .capacity()
                .checked_mul(2)
                .ok_or(ResourceError::ArithmeticOverflow)?;
            let capacity = grown.max(64).max(required).min(self.cap);
            self.bytes
                .try_reserve_exact(capacity - self.bytes.len())
                .map_err(|_| ResourceError::AllocationFailed)?;
            if self.bytes.capacity() > self.cap {
                return Err(ResourceError::Exhausted(ResourceKind::Ram));
            }
        }
        self.bytes.extend_from_slice(input);
        Ok(())
    }
}

#[cfg(test)]
mod stage_tests {
    use super::*;
    use lc631_core::context::SourceRevision;
    use lc631_runtime::resource::{Cancellation, Deadline, ResourceBroker, ResourceLimits};
    use serde::ser::Error;
    use std::time::Duration;

    struct InvalidValue;
    impl Serialize for InvalidValue {
        fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            Err(S::Error::custom("deliberate encoding failure"))
        }
    }

    #[derive(Default)]
    struct PartialSink {
        writes: usize,
        visible: usize,
    }
    impl Write for PartialSink {
        fn write(&mut self, input: &[u8]) -> io::Result<usize> {
            self.writes += 1;
            if self.writes == 1 {
                self.visible += input.len().min(1);
                Ok(input.len().min(1))
            } else {
                Err(io::Error::from(io::ErrorKind::BrokenPipe))
            }
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn resources(cap: u64, check: impl FnOnce(&ResourceReservationView<'_>)) {
        let limits = ResourceLimits::uniform(16_384).unwrap();
        let owner = ResourceBroker::new(limits).unwrap();
        let mut run = owner
            .begin_run(
                1,
                SourceRevision::of("output-stage fixture").digest(),
                limits,
                Deadline::after(Duration::from_secs(30)).unwrap(),
                Cancellation::default(),
            )
            .unwrap();
        let mut reservation = run
            .reserve(&[
                ResourceRequest::new(ResourceKind::Ram, ResourceAmount::checked(4096).unwrap()),
                ResourceRequest::new(ResourceKind::Output, ResourceAmount::checked(cap).unwrap()),
            ])
            .unwrap();
        {
            let view = reservation.lease();
            check(&view);
        }
        reservation.release_confirmed().unwrap();
        run.close_confirmed().unwrap();
        assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 0);
        assert_eq!(owner.used(ResourceKind::Output).unwrap(), 0);
    }

    #[test]
    fn tlcx05_json_encoding_failure_never_becomes_stdout_write_or_calls_sink() {
        resources(4096, |view| {
            let mut sink = PartialSink::default();
            assert_eq!(
                write_pretty_json_observed(&InvalidValue, view, &mut sink),
                Err(JsonWriteFailure::BeforeWrite(ResourceError::Io(
                    io::ErrorKind::InvalidData
                )))
            );
            assert_eq!(sink.writes, 0);
        });
    }

    #[test]
    fn tlcx05_json_quota_failure_is_before_external_output() {
        resources(16, |view| {
            let mut sink = PartialSink::default();
            assert_eq!(
                write_pretty_json_observed(&"a".repeat(100), view, &mut sink),
                Err(JsonWriteFailure::BeforeWrite(ResourceError::Exhausted(
                    ResourceKind::Output
                )))
            );
            assert_eq!(sink.visible, 0);
        });
    }

    #[test]
    fn tlcx05_partial_sink_failure_remains_distinct_from_before_write() {
        resources(4096, |view| {
            let mut sink = PartialSink::default();
            assert_eq!(
                write_pretty_json_observed(&"actual bytes", view, &mut sink),
                Err(JsonWriteFailure::SinkWrite(io::ErrorKind::BrokenPipe))
            );
            assert_eq!((sink.writes, sink.visible), (2, 1));
            let mut legacy = PartialSink::default();
            assert_eq!(
                write_pretty_json(&"actual bytes", view, &mut legacy),
                Err(ResourceError::Io(io::ErrorKind::BrokenPipe))
            );
        });
    }
}
