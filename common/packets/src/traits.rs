use std::{io::{Read, self}, fmt, net::{TcpStream, Shutdown}};

use crate::packet_error::PacketResult;

pub type MQTTBytes = Vec<u8>;

pub trait MQTTEncoding {
    fn encode(&self) -> PacketResult<MQTTBytes>;
}

pub trait MQTTDecoding {
    fn read_from(bytes: &mut impl Read, control_byte: u8) -> PacketResult<Self>
    where
        Self: Sized;
}

pub trait Close {
    fn close(&mut self) -> io::Result<()>;
}

pub trait TryClone {
    fn try_clone(&self) -> Option<Self>
    where
        Self: Sized;
}

#[derive(Debug, PartialEq)]
pub enum LoginResult {
    UsernameNotFound,
    InvalidPassword,
    Accepted,
}

pub trait Login: fmt::Display + std::error::Error + Send + Sync + 'static {
    fn login(&mut self, user_name: &str, password: &str) -> io::Result<LoginResult>;
}

impl TryClone for TcpStream {
    fn try_clone(&self) -> Option<Self>
    where
        Self: Sized,
    {
        if let Ok(clone) = TcpStream::try_clone(self) {
            Some(clone)
        } else {
            None
        }
    }
}

impl Close for TcpStream {
    fn close(&mut self) -> io::Result<()> {
        self.shutdown(Shutdown::Both)
    }
}
