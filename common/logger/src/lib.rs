use tracing::{Level, Subscriber};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    fmt::{self, writer::MakeWriterExt, MakeWriter},
    prelude::__tracing_subscriber_SubscriberExt,
    Registry,
};

const LOG_PREFIX: &str = "log.";

/// Logger structs. Holds the subscriber guards.
/// If they were dropped, nothing would be logged.
pub struct Logger {
    _file_guard: WorkerGuard,
    _stdout_guard: WorkerGuard,
}

impl Logger {
    pub fn new(log_path: &str, file_level: Level, stdout_level: Level) -> Self {
        let file_appender = tracing_appender::rolling::hourly(log_path, LOG_PREFIX);
        let (file, _file_guard) = tracing_appender::non_blocking(file_appender);
        let (stdout, _stdout_guard) = tracing_appender::non_blocking(std::io::stdout());

        tracing::subscriber::set_global_default(Self::get_subscriber(
            file.with_max_level(file_level),
            stdout.with_max_level(stdout_level),
        ))
        .expect("Error inicializando el logger");

        Self {
            _file_guard,
            _stdout_guard,
        }
    }

    /// Sets up the tracing log subscriber and returns it
    fn get_subscriber<W1, W2>(file: W1, stdout: W2) -> impl Subscriber
    where
        W1: for<'writer> MakeWriter<'writer> + 'static,
        W2: for<'writer> MakeWriter<'writer> + 'static,
    {
        Registry::default()
            .with(
                fmt::Layer::default()
                    .json()
                    .with_thread_names(true)
                    .with_writer(file),
            )
            .with(
                fmt::Layer::default()
                    .with_thread_names(true)
                    .pretty()
                    .with_writer(stdout),
            )
    }
}
