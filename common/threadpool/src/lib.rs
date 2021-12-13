use std::{
    sync::{
        mpsc::{self, channel, Receiver, Sender},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub use threadpool_error::ThreadPoolError;
mod threadpool_error;
type Job = Box<dyn FnOnce() + Send + 'static>;
type WorkerId = usize;

// Cuanto debe esperar a que se libere un thread (máximo)
// antes de verificar si hubo alguno que haya hecho panic
// Si es muy grande, hay más chances de que no se entere que perdió un thread
// Si es muy chico, pierde más tiempo verificando si perdió alguno
const THREAD_WAIT_TIMEOUT: Duration = Duration::from_micros(1000); // 0.001s

/// ThreadPool implementation
/// Allows to execute jobs concurrently
/// with a fixed number of threads
#[derive(Clone)]
pub struct ThreadPool {
    job_sender: Sender<Job>, // Sender por el que se le envían las tareas al ThreadManager
    _thread_manager_handler: Arc<ManagerHandle>, // Handler del thread que ejecuta al ThreadManager
} // Es importante que el sender este definido primero para que se dropee antes, sino el manager va a quedar bloqueado

// Información que se guarda el ThreadManager de cada worker thread
struct ThreadInfo {
    handler: Option<JoinHandle<()>>, // El handler para hacer join al thread
    job_sender: Sender<Job>,         // El canal para envíar tareas al thread
    alive_receiver: Receiver<bool>, // Un receiver por el que no se envía info, se usa para verificar
                                    // si se dropeó el sender, en cuyo caso el thread paniqueó
}

// Estructura que se ejecuta en el thread de la Threadpool, que se encarga
// de hacer de intermediario entre la interfaz de la ThreadPool y los worker threads
struct ThreadManager {
    threads: Vec<ThreadInfo>,           // El vector de threads
    ready_receiver: Receiver<WorkerId>, // Por donde se recibe la id de los threads que están libres
    job_receiver: Receiver<Job>,        // Por donde se reciben las tareas
    ready_sender: Sender<WorkerId>, // Una copia del receiver que se usa para saber que threads están libres
                                    // (se guarda para dársela a los threads que se revivan al haber paniqueado)
}

// Guarda el handle del thread que ejecuta al ThreadManager, cosa de hacerle join cuando se dropee
struct ManagerHandle(Option<JoinHandle<()>>);

impl Drop for ManagerHandle {
    fn drop(&mut self) {
        // Si falla acá mucho no se puede hacer
        if let Some(handle) = self.0.take() {
            let _res = handle.join();
        }
    }
}

impl ThreadManager {
    // Crea el ThreadManager con amount threads, recibe tareas por el job_receiver hasta que se cierre el sender
    fn new(amount: usize, job_receiver: Receiver<Job>) -> Self {
        let (ready_sender, ready_receiver) = channel();

        let ready_sender_clone = ready_sender.clone();
        let threads = Self::initialize_threads(amount, ready_sender_clone);
        ThreadManager {
            threads,
            ready_receiver,
            job_receiver,
            ready_sender,
        }
    }

    // Comienza a esperar por una tarea. Cuando se cierra el job_sender que tiene
    // la threadpool sale del ciclo infinito
    fn run(&mut self) {
        while let Ok(job) = self.job_receiver.recv() {
            let i = self.get_free_thread();
            // Nunca debería fallar ya que me mandó la señal de que está listo
            let _res = self.threads[i].job_sender.send(job);
        }
    }

    // Obtiene el índice del un thread worker libre
    // Espera hasta que haya uno disponible, y en caso de que no haya ninguno,
    // intenta resucitar threads que puedan haber paniqueado
    fn get_free_thread(&mut self) -> WorkerId {
        let mut resultado = self.ready_receiver.recv_timeout(THREAD_WAIT_TIMEOUT);
        while resultado.is_err() {
            // Cabe la posibilidad que alguno haya paniqueado, asi que intento arreglarlo
            self.recover_threads();
            resultado = self.ready_receiver.recv_timeout(THREAD_WAIT_TIMEOUT);
        }
        resultado.unwrap()
    }

    // Recorre la lista de threads y revive a aquellos que estén muertos (lo hace con el ready_receiver)
    fn recover_threads(&mut self) {
        for (id, thread) in self.threads.iter_mut().enumerate() {
            if let Err(mpsc::TryRecvError::Disconnected) = thread.alive_receiver.try_recv() {
                // Murio el thread
                Self::reset_thread(thread, id, self.ready_sender.clone());
            }
        }
    }

    // Resucita un thread muerto. Le hace join y lo inicia de nuevo, actualizando los channels
    fn reset_thread(thread: &mut ThreadInfo, id: WorkerId, ready_sender: Sender<WorkerId>) {
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

    // Crea el vector de amount threads workers, inicializándolos con sus canales de comunicación
    fn initialize_threads(amount: usize, ready_sender: Sender<WorkerId>) -> Vec<ThreadInfo> {
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
}

impl ThreadPool {
    /// Creates a new threadpool with the given amount of threads.
    /// The threadpool uses an extra thread for internal processing.
    pub fn new(amount: usize) -> ThreadPool {
        let (sender, receiver): (Sender<Job>, Receiver<Job>) = mpsc::channel();
        let handler = thread::spawn(move || {
            ThreadManager::new(amount, receiver).run();
        });

        ThreadPool {
            job_sender: sender,
            _thread_manager_handler: Arc::new(ManagerHandle(Some(handler))),
        }
    }

    /// Submits a job to the thread pool.
    pub fn execute<F>(&self, job: F) -> Result<(), ThreadPoolError>
    where
        F: FnOnce() + Send + 'static,
    {
        self.job_sender.send(Box::new(job))?;
        Ok(())
    }
}

impl Drop for ThreadManager {
    fn drop(&mut self) {
        while let Some(mut thread) = self.threads.pop() {
            // Si falla en esta instancia mucho no se puede hacer
            if let Some(handle) = thread.handler.take() {
                // Dropeo el thread, que tiene el channel con el que se le envian las tareas,
                // con lo que la función sale del loop
                drop(thread);

                let _ = handle.join();
            }
        }
    }
}

// Función que ejecuta cada thread worker
fn worker(
    job_receiver: Receiver<Job>,
    _alive: Sender<bool>,
    ready_sender: Sender<WorkerId>,
    id: WorkerId,
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
    use std::{
        sync::{Arc, Mutex},
        thread,
    };

    #[test]
    fn test_sum_numbers() {
        let x = Arc::new(Mutex::new(0));
        let mut y = 0;

        let threadpool = ThreadPool::new(10);
        for i in 0..1000 {
            y += i;
            let x_copy = x.clone();
            let _res = threadpool.execute(move || {
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
            let _res = threadpool.execute(move || {
                panic!("Test panic");
            });
        }

        for i in 0..1000 {
            y += i;
            let x_copy = x.clone();
            let _res = threadpool.execute(move || {
                *x_copy.lock().unwrap() += i;
            });
        }
        drop(threadpool);

        assert_eq!(*x.lock().unwrap(), y);
    }

    #[test]
    fn test_cloning_threadpool() {
        let x = Arc::new(Mutex::new(0));
        let mut y = 0;

        let threadpool = ThreadPool::new(10);
        let threadpool_2 = threadpool.clone();
        for i in 0..1000 {
            y += i;
            let x_copy = x.clone();
            let mut curr_threadpool = &threadpool;
            if i % 2 == 0 {
                curr_threadpool = &threadpool_2;
            }
            let _res = curr_threadpool.execute(move || {
                *x_copy.lock().unwrap() += i;
            });
        }
        drop(threadpool);
        drop(threadpool_2);

        assert_eq!(*x.lock().unwrap(), y);
    }

    #[test]
    fn test_cloning_threadpool_multithread() {
        let x = Arc::new(Mutex::new(0));

        let threadpool = ThreadPool::new(10);
        let threadpool_clone = threadpool.clone();
        let x_clone = x.clone();
        let handle = thread::spawn(move || {
            sum(x_clone, threadpool_clone);
        });
        let y = sum(x.clone(), threadpool);
        let _res = handle.join();

        assert_eq!(*x.lock().unwrap(), y * 2);
    }

    fn sum(x: Arc<Mutex<i32>>, threadpool: ThreadPool) -> i32 {
        let mut y = 0;
        for i in 0..1000 {
            y += i;
            let x_copy = x.clone();
            let _res = threadpool.execute(move || {
                *x_copy.lock().unwrap() += i;
            });
        }
        y
    }
}
