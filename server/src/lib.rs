use std::io::Read;

use tracing::{info, Level};

use crate::config::FileConfig;
use crate::logger::Logger;
pub use crate::server::{Server, ServerController};
pub use crate::traits::Config;

mod client;
mod clients_manager;
mod config;
mod logger;
mod network_connection;
mod server;
mod test_helpers;
mod thread_joiner;
mod topic_handler;
pub mod traits;

/// Initializes the server
pub fn init(config_path: &str) {
    let config = FileConfig::new(config_path).expect("Error cargando la configuracion");

    let _logger = Logger::new(config.log_path(), Level::INFO, Level::DEBUG);

    let threadpool_size = 8;
    let server = Server::new(config, threadpool_size).expect("Error iniciando el servidor");
    let _controller = server
        .run()
        .expect("Error iniciando ejecución del servidor");

    info!("Presione [ENTER] para detener la ejecucion del servidor");

    let mut buf = [0u8; 1];
    std::io::stdin().read_exact(&mut buf).unwrap_or(());
}
