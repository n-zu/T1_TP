use std::{sync::mpsc::Sender, thread::JoinHandle};

use crate::logging::{self, LogKind};

use super::ServerResult;

/// It is responsible for shutting down the
/// server from a different thread than
/// the one running it
pub struct ServerController {
    shutdown_sender: Sender<()>,
    handle: Option<JoinHandle<()>>,
}

impl ServerController {
    /// Create a new [ServerController] for the server that
    /// runs on the thread associated with the *handle* received
    pub fn new(shutdown_sender: Sender<()>, handle: JoinHandle<()>) -> ServerController {
        ServerController {
            shutdown_sender,
            handle: Some(handle),
        }
    }

    /// Turn of the server
    fn shutdown(&mut self) -> ServerResult<()> {
        self.shutdown_sender.send(())?;
        let sv_thread_id = self.handle.as_ref().unwrap().thread().id();
        match self.handle.take().unwrap().join() {
            Ok(()) => logging::log::<&str>(LogKind::ThreadEndOk(sv_thread_id)),
            Err(err) => {
                logging::log::<&str>(LogKind::ThreadEndErr(sv_thread_id, &format!("{:?}", err)))
            }
        }
        Ok(())
    }
}

impl Drop for ServerController {
    fn drop(&mut self) {
        self.shutdown().expect("Error al cerrar el servidor");
    }
}
