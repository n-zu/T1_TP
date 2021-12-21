use std::{
    collections::HashMap,
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle, ThreadId},
};

use tracing::{error, trace};

/// Joins threads in a simple way.
/// It creates a thread that receives
/// the handles of the threads and
/// joins them when they finish
pub struct ThreadJoiner {
    finished_sender: Sender<Message>,
    joiner_thread_handle: Option<JoinHandle<()>>,
}

/// It is created by spawning a new thread.
///
/// When the thread ends, Drop is invoked, and
/// sends a message through the channel indicating
/// the ThreadJoiner the ID of the thread so that
/// it can join it
pub struct ThreadGuard {
    id: ThreadId,
    sender: Sender<Message>,
}

enum Message {
    Started(JoinHandle<()>),
    Finished(ThreadId),
    Stop,
}

impl Default for ThreadJoiner {
    fn default() -> Self {
        Self::new()
    }
}

impl ThreadJoiner {
    /// Creates a new ThreadJoiner. Spawns a new
    /// thread for its operation
    pub fn new() -> ThreadJoiner {
        let (sender, receiver) = mpsc::channel();
        let joiner_thread_handle = thread::spawn(move || {
            ThreadJoiner::join_loop(receiver);
        });
        trace!("Creado thread {:?}", joiner_thread_handle.thread().id());

        ThreadJoiner {
            finished_sender: sender,
            joiner_thread_handle: Some(joiner_thread_handle),
        }
    }

    /// Spawns a new thread in which it executes the
    /// received action
    pub fn spawn<F>(&mut self, action: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let sender_clone = self.finished_sender.clone();
        let handle = thread::spawn(move || {
            let guard = ThreadGuard::new(thread::current().id(), sender_clone);
            action();
            drop(guard);
        });
        trace!("Creando thread {:?}", handle.thread().id());
        self.finished_sender.send(Message::Started(handle)).unwrap();
    }

    /// Executes the loop that joins the threads
    fn join_loop(receiver: Receiver<Message>) {
        let mut handles = HashMap::new();
        for message in receiver {
            match message {
                Message::Started(handle) => {
                    handles.insert(handle.thread().id(), handle);
                }
                Message::Finished(id) => {
                    if let Some(handle) = handles.remove(&id) {
                        Self::join(id, handle);
                    }
                }
                Message::Stop => break,
            }
        }
        Self::join_remaining(handles);
    }

    /// Joins all the remaining threads in the hashmap which may still
    /// be executing. Intented to be used on shutdown
    fn join_remaining(handles: HashMap<ThreadId, JoinHandle<()>>) {
        for (id, handle) in handles {
            Self::join(id, handle);
        }
    }

    /// Joins a single thread and logs any errors
    fn join(id: ThreadId, handle: JoinHandle<()>) {
        trace!("Join thread {:?}", id);
        handle.join().unwrap_or_else(|e| {
            error!("{:?} - Thread joineado con panic: {:?}", id, e);
        });
    }
}

impl Drop for ThreadJoiner {
    fn drop(&mut self) {
        self.finished_sender
            .send(Message::Stop)
            .unwrap_or_else(|e| {
                error!("Error de Sender: {}", e);
            });

        let joiner_thread_id = self.joiner_thread_handle.as_ref().unwrap().thread().id();
        trace!("Join thread {:?}", joiner_thread_id);
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

impl ThreadGuard {
    fn new(id: ThreadId, sender: Sender<Message>) -> Self {
        ThreadGuard { id, sender }
    }
}

impl Drop for ThreadGuard {
    fn drop(&mut self) {
        trace!("Drop de ThreadGuard: {:?}", self.id);
        self.sender
            .send(Message::Finished(self.id))
            .unwrap_or_else(|e| {
                error!("Error invocando al drop de ThreadGuard: {}", e);
            });
    }
}
