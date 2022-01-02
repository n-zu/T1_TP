use config::config::Config;
use std::{
    io::prelude::*,
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
};
use threadpool::ThreadPool;

const N_THREADS: usize = 5;

pub struct Server {
    config: Config,
    _connections: Mutex<Vec<TcpStream>>,
    _pool: Mutex<ThreadPool>,
}

impl Server {
    pub fn new(config: &Config) -> Self {
        Server {
            config: config.clone(),
            _connections: Mutex::new(Vec::new()),
            _pool: Mutex::new(ThreadPool::new(N_THREADS)), // config.threads
        }
    }

    pub fn run(self: Arc<Self>) {
        let _server = self.clone();

        let _connection_listener = std::thread::spawn(move || {
            let listener =
                TcpListener::bind(&format!("{}:{}", self.config.server, self.config.port)).unwrap();
            println!(
                "_________\n\nListening on {}:{}\n_________",
                self.config.server, self.config.port
            );
            for stream in listener.incoming() {
                let stream = stream.unwrap();
                Self::handle_connection(stream);
                //_server.connections.lock().unwrap().push(stream);
            }
        });
    }

    fn handle_connection(mut stream: TcpStream) {
        let mut buffer = [0; 1024];

        stream.read(&mut buffer).unwrap();

        let contents = "HELLO WORLD";

        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
            contents.len(),
            contents
        );

        stream.write(response.as_bytes()).unwrap();
        stream.flush().unwrap();
    }
}
