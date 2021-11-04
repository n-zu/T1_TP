use std::{sync::mpsc::{self, channel}, thread::{self, JoinHandle}};

type Job = Box<dyn FnOnce() + Send + 'static>;

struct ThreadPool {
    thread_manager_handler : Option<JoinHandle<()>>,
    job_sender : Option<mpsc::Sender<Job>>,
}

impl ThreadPool {
    #![allow(dead_code)]
    
    fn thread_manager(cantidad_threads : usize, job_receiver : mpsc::Receiver<Job>)
    {
        let mut handlers = Vec::new();
        let mut job_senders : Vec<mpsc::Sender<Job>> = Vec::new();
        let (sender_threads, receiver_threads ) = channel();
        for i in 0..cantidad_threads {
            let (s ,r ) = channel();
            job_senders.push(s);
            let sender_threads_copy = sender_threads.clone();
            handlers.push(thread::spawn(move || {
                    for job in r {
                        job();
                        let resultado = sender_threads_copy.send(i);
                        if let Err(error) = resultado {
                            println!("Error enviando aviso de disponibilidad, thread {} inutilizado: {}", i, error.to_string())
                        }
                    }
            }));
            if let Err(error) = sender_threads.send(i) {
                println!("Error enviando aviso de disponibilidad, thread {} inutilizado: {}", i, error.to_string())
            }
        }
    
        drop(sender_threads);

        for tarea in job_receiver {
            let i = receiver_threads.recv().expect("Error: threads hijos dropearon antes de tiempo");
            let sender = job_senders.get(i).expect("Error: Llegó un índice de thread inválido");
            if let Err(error) = sender.send(tarea) {
                println!("Error enviando tarea: {}", error.to_string())
            }
        }

        drop(job_senders);

        for h in handlers.into_iter() {
            if let Err(_error) = h.join() {
                println!("Error haciendo join a thread hijo");
            }
        }
    }

    pub fn new(cantidad_threads : usize) -> ThreadPool
    {
        let (sender, receiver) : (mpsc::Sender<Job>, mpsc::Receiver<Job>) = mpsc::channel();

        let handler = thread::spawn(move || {
            ThreadPool::thread_manager(cantidad_threads, receiver);
        });

        ThreadPool {
            thread_manager_handler : Some(handler),
            job_sender : Some(sender),
        }
    }

    pub fn spawn<F>(&self, funcion : F ) where
    F: FnOnce() -> (),
    F: Send + 'static,
    {
        if let Err(error) = self.job_sender.as_ref().unwrap().send(Box::new(funcion)) {
            println!("Error enviando tarea a thread hijo: {}", error.to_string());
        }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        drop(self.job_sender.take().unwrap());
        if let Err(_error) = self.thread_manager_handler.take().unwrap().join() {
            println!("Error haciendo join a thread hijo");
        }   
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use super::ThreadPool;

    #[test]
    fn test_sum_numbers() {
        let mut x = Arc::new(Mutex::new(0));
        let mut y = 0;
        let threadpool = ThreadPool::new(10);
        for i in 0..1000 {
            y += i;
            let x_copy = x.clone();
            threadpool.spawn(move || {
                *x_copy.lock().unwrap() += i;
            });
        }
        drop(threadpool);

        assert_eq!(*x.lock().unwrap(), y);
    }
}
