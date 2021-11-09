use std::{error::Error, fmt::Display};

use packets::packet_reader::PacketError;
use threadpool::ThreadPoolError;

#[derive(Debug)]
pub struct ClientError {
    msg: String,
}

impl Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl Error for ClientError {
    fn description(&self) -> &str {
        &self.msg
    }
}

impl ClientError {
    pub fn new(msg: &str) -> ClientError {
        ClientError {
            msg: msg.to_string(),
        }
    }
}

impl From<PacketError> for ClientError {
    fn from(err: PacketError) -> ClientError {
        ClientError::new(&format!("Error parseando paquete del servidor: {}", err))
    }
}

impl From<std::io::Error> for ClientError {
    fn from(err: std::io::Error) -> ClientError {
        ClientError::new(&format!("Error inesperado: {}", err))
    }
}

impl From<ThreadPoolError> for ClientError {
    fn from(err: ThreadPoolError) -> ClientError {
        ClientError::new(&format!("Error inesperado: {}", err))
    }
}