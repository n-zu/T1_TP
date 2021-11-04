use server::Server;
use tracing_subscriber::{fmt, prelude::__tracing_subscriber_SubscriberExt, Registry};
#[allow(dead_code, unused_imports)]
use threadpool;
use crate::config::Config;

mod client;
mod config;
mod packet_scheduler;
mod server;
mod server_packets;
mod topic_handler;

fn main() {
    let config = Config::new("config.txt").expect("Error cargando la configuracion");

    let file_appender = tracing_appender::rolling::hourly(config.log_path(), "logs.log");
    let (file_writer, _guard) = tracing_appender::non_blocking(file_appender);

    let subscriber = Registry::default()
        .with(fmt::Layer::default().with_writer(file_writer))
        .with(fmt::Layer::default().with_writer(std::io::stdout));

    tracing::subscriber::set_global_default(subscriber).unwrap();

    let server = Server::new(config);
    server.run().unwrap()
}
