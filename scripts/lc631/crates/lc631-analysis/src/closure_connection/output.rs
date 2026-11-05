//! A producer cannot turn skipped/repeated or ignored failed consumption into
//! a result. FnOnce prevents the callback itself from being replayed.
use super::ConnectionError;

pub(super) struct OutputConsumer<F, T> {
    callback: Option<F>,
    result: Option<T>,
    calls: usize,
}
impl<F, T> OutputConsumer<F, T>
where
    F: FnOnce(&[u8]) -> Result<T, ConnectionError>,
{
    pub(super) fn new(callback: F) -> Self {
        Self {
            callback: Some(callback),
            result: None,
            calls: 0,
        }
    }
    pub(super) fn invoke(&mut self, bytes: &[u8]) -> Result<(), ConnectionError> {
        self.calls = self.calls.saturating_add(1);
        let callback = self.callback.take().ok_or(ConnectionError::OutputInvalid)?;
        self.result = Some(callback(bytes)?);
        Ok(())
    }
    pub(super) fn finish(self) -> Result<T, ConnectionError> {
        if self.calls != 1 {
            return Err(ConnectionError::OutputInvalid);
        }
        self.result.ok_or(ConnectionError::OutputInvalid)
    }
}
