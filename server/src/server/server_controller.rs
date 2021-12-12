use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
};

use tracing::error;

use super::ServerResult;

/// It is responsible for shutting down the
/// server from a different thread than
/// the one running it
pub struct ServerController {
    /// When false, the server should continue
    /// to function. When true, the server should
    /// be stopped
    shutdown_bool: Arc<AtomicBool>,
    /// Handle of the main server thread (the one
    /// that executes the server loop)
    handle: Option<JoinHandle<()>>,
}

impl ServerController {
    /// Create a new [`ServerController`] for the server that
    /// runs on the thread associated with the *handle* received
    pub fn new(shutdown_bool: Arc<AtomicBool>, handle: JoinHandle<()>) -> ServerController {
        ServerController {
            shutdown_bool,
            handle: Some(handle),
        }
    }

    /// Turn of the server
    pub fn shutdown(&mut self) -> ServerResult<()> {
        self.shutdown_bool.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let sv_thread_id = handle.thread().id();
            handle.join().unwrap_or_else(|e| {
                error!("{:?}: Thread joineado con panic: {:?}", sv_thread_id, e);
            });
        }
        Ok(())
    }
}

impl Drop for ServerController {
    fn drop(&mut self) {
        self.shutdown().expect("Error al cerrar el servidor");
    }
}
