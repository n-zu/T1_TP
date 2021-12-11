use std::{
    collections::HashMap,
    sync::mpsc::{self, Receiver, Sender},
    thread::{JoinHandle, ThreadId, self},
};

use tracing::{error, instrument};

use crate::{
    server::{ServerError, ServerResult},
};

pub struct ThreadJoiner {
    handles: Option<HashMap<ThreadId, JoinHandle<()>>>,
    finished_sender: Option<Sender<JoinHandle<()>>>,
    joiner_thread_handle: Option<JoinHandle<()>>,
}

impl Default for ThreadJoiner {
    fn default() -> Self {
        Self::new()
    }
}

impl ThreadJoiner {
    pub fn new() -> ThreadJoiner {
        let (sender, receiver) = mpsc::channel();
        let joiner_thread_handle = thread::spawn(move || {
            ThreadJoiner::join_loop(receiver);
        });

        ThreadJoiner {
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

    #[instrument(skip(receiver) fields(thread_id) target = "thread_joining")]
    fn join_loop(receiver: Receiver<JoinHandle<()>>) {
        for handle in receiver {
            let thread_id = handle.thread().id();
            handle.join().unwrap_or_else(|e| {
                error!("{:?} - Thread joineado con panic: {:?}", thread_id, e);
            });
        }
    }

    pub fn finished(&mut self, id: ThreadId) -> ServerResult<()> {
        if let Some(handle) = self.handles.as_mut().expect("Handles es None").remove(&id) {
            self.finished_sender
                .as_ref()
                .expect("finished_sender es None")
                .send(handle)
                .expect("Error de sender");
            Ok(())
        } else {
            Err(ServerError::new_msg(
                "Se intento joinear un thread invalido",
            ))
        }
    }
}

impl Drop for ThreadJoiner {
    fn drop(&mut self) {
        for (_id, handle) in self.handles.take().expect("handles es None") {
            self.finished_sender
                .as_ref()
                .expect("finished_sender es None")
                .send(handle)
                .expect("Error de sender");
        }
        drop(
            self.finished_sender
                .take()
                .expect("finished_sender es None"),
        );
        let joiner_thread_id = self.joiner_thread_handle.as_ref().unwrap().thread().id();
        self.joiner_thread_handle
            .take()
            .expect("joiner_thread_handle es None")
            .join()
            .unwrap_or_else(|e| {
                error!(
                    "{:?} - Thread joineado con panic: {:?}",
                    joiner_thread_id, e
                );
            });
    }
}
