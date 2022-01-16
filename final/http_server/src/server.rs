use config::config::Config;
use std::{
    error::Error,
    io::prelude::*,
    net::{TcpListener, TcpStream},
    sync::{mpsc::Receiver, Arc, Mutex},
};

use crate::{html, http_response};

pub(crate) type ServerResult<T> = Result<T, Box<dyn Error>>;

pub struct Server {
    config: Config,
    connections: Arc<Mutex<Vec<TcpStream>>>,
}

impl Server {
    pub fn new(config: &Config) -> Self {
        Server {
            config: config.clone(),
            connections: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn run(self: Arc<Self>, receiver: Receiver<String>) -> ServerResult<()> {
        let server = self.clone();

        let _connection_listener = std::thread::spawn(move || {
            if let Err(e) = server.handle_connections() {
                println!("Error interno: {}", e);
            }
        });

        let _message_listener = std::thread::spawn(move || {
            if let Err(e) = self.handle_messages(receiver) {
                println!("Error interno: {}", e);
            }
        });

        Ok(())
    }

    fn handle_connections(self: Arc<Self>) -> ServerResult<()> {
        let listener = TcpListener::bind(&format!("{}:{}", self.config.server, self.config.port))?;

        println!(
            "_________\n\nListening on {}:{}\n_________",
            self.config.server, self.config.port
        );

        let mut connections = self
            .connections
            .lock()
            .map_err(|_| "Error inesperado: No se pudo obtener un lock")?;

        for stream in listener.incoming().collect::<Result<Vec<TcpStream>, _>>()? {
            connections.push(stream);
        }

        Ok(())
    }

    fn handle_messages(self: Arc<Self>, reciever: Receiver<String>) -> ServerResult<()> {
        loop {
            let message = reciever.recv()?;

            let mut connections = self
                .connections
                .lock()
                .map_err(|_| "Error inesperado: No se pudo obtener un lock")?;

            for stream in connections.iter() {
                let _stream = stream.try_clone()?;
                Self::post_message(_stream, &message)?;
                stream.shutdown(std::net::Shutdown::Both)?;
            }

            connections.clear();
        }
    }

    fn post_message(mut stream: TcpStream, message: &str) -> ServerResult<()> {
        let mut buffer = [0; 1024];

        stream.read_exact(&mut buffer)?;

        println!(
            "\x1b[0;33m----------------\n\n{}\n----------------\x1b[0m",
            String::from_utf8_lossy(&buffer)
        );

        let response = http_response!(html!(
            200, // refresh rate
            10,  // n-points
            message
        ));

        stream.write_all(response.as_bytes())?;
        stream.flush()?;
        Ok(())
    }
}
