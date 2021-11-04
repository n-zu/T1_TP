use std::{
    sync::mpsc::{self, channel, Receiver, Sender},
    thread::{self, JoinHandle},
    time::Duration,
};

use threadpool_error::ThreadPoolError;
mod threadpool_error;
type Job = Box<dyn FnOnce() + Send + 'static>;

// Cuanto debe esperar a que se libere un thread (máximo)
const THREAD_WAIT_TIMEOUT: Duration = Duration::from_millis(200);

pub struct ThreadPool {
    thread_manager_handler: Option<JoinHandle<()>>,
    job_sender: Option<Sender<Job>>,
}

struct ThreadInfo {
    handler: Option<JoinHandle<()>>,
    alive_receiver: Receiver<bool>,
    job_sender: Sender<Job>,
}

struct ThreadManager {
    threads: Vec<ThreadInfo>,
    ready_receiver: Receiver<usize>,
    job_receiver: Receiver<Job>,
    ready_sender: Option<Sender<usize>>,
}

impl ThreadManager {
    fn new(amount: usize, job_receiver: Receiver<Job>) -> Self {
        let (ready_sender, ready_receiver) = channel();

        let ready_sender_clone = ready_sender.clone();
        let threads = intialize_threads(amount, ready_sender_clone);
        ThreadManager {
            threads,
            ready_receiver,
            job_receiver,
            ready_sender: Some(ready_sender),
        }
    }

    fn run(&mut self) {
        while let Ok(job) = self.job_receiver.recv() {
            let i = self.get_free_thread();
            // Nunca debería fallar ya que me mandó la señal de que está listo
            let _res = self.threads[i].job_sender.send(job);

            self.recover_threads();
        }
    }

    fn get_free_thread(&mut self) -> usize {
        let mut resultado = self.ready_receiver.recv_timeout(THREAD_WAIT_TIMEOUT);
        while resultado.is_err() {
            // Cabe la posibilidad que hayan paniqueado todos los threads, asi que intento arreglarlos
            self.recover_threads();
            resultado = self.ready_receiver.recv_timeout(THREAD_WAIT_TIMEOUT);
        }
        resultado.unwrap()
    }

    fn recover_threads(&mut self) {
        let ready_sender = self
            .ready_sender
            .clone()
            .expect("Error: ready_sender was dropped but the threadpool is still executing");

        for (id, thread) in self.threads.iter_mut().enumerate() {
            if let Err(mpsc::TryRecvError::Disconnected) = thread.alive_receiver.try_recv() {
                // Murio el thread
                Self::reset_thread(thread, id, ready_sender.clone());
            }
        }
    }

    fn reset_thread(thread: &mut ThreadInfo, id: usize, ready_sender: Sender<usize>) {
        if let Some(handle) = thread.handler.take() {
            let _res = handle.join();
        }

        let (alive_sender, alive_receiver) = channel();
        let (job_sender, job_receiver) = channel();

        thread.handler = Some(thread::spawn(move || {
            worker(job_receiver, alive_sender, ready_sender, id)
        }));

        thread.job_sender = job_sender;
        thread.alive_receiver = alive_receiver;
    }
}

impl ThreadPool {
    #![allow(dead_code)]
    pub fn new(amount: usize) -> ThreadPool {
        let (sender, receiver): (Sender<Job>, Receiver<Job>) = mpsc::channel();
        let handler = thread::spawn(move || {
            ThreadManager::new(amount, receiver).run();
        });

        ThreadPool {
            thread_manager_handler: Some(handler),
            job_sender: Some(sender),
        }
    }

    pub fn spawn<F>(&self, job: F) -> Result<(), ThreadPoolError>
    where
        F: FnOnce() + Send + 'static,
    {
        self.job_sender
            .as_ref()
            .ok_or_else(ThreadPoolError::new)?
            .send(Box::new(job))?;
        Ok(())
    }
}

fn intialize_threads(amount: usize, ready_sender: Sender<usize>) -> Vec<ThreadInfo> {
    let mut threads = Vec::new();
    for i in 0..amount {
        let (alive_sender, alive_receiver) = channel();
        let (job_sender, job_receiver) = channel();
        let rs = ready_sender.clone();

        let handler = thread::spawn(move || worker(job_receiver, alive_sender, rs, i));

        threads.push(ThreadInfo {
            handler: Some(handler),
            alive_receiver,
            job_sender,
        });
    }
    threads
}

impl Drop for ThreadManager {
    fn drop(&mut self) {
        if let Some(ready_sender) = self.ready_sender.take() {
            // Si falla en esta instancia mucho no se puede hacer
            drop(ready_sender);
            while let Some(mut thread) = self.threads.pop() {
                if let Some(handle) = thread.handler.take() {
                    drop(thread);
                    let _res = handle.join();
                }
            }
        }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        if let Some(job_sender) = self.job_sender.take() {
            // Si falla en esta instancia mucho no se puede hacer
            let _res = drop(job_sender);
            if let Some(handler) = self.thread_manager_handler.take() {
                let _res = handler.join();
            }
        }
    }
}

fn worker(
    job_receiver: Receiver<Job>,
    _alive: Sender<bool>,
    ready_sender: Sender<usize>,
    id: usize,
) {
    if let Err(_err) = ready_sender.send(id) {
        return;
    }

    for job in job_receiver {
        job();
        if let Err(_err) = ready_sender.send(id) {
            // Si falla por alguna razón, que muera el thread y de última después se recupera
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ThreadPool;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_sum_numbers() {
        let x = Arc::new(Mutex::new(0));
        let mut y = 0;

        let threadpool = ThreadPool::new(10);
        for i in 0..1000 {
            y += i;
            let x_copy = x.clone();
            let _res = threadpool.spawn(move || {
                *x_copy.lock().unwrap() += i;
            });
        }
        drop(threadpool);

        assert_eq!(*x.lock().unwrap(), y);
    }

    #[test]
    fn test_panic() {
        let x = Arc::new(Mutex::new(0));
        let mut y = 0;

        let threadpool = ThreadPool::new(10);

        for _ in 0..50 {
            let _res = threadpool.spawn(move || {
                panic!("Test panic");
            });
        }

        for i in 0..1000 {
            y += i;
            let x_copy = x.clone();
            let _res = threadpool.spawn(move || {
                *x_copy.lock().unwrap() += i;
            });
        }
        drop(threadpool);

        assert_eq!(*x.lock().unwrap(), y);
    }
}
