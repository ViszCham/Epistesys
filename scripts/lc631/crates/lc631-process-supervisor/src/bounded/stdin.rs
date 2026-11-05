//! Owned stdin pump; fallible OS-thread startup never abandons child cleanup.
use std::fs::File;
use std::io::{self, Write};
#[cfg(windows)]
use std::thread;

pub(super) struct StdinPump {
    #[cfg(unix)]
    file: Option<File>,
    #[cfg(unix)]
    bytes: Vec<u8>,
    #[cfg(unix)]
    position: usize,
    #[cfg(windows)]
    worker: Option<thread::JoinHandle<io::Result<()>>>,
    #[cfg(windows)]
    stop: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    pub(super) error: bool,
}

impl StdinPump {
    pub(super) fn new(file: File, input: Option<Vec<u8>>) -> io::Result<Self> {
        #[cfg(unix)]
        {
            Ok(Self {
                file: input.as_ref().map(|_| file),
                bytes: input.unwrap_or_default(),
                position: 0,
                error: false,
            })
        }
        #[cfg(windows)]
        {
            use std::sync::atomic::{AtomicBool, Ordering};
            let stop = std::sync::Arc::new(AtomicBool::new(false));
            let worker = input
                .map(|bytes| {
                    let stopped = stop.clone();
                    thread::Builder::new()
                        .name("epistesys-stdin".into())
                        .spawn(move || {
                            let mut file = file;
                            let mut position = 0;
                            while position < bytes.len() && !stopped.load(Ordering::Acquire) {
                                let count = file.write(&bytes[position..])?;
                                if count == 0 {
                                    return Err(io::ErrorKind::WriteZero.into());
                                }
                                position += count;
                            }
                            Ok(())
                        })
                })
                .transpose()?;
            Ok(Self {
                worker,
                stop: Some(stop),
                error: false,
            })
        }
    }

    pub(super) fn closed_after_start_failure() -> Self {
        Self {
            #[cfg(unix)]
            file: None,
            #[cfg(unix)]
            bytes: Vec::new(),
            #[cfg(unix)]
            position: 0,
            #[cfg(windows)]
            worker: None,
            #[cfg(windows)]
            stop: None,
            error: true,
        }
    }

    pub(super) fn advance(&mut self) -> io::Result<()> {
        #[cfg(unix)]
        if let Some(file) = &mut self.file {
            if self.position == self.bytes.len() {
                self.file = None;
            } else {
                match file.write(&self.bytes[self.position..]) {
                    Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                    Ok(count) => self.position += count,
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                        ) => {}
                    Err(error) => {
                        self.error = true;
                        return Err(error);
                    }
                }
            }
        }
        #[cfg(windows)]
        if self
            .worker
            .as_ref()
            .is_some_and(thread::JoinHandle::is_finished)
        {
            self.error = !matches!(
                self.worker.take().map(thread::JoinHandle::join),
                Some(Ok(Ok(())))
            );
        }
        if self.error {
            Err(io::Error::other("stdin writer failed"))
        } else {
            Ok(())
        }
    }

    pub(super) fn finished(&mut self) -> bool {
        let _ = self.advance();
        #[cfg(unix)]
        {
            self.file.is_none()
        }
        #[cfg(windows)]
        {
            self.worker.is_none()
        }
    }

    pub(super) fn cancel(&mut self) {
        #[cfg(unix)]
        {
            self.file = None;
        }
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use std::sync::atomic::Ordering;
            use windows_sys::Win32::System::IO::CancelSynchronousIo;
            if let Some(stop) = &self.stop {
                stop.store(true, Ordering::Release);
            }
            if let Some(worker) = &self.worker {
                // SAFETY: JoinHandle retains an owned thread handle; cancellation
                // targets only this run's writer and never terminates the thread.
                let _ = unsafe { CancelSynchronousIo(worker.as_raw_handle().cast()) };
            }
        }
    }

    pub(super) fn cancel_if_finished_run(&mut self) {
        self.cancel();
    }
}
