use std::{
    collections::HashMap,
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle, ThreadId},
};

use crate::{
    logging::{self, LogKind},
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
            logging::log::<&str>(LogKind::ThreadStart(thread::current().id()));
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

    fn join_loop(receiver: Receiver<JoinHandle<()>>) {
        for handle in receiver {
            let thread_id = handle.thread().id();
            match handle.join() {
                Ok(()) => logging::log::<&str>(LogKind::ThreadEndOk(thread_id)),
                Err(err) => {
                    logging::log::<&str>(LogKind::ThreadEndErr(thread_id, &format!("{:?}", err)))
                }
            }
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
        if let Err(err) = self
            .joiner_thread_handle
            .take()
            .expect("joiner_thread_handle es None")
            .join()
        {
            logging::log::<&str>(LogKind::ThreadEndErr(
                joiner_thread_id,
                &format!("{:?}", err),
            ));
        } else {
            logging::log::<&str>(LogKind::ThreadEndOk(joiner_thread_id));
        }
    }
}
