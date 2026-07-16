use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
};

use tracing_subscriber::fmt::MakeWriter;

#[derive(Clone, Default)]
pub(crate) struct CapturedLogs(Arc<Mutex<Vec<u8>>>);

impl CapturedLogs {
    pub(crate) fn contents(&self) -> String {
        String::from_utf8(self.0.lock().expect("captured log mutex poisoned").clone())
            .expect("tracing output is UTF-8")
    }
}

pub(crate) struct CapturedWriter(Arc<Mutex<Vec<u8>>>);

impl Write for CapturedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.0.lock().expect("captured log mutex poisoned").extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for CapturedLogs {
    type Writer = CapturedWriter;

    fn make_writer(&'a self) -> Self::Writer {
        CapturedWriter(Arc::clone(&self.0))
    }
}

pub(crate) fn capture() -> (impl tracing::Subscriber + Send + Sync, CapturedLogs) {
    let logs = CapturedLogs::default();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .without_time()
        .with_target(true)
        .with_max_level(tracing::Level::TRACE)
        .with_writer(logs.clone())
        .finish();
    (subscriber, logs)
}
