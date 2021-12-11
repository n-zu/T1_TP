use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle
};

use tracing::error;

use super::ServerResult;

/// It is responsible for shutting down the
/// server from a different thread than
/// the one running it
pub struct ServerController {
    shutdown_bool: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl ServerController {
    /// Create a new [ServerController] for the server that
    /// runs on the thread associated with the *handle* received
    pub fn new(shutdown_bool: Arc<AtomicBool>, handle: JoinHandle<()>) -> ServerController {
        ServerController {
            shutdown_bool,
            handle: Some(handle),
        }
    }

    /// Turn of the server
    fn shutdown(&mut self) -> ServerResult<()> {
        self.shutdown_bool.store(true, Ordering::Relaxed);
        let sv_thread_id = self.handle.as_ref().unwrap().thread().id();
        self.handle.take().unwrap().join().unwrap_or_else(|e| {
            error!("{:?}: Thread joineado con panic: {:?}", sv_thread_id, e);
        });
        Ok(())
    }
}

impl Drop for ServerController {
    fn drop(&mut self) {
        self.shutdown().expect("Error al cerrar el servidor");
    }
}
