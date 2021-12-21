use std::io::Read;

use tracing::info;

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

    let _logger = Logger::new(
        config.log_path(),
        config.log_file_level(),
        config.log_stdout_level(),
    );

    let threadpool_size = 8;
    let server = Server::new(config, threadpool_size).expect("Error iniciando el servidor");
    let controller = server
        .run()
        .expect("Error iniciando ejecución del servidor");

    info!("Presione [ENTER] para detener la ejecucion del servidor");

    let mut buf = [0u8; 1];
    std::io::stdin().read_exact(&mut buf).unwrap_or(());
    drop(controller);
}
