use config::config::Config;
use std::{
    io::prelude::*,
    net::{TcpListener, TcpStream},
    sync::{mpsc::Receiver, Arc, Mutex},
};

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

    pub fn run(self: Arc<Self>, reciever: Receiver<String>) {
        let server = self.clone();

        let _connection_listener = std::thread::spawn(move || {
            server.handle_connections();
        });

        let _message_listener = std::thread::spawn(move || {
            self.handle_messages(reciever);
        });
    }

    fn handle_connections(self: Arc<Self>) {
        let listener =
            TcpListener::bind(&format!("{}:{}", self.config.server, self.config.port)).unwrap();
        println!(
            "_________\n\nListening on {}:{}\n_________",
            self.config.server, self.config.port
        );
        for stream in listener.incoming() {
            let stream = stream.unwrap();
            self.connections.lock().unwrap().push(stream);
        }
    }

    fn handle_messages(self: Arc<Self>, reciever: Receiver<String>) {
        loop {
            let message = reciever.recv().unwrap();
            for stream in self.connections.lock().unwrap().iter() {
                let _stream = stream.try_clone().unwrap();
                Self::post_message(_stream, &message);
                stream.shutdown(std::net::Shutdown::Both).unwrap();
            }
            self.connections.lock().unwrap().clear();
        }
    }

    fn post_message(mut stream: TcpStream, message: &str) {
        let mut buffer = [0; 1024];

        stream.read(&mut buffer).unwrap();
        println!(
            "\x1b[0;33m----------------\n\n{}\n----------------\x1b[0m",
            String::from_utf8_lossy(&buffer)
        );

        let html = format!(
            "
            <!DOCTYPE html>
            <html lang=\"en\">
            <head>
                <title>monitor</title>
            </head>
            <script>
                setTimeout(function() {{
                    window.location.reload(1);
                }}, {});
            </script>
            <div style=\"display: flex;align-items: center;justify-content: center;min-height: 100vh;\">
                <h1>{}</h1>
            </div>
            </html>
            ",
            2000,
            message
        );

        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
            html.len(),
            html
        );

        stream.write(response.as_bytes()).unwrap();
        stream.flush().unwrap();
    }
}
