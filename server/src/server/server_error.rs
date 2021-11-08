use std::{
    collections::HashMap,
    fmt, io,
    sync::{Mutex, MutexGuard, PoisonError, RwLockReadGuard, RwLockWriteGuard},
};

use packets::packet_reader::{ErrorKind, PacketError};
use threadpool::{ThreadPool, ThreadPoolError};

use crate::{
    client::Client, session::Session, topic_handler::topic_handler_error::TopicHandlerError,
};

#[derive(Debug)]
pub struct ServerError {
    msg: String,
    kind: ServerErrorKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerErrorKind {
    ProtocolViolation,
    ClientDisconnected,
    ClientNotFound,
    Timeout,
    PoinsonedLock,
    Irrecoverable,
    Other,
    _NonExhaustive,
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

impl From<PoisonError<RwLockReadGuard<'_, HashMap<String, Mutex<Client>>>>> for ServerError {
    fn from(err: PoisonError<RwLockReadGuard<'_, HashMap<String, Mutex<Client>>>>) -> ServerError {
        ServerError::new_kind(&err.to_string(), ServerErrorKind::PoinsonedLock)
    }
}

impl From<PoisonError<RwLockWriteGuard<'_, HashMap<String, Mutex<Client>>>>> for ServerError {
    fn from(err: PoisonError<RwLockWriteGuard<'_, HashMap<String, Mutex<Client>>>>) -> ServerError {
        ServerError::new_kind(&err.to_string(), ServerErrorKind::PoinsonedLock)
    }
}

impl From<PoisonError<RwLockReadGuard<'_, Session>>> for ServerError {
    fn from(err: PoisonError<RwLockReadGuard<Session>>) -> ServerError {
        ServerError::new_kind(&err.to_string(), ServerErrorKind::PoinsonedLock)
    }
}

impl From<PoisonError<MutexGuard<'_, Client>>> for ServerError {
    fn from(err: PoisonError<MutexGuard<'_, Client>>) -> Self {
        ServerError::new_kind(&err.to_string(), ServerErrorKind::PoinsonedLock)
    }
}

impl From<PoisonError<MutexGuard<'_, ThreadPool>>> for ServerError {
    fn from(err: PoisonError<MutexGuard<'_, ThreadPool>>) -> Self {
        ServerError::new_kind(&err.to_string(), ServerErrorKind::PoinsonedLock)
    }
}

impl From<PoisonError<RwLockWriteGuard<'_, Session>>> for ServerError {
    fn from(err: PoisonError<RwLockWriteGuard<'_, Session>>) -> Self {
        ServerError::new_kind(&err.to_string(), ServerErrorKind::PoinsonedLock)
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
