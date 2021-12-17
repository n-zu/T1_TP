use tracing::Subscriber;
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_subscriber::{fmt, prelude::__tracing_subscriber_SubscriberExt, Registry};

//trait Subscriber: tracing::Subscriber + Send + Sync + 'static {}

const LOG_PREFIX: &str = "log.";

/// Loger structs. Holds the subscriber guards.
/// If they were dropped, nothing would be logged.
pub struct Logger {
    _file_guard: WorkerGuard,
    _stdout_guard: WorkerGuard,
}

impl Logger {
    pub fn new(log_path: &str) -> Self {
        let file_appender = tracing_appender::rolling::hourly(log_path, LOG_PREFIX);
        let (file, _file_guard) = tracing_appender::non_blocking(file_appender);
        let (stdout, _stdout_guard) = tracing_appender::non_blocking(std::io::stdout());

        tracing::subscriber::set_global_default(Self::get_subscriber(file, stdout))
            .expect("Error inicializando el logger");

        Self {
            _file_guard,
            _stdout_guard,
        }
    }

    /// Sets up the tracing log subscriber and returns it
    fn get_subscriber(
        file: NonBlocking,
        stdout: NonBlocking,
    ) -> impl Subscriber + Send + Sync + 'static {
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
