use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::mpsc::{self, Receiver, Sender},
    thread::JoinHandle,
};

use tracing::{debug, error};

use crate::server::ServerResult;

pub struct ClientThreadJoiner {
    handles: Option<HashMap<SocketAddr, JoinHandle<()>>>,
    pending_to_join: Receiver<SocketAddr>,
    pending_sender: Sender<SocketAddr>,
}

impl ClientThreadJoiner {
    pub fn new() -> ClientThreadJoiner {
        let (sender, receiver) = mpsc::channel();
        ClientThreadJoiner {
            handles: Some(HashMap::new()),
            pending_to_join: receiver,
            pending_sender: sender,
        }
    }

    fn join(key: &SocketAddr, handle: JoinHandle<()>) {
        match handle.join() {
            Ok(()) => debug!("Thread de Address {} termino de forma esperada", key),
            Err(err) => error!("Thread de Address {} termino en panic: {:?}", key, err),
        }
    }

    pub fn join_finished(&mut self) {
        while let Ok(addr) = self.pending_to_join.try_recv() {
            if let Some(handle) = self.handles.as_mut().unwrap().remove(&addr) {
                ClientThreadJoiner::join(&addr, handle)
            }
        }
    }

    pub fn add(&mut self, key: SocketAddr, handle: JoinHandle<()>) {
        if let Some(old_handle) = self.handles.as_mut().unwrap().insert(key, handle) {
            ClientThreadJoiner::join(&key, old_handle);
        }
    }

    pub fn finished(&mut self, key: SocketAddr) -> ServerResult<()> {
        self.pending_sender.send(key)?;
        Ok(())
    }
}

impl Drop for ClientThreadJoiner {
    fn drop(&mut self) {
        for (key, handle) in self.handles.take().unwrap() {
            ClientThreadJoiner::join(&key, handle);
        }
    }
}
