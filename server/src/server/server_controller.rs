use std::{sync::mpsc::Sender, thread::JoinHandle};

use tracing::debug;

use super::ServerResult;

pub struct ServerController {
    shutdown_sender: Sender<()>,
    handle: Option<JoinHandle<()>>,
}

impl ServerController {
    pub fn new(shutdown_sender: Sender<()>, handle: JoinHandle<()>) -> ServerController {
        ServerController {
            shutdown_sender,
            handle: Some(handle),
        }
    }

    fn shutdown(&mut self) -> ServerResult<()> {
        self.shutdown_sender.send(())?;
        match self.handle.take().unwrap().join() {
            Ok(()) => debug!("El thread del servidor fue joineado normalmente (sin panic)"),
            Err(err) => debug!("El thread del servidor fue joineado con panic: {:?}", err),
        }
        Ok(())
    }
}

impl Drop for ServerController {
    fn drop(&mut self) {
        self.shutdown().expect("Error al cerrar el servidor");
    }
}
