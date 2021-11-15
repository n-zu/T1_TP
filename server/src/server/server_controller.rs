use std::{sync::mpsc::Sender, thread::JoinHandle};

use tracing::debug;

use super::ServerResult;

pub struct ServerController {
    shutdown_sender: Sender<()>,
    handle: JoinHandle<()>,
}

impl ServerController {
    pub fn new(shutdown_sender: Sender<()>, handle: JoinHandle<()>) -> ServerController {
        ServerController {
            shutdown_sender,
            handle,
        }
    }

    pub fn shutdown(self) -> ServerResult<()> {
        self.shutdown_sender.send(())?;
        match self.handle.join() {
            Ok(()) => debug!("El thread del servidor fue joineado normalmente (sin panic)"),
            Err(err) => debug!("El thread del servidor fue joineado con panic: {:?}", err),
        }
        Ok(())
    }
}
