use config::config::Config;
use std::{
    error::Error,
    fs,
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{mpsc::Receiver, Arc, Mutex, RwLock},
    time::Duration,
};
use threadpool::ThreadPool;
use tracing::{debug, error, info, instrument};

use crate::messages::{HttpRequest, HttpResponse, Request};

pub(crate) type ServerResult<T> = Result<T, Box<dyn Error>>;
const LOCK_ERR: &str = "Error desbloqueando lock";

// Crea un header del tipo especificado
macro_rules! hdr {
    ($x:expr) => {
        Some(format!(
            "Content-Type: {}\r\nCache-Control: max-age=3600\r\n",
            $x
        ))
    };
}

pub struct Server {
    config: Config,
    data: RwLock<String>,
    pool: Mutex<ThreadPool>,
}

impl Server {
    pub fn new(config: &Config) -> Self {
        Server {
            config: config.clone(),
            data: RwLock::new(String::from("")),
            pool: Mutex::new(ThreadPool::new(8)),
        }
    }

    #[instrument(skip(self, receiver) fields(ip = %self.config.server, port = %self.config.port))]
    pub fn run(self: Arc<Self>, receiver: Receiver<String>) -> ServerResult<()> {
        info!("Iniciando servidor");

        let server = self.clone();

        let _ = std::thread::spawn(move || {
            if let Err(e) = server.update_data(receiver) {
                error!("Error interno: {}", e);
            }
        });

        let _connection_listener = std::thread::spawn(move || {
            if let Err(e) = self.handle_connections() {
                error!("Error interno: {}", e);
            }
        });

        Ok(())
    }

    fn update_data(&self, receiver: Receiver<String>) -> ServerResult<()> {
        loop {
            let msg = receiver.recv()?;
            info!("Actualizando data: {}", msg);
            *self.data.write().map_err(|_| LOCK_ERR)? = msg;
        }
        //Ok(())
    }

    #[instrument(skip(self) fields(ip = %self.config.server, port = %self.config.port))]
    fn handle_connections(self: Arc<Self>) -> ServerResult<()> {
        let listener = TcpListener::bind(&format!("{}:{}", self.config.server, self.config.port))?;
        info!("Escuchando conexiones");

        for stream in listener.incoming() {
            let stream = stream?;
            let addr = stream.peer_addr()?;
            debug!("Nueva conexion: {}", stream.peer_addr()?);
            stream.set_read_timeout(Some(Duration::from_secs(15)))?;
            let server = self.clone();
            self.pool.lock().map_err(|_| LOCK_ERR)?.execute(move || {
                server.handle_request(addr, stream).unwrap_or_else(|e| {
                    error!("Error manejando el request: {}", e);
                });
            })?;
        }

        Ok(())
    }

    #[instrument(skip(self, stream))]
    fn handle_request(
        self: Arc<Self>,
        addr: SocketAddr,
        mut stream: TcpStream,
    ) -> ServerResult<()> {
        let http_request = HttpRequest::read_from(&mut stream)?;
        let headers;
        let body = match http_request.request() {
            Request::Index => {
                debug!("Procesando request de Index");
                let body = fs::read_to_string("page/index.html")?;
                headers = hdr!("text/html");
                body.as_bytes().to_owned()
            }
            Request::Data => {
                debug!("Procesando request de Data");
                let data = self.data.read().map_err(|_| LOCK_ERR)?;
                headers = None;
                data.as_bytes().to_owned()
            }
            Request::Favicon => {
                debug!("Procesando request de Favicon");
                let body =
                    fs::read("page/favicon.ico").map_err(|e| format!("Error de favicon: {}", e))?;
                headers = hdr!("image/x-icon");
                body
            }
            Request::Css(filename) => {
                debug!("Procesando request de CSS: {}", filename);
                let body = fs::read(format!("page/resources/css/{}", filename))
                    .map_err(|e| format!("Error de css: {}", e))?;
                headers = hdr!("text/css");
                body
            }
            Request::Image(filename) => {
                debug!("Procesando request de imagen: {}", filename);
                let body = fs::read(format!("page/resources/image/{}", filename))
                    .map_err(|e| format!("Error de imagen: {}", e))?;
                headers = hdr!("image/png");
                body
            }
        };
        let response = HttpResponse::new(
            crate::messages::HttpStatusCode::Ok,
            crate::messages::HttpVersion::V1_1,
            headers,
            Some(body),
        );
        response.send_to(&mut stream)?;
        Ok(())
    }
}
