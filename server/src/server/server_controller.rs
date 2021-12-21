use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
};

use tracing::{error, trace};

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
}

impl Drop for ServerController {
    fn drop(&mut self) {
        self.shutdown_bool.store(true, Ordering::Relaxed);
        let handle = self
            .handle
            .take()
            .expect("Server tried to shut down but it was already turned off");
        let id = handle.thread().id();
        if let Err(e) = handle.join() {
            error!("{:?}: Thread joineado con panic: {:?}", id, e);
        } else {
            trace!("{:?}: Server thread joineado con éxito", id);
        }
    }
}
