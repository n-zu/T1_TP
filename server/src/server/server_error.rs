use std::{
    fmt, io,
    sync::{mpsc::SendError, PoisonError},
};

use packets::{
    connack::ConnackReturnCode,
    packet_error::{ErrorKind, PacketError},
};
use threadpool::ThreadPoolError;

use crate::topic_handler::topic_handler_error::TopicHandlerError;

#[derive(Debug)]
#[non_exhaustive]
pub enum Poisonable {
    Client,
    ClientsManager,
    TopicHandler,
    ThreadJoiner,
}

#[non_exhaustive]
#[derive(Debug)]
pub enum ServerFatalErrorKind {
    PoisonedLock(Poisonable),
    IOError(io::Error),
    Other(Box<dyn std::error::Error>),
}

// Errores graves del servidor
// Requieren un tratamiento especial por parte
// de un ErrorHandler dedicado
#[derive(Debug)]
pub struct ServerFatalError {
    kind: ServerFatalErrorKind,
}

impl ServerFatalError {
    pub fn new(kind: ServerFatalErrorKind) -> Self {
        Self { kind }
    }
}

#[derive(Debug)]
pub struct ServerError {
    msg: String,
    kind: ServerErrorKind,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerErrorKind {
    ProtocolViolation,
    ClientDisconnected,
    ClientNotFound,
    ConnectionRefused(ConnackReturnCode),
    Timeout,
    PoinsonedLock,
    Irrecoverable,
    Idle,
    Other,
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl std::error::Error for ServerError {
    fn description(&self) -> &str {
        &self.msg
    }
}

impl From<io::Error> for ServerError {
    fn from(error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::UnexpectedEof => ServerError::new_kind(
                "Se desconecto sin avisar",
                ServerErrorKind::ClientDisconnected,
            ),
            io::ErrorKind::WouldBlock => {
                ServerError::new_kind("Connection timeout", ServerErrorKind::Timeout)
            }
            _ => ServerError::new_msg(&error.to_string()),
        }
    }
}

impl From<PacketError> for ServerError {
    fn from(packet_error: PacketError) -> Self {
        if packet_error.kind() == ErrorKind::WouldBlock {
            ServerError::new_kind(&packet_error.to_string(), ServerErrorKind::Timeout)
        } else {
            ServerError::new_msg(&packet_error.to_string())
        }
    }
}

impl<T> From<PoisonError<T>> for ServerError {
    fn from(err: PoisonError<T>) -> Self {
        ServerError::new_kind(&err.to_string(), ServerErrorKind::PoinsonedLock)
    }
}

impl From<SendError<()>> for ServerError {
    fn from(err: SendError<()>) -> Self {
        ServerError::new_msg(&err.to_string())
    }
}

impl From<TopicHandlerError> for ServerError {
    fn from(err: TopicHandlerError) -> Self {
        ServerError::new_kind(
            &format!("TopicHandlerError: {}", err.to_string()),
            ServerErrorKind::Irrecoverable,
        )
    }
}

impl From<ThreadPoolError> for ServerError {
    fn from(err: ThreadPoolError) -> Self {
        ServerError::new_kind(
            &format!("ThreadPoolError: {}", err.to_string()),
            ServerErrorKind::Irrecoverable,
        )
    }
}

impl ServerError {
    pub fn new_msg(msg: &str) -> ServerError {
        ServerError {
            msg: msg.to_string(),
            kind: ServerErrorKind::Other,
        }
    }

    pub fn new_kind(msg: &str, kind: ServerErrorKind) -> ServerError {
        ServerError {
            msg: msg.to_string(),
            kind,
        }
    }

    pub fn kind(&self) -> ServerErrorKind {
        self.kind
    }
}
