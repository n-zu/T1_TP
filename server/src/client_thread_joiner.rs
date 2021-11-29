use std::{
    collections::HashMap,
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle, ThreadId},
};

use tracing::{debug, error};

use crate::server::{ServerError, ServerResult};

struct ThreadInfo {
    id: ThreadId,
    handle: JoinHandle<()>,
}

pub struct ClientThreadJoiner {
    handles: Option<HashMap<ThreadId, JoinHandle<()>>>,
    finished_sender: Option<Sender<ThreadInfo>>,
    joiner_thread_handle: Option<JoinHandle<()>>,
}

impl Default for ClientThreadJoiner {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientThreadJoiner {
    pub fn new() -> ClientThreadJoiner {
        let (sender, receiver) = mpsc::channel();
        let joiner_thread_handle = thread::spawn(move || ClientThreadJoiner::join_loop(receiver));

        ClientThreadJoiner {
            handles: Some(HashMap::new()),
            finished_sender: Some(sender),
            joiner_thread_handle: Some(joiner_thread_handle),
        }
    }

    pub fn add_thread(&mut self, id: ThreadId, handle: JoinHandle<()>) -> ServerResult<()> {
        if let Some(_prev_handle) = self
            .handles
            .as_mut()
            .expect("Handles es None")
            .insert(id, handle)
        {
            Err(ServerError::new_msg(&format!(
                "Se agrego un handle con Id repetido ({:?})",
                id
            )))
        } else {
            Ok(())
        }
    }

    fn join_loop(receiver: Receiver<ThreadInfo>) {
        for thread_info in receiver {
            match thread_info.handle.join() {
                Ok(()) => debug!("{:?} termino de forma esperada", thread_info.id),
                Err(err) => error!("{:?} termino en panic: {:?}", thread_info.id, err),
            }
        }
    }

    pub fn finished(&mut self, id: ThreadId) -> ServerResult<()> {
        if let Some(handle) = self.handles.as_mut().expect("Handles es None").remove(&id) {
            self.finished_sender
                .as_ref()
                .expect("finished_sender es None")
                .send(ThreadInfo { id, handle })
                .expect("Error de sender");
            Ok(())
        } else {
            Err(ServerError::new_msg(
                "Se intento joinear un thread invalido",
            ))
        }
    }
}

impl Drop for ClientThreadJoiner {
    fn drop(&mut self) {
        for (id, handle) in self.handles.take().expect("handles es None") {
            self.finished_sender
                .as_ref()
                .expect("finished_sender es None")
                .send(ThreadInfo { id, handle })
                .expect("Error de sender");
        }
        drop(
            self.finished_sender
                .take()
                .expect("finished_sender es None"),
        );
        if let Err(err) = self
            .joiner_thread_handle
            .take()
            .expect("joiner_thread_handle es None")
            .join()
        {
            error!("Thread de ClientThreadJoiner termino en panic: {:?}", err);
        } else {
            debug!("Thread de ClientThreadJoiner fue joineado normalmente")
        }
    }
}
