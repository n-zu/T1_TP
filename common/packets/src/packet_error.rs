use std::error::Error;
use std::{fmt, io};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    InvalidProtocol,
    InvalidProtocolLevel,
    InvalidFlags,
    InvalidReservedBits,
    ClientDisconnected,
    InvalidQoSLevel,
    InvalidDupFlag,
    InvalidControlPacketType,
    ErrorAtReadingPacket,
    TopicNameMustBeAtLeastOneCharacterLong,
    TopicNameMustNotHaveWildcards,
    InvalidReturnCode,
    WouldBlock,
    UnexpectedEof,
    UnacceptableProtocolVersion,
    IdentifierRejected,
    ServerUnavailable,
    BadUserNameOrPassword,
    NotAuthorized,
    Other,
}

#[derive(Debug, PartialEq)]
pub struct PacketError {
    msg: String,
    kind: ErrorKind,
}

impl Default for PacketError {
    fn default() -> Self {
        Self::new()
    }
}

const DEFAULT_MSG: &str = "Invalid packet encoding";

impl PacketError {
    pub fn new() -> PacketError {
        PacketError {
            msg: DEFAULT_MSG.to_string(),
            kind: ErrorKind::Other,
        }
    }

    pub fn new_msg(msg: &str) -> PacketError {
        PacketError {
            msg: msg.to_string(),
            kind: ErrorKind::Other,
        }
    }

    pub fn new_kind(msg: &str, kind: ErrorKind) -> PacketError {
        PacketError {
            msg: msg.to_string(),
            kind,
        }
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
}

impl fmt::Display for PacketError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl Error for PacketError {
    fn description(&self) -> &str {
        &self.msg
    }
}

impl From<io::Error> for PacketError {
    fn from(error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::UnexpectedEof => {
                PacketError::new_kind(&error.to_string(), ErrorKind::UnexpectedEof)
            }
            io::ErrorKind::WouldBlock => {
                PacketError::new_kind(&error.to_string(), ErrorKind::WouldBlock)
            }
            _ => PacketError::new_msg(&error.to_string()),
        }
    }
}

impl From<PacketError> for String {
    fn from(error: PacketError) -> Self {
        error.to_string()
    }
}
