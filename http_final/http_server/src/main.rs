use logger::Logger;

use observer::Observer;

use server::{Server, ServerGuard, ServerResult};
use std::io::Read;
use tracing::{error, info, instrument, Level};

mod messages;
mod observer;
mod server;
mod setup;

fn main() {
    let _logger = Logger::new("logs", Level::INFO, Level::TRACE);

    match setup::initialize_server() {
        Err(e) => error!("Error inicializando el servidor: {}", e),
        Ok(_guards) => {
            info!("Presione [ENTER] para detener la ejecución del servidor");
            let mut buf = [0u8; 1];
            std::io::stdin().read_exact(&mut buf).unwrap_or(());
        }
    }
}
