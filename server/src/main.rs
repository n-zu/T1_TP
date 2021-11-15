use std::thread;

use crate::config::Config;
use server::Server;
use tracing_subscriber::{fmt, prelude::__tracing_subscriber_SubscriberExt, Registry};

mod client;
mod clients_manager;
mod config;
mod server;
mod server_packets;
mod topic_handler;
mod client_thread_joiner;

fn main() {
    let config = Config::new("config.txt").expect("Error cargando la configuracion");

    let file_appender = tracing_appender::rolling::hourly(config.log_path(), "logs.log");
    let (file_writer, _guard) = tracing_appender::non_blocking(file_appender);

    let subscriber = Registry::default()
        .with(fmt::Layer::default().with_writer(file_writer))
        .with(fmt::Layer::default().with_writer(std::io::stdout));

    tracing::subscriber::set_global_default(subscriber).unwrap();

    let threadpool_size = 5;
    let server = Server::new(config, threadpool_size);
    let _controller = server.run().unwrap();

    thread::park();
    //thread::sleep(Duration::from_secs(1));
    //controller.shutdown();
}
