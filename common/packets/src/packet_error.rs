use std::error::Error;
use std::string::FromUtf8Error;
use std::{fmt, io};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Represents all kind of errors that could appear on processing any type of packet
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
    InvalidTopicName,
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
/// Represents a packet's error. It includes a message and the error's kind
pub struct PacketError {
    msg: String,
    kind: ErrorKind,
}

impl Default for PacketError {
    fn default() -> Self {
        Self::new()
    }
}

#[doc(hidden)]
const DEFAULT_MSG: &str = "Invalid packet encoding";
pub type PacketResult<T> = Result<T, PacketError>;

impl PacketError {
    /// Returns a new PacketError with a default message
    pub fn new() -> PacketError {
        PacketError {
            msg: DEFAULT_MSG.to_string(),
            kind: ErrorKind::Other,
        }
    }

    /// Returns a new PacketError with the given message
    pub fn new_msg(msg: &str) -> PacketError {
        PacketError {
            msg: msg.to_string(),
            kind: ErrorKind::Other,
        }
    }

    /// Returns a new PacketError with the given message and the given kind
    pub fn new_kind(msg: &str, kind: ErrorKind) -> PacketError {
        PacketError {
            msg: msg.to_string(),
            kind,
        }
    }

    /// Returns the kind of the PacketError
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

impl From<FromUtf8Error> for PacketError {
    fn from(error: FromUtf8Error) -> Self {
        PacketError::new_kind(&error.to_string(), ErrorKind::ErrorAtReadingPacket)
    }
}

impl From<PacketError> for String {
    fn from(error: PacketError) -> Self {
        error.to_string()
    }
}
