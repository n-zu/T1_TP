use std::io::Read;

use crate::config::Config;
use server::{Server, ServerInterface};
use tracing::info;
use tracing_subscriber::{fmt, prelude::__tracing_subscriber_SubscriberExt, Registry};

mod client;
mod client_thread_joiner;
mod clients_manager;
mod config;
mod server;
mod topic_handler;

fn main() {
    let config = Config::new("config.txt").expect("Error cargando la configuracion");

    let file_appender = tracing_appender::rolling::hourly(config.log_path(), "logs.log");
    let (file_writer, _guard) = tracing_appender::non_blocking(file_appender);

    let subscriber = Registry::default()
        .with(fmt::Layer::default().with_writer(file_writer))
        .with(fmt::Layer::default().with_writer(std::io::stdout));

    tracing::subscriber::set_global_default(subscriber).unwrap();

    let threadpool_size = 20;
    let server = Server::new(config, threadpool_size);
    let _controller = server.run().unwrap();

    info!("Presione [ENTER] para detener la ejecucion del servidor");

    let mut buf = [0u8; 1];
    std::io::stdin()
        .read_exact(&mut buf)
        .expect("Error al leer de stdin");
}
