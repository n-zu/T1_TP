use std::{
    fmt, io,
    sync::{mpsc::SendError, PoisonError},
};

use packets::{
    connack::ConnackReturnCode,
    packet_error::{ErrorKind, PacketError},
};
use threadpool::ThreadPoolError;
use tracing::error;

use crate::topic_handler::topic_handler_error::TopicHandlerError;

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
    DumpError,
    Timeout,
    PoinsonedLock,
    Irrecoverable,
    Idle,
    Other,
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}-{}", self.kind, self.msg)
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
            io::ErrorKind::UnexpectedEof
            | io::ErrorKind::NotConnected
            | io::ErrorKind::BrokenPipe
            | io::ErrorKind::ConnectionReset => ServerError::new_kind(
                "Se desconecto sin avisar",
                ServerErrorKind::ClientDisconnected,
            ),
            io::ErrorKind::WouldBlock => {
                ServerError::new_kind("Connection timeout", ServerErrorKind::Timeout)
            }
            _ => ServerError::new_msg(format!("{:?}", error)),
        }
    }
}

impl From<PacketError> for ServerError {
    fn from(packet_error: PacketError) -> Self {
        match packet_error.kind() {
            ErrorKind::WouldBlock => {
                ServerError::new_kind(&packet_error.to_string(), ServerErrorKind::Timeout)
            }
            ErrorKind::UnexpectedEof => ServerError::new_kind(
                &packet_error.to_string(),
                ServerErrorKind::ClientDisconnected,
            ),
            _ => ServerError::new_msg(&format!("packet_error: {:?}", packet_error)),
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
        error!("Error de Sender: {}", err);
        ServerError::new_msg(&err.to_string())
    }
}

impl From<TopicHandlerError> for ServerError {
    fn from(err: TopicHandlerError) -> Self {
        error!("Error de TopicHandler: {}", err);
        ServerError::new_kind(
            &format!("TopicHandlerError: {}", err.to_string()),
            ServerErrorKind::Irrecoverable,
        )
    }
}

impl From<ThreadPoolError> for ServerError {
    fn from(err: ThreadPoolError) -> Self {
        error!("Error de ThreadPool: {}", err);
        ServerError::new_kind(
            &format!("ThreadPoolError: {}", err.to_string()),
            ServerErrorKind::Irrecoverable,
        )
    }
}

impl ServerError {
    pub fn new_msg<T: Into<String>>(msg: T) -> ServerError {
        ServerError {
            msg: msg.into(),
            kind: ServerErrorKind::Other,
        }
    }

    pub fn new_kind<T: Into<String>>(msg: T, kind: ServerErrorKind) -> ServerError {
        ServerError {
            msg: msg.into(),
            kind,
        }
    }

    pub fn kind(&self) -> ServerErrorKind {
        self.kind
    }
}
