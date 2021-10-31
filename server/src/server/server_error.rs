use std::{fmt, io};

use packets::packet_reader::PacketError;

#[derive(Debug)]
pub struct ServerError {
    msg: String,
    kind: ServerErrorKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerErrorKind {
    ProtocolViolation,
    RepeatedId,
    ClientDisconnected,
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
        ServerError::new_msg(&error.to_string())
    }
}

impl From<ServerError> for io::Error {
    fn from(server_error: ServerError) -> Self {
        io::Error::new(io::ErrorKind::ConnectionAborted, "Cliente muerto")
    }
}

impl From<PacketError> for ServerError {
    fn from(packet_error: PacketError) -> Self {
        ServerError::new_msg(&packet_error.to_string())
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
