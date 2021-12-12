use std::{
    fmt, io,
    net::{Shutdown, TcpStream},
    time::Duration,
};

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

/// Config trait for the server
pub trait Config: Send + Sync + Clone + 'static {
    /// Returns the port to be connected
    fn port(&self) -> u16;

    /// Returns dump info, if specified. This info is
    /// a tuple with the dump path and time interval.
    /// Otherwise, it returns None
    fn dump_info(&self) -> Option<(&str, Duration)>;

    /// Returns the path to the logs directory
    fn log_path(&self) -> &str;

    /// Returns the path to CSV file with the accounts
    /// Format: username,password
    fn accounts_path(&self) -> &str;

    /// Returns the IP address of the server
    fn ip(&self) -> &str;
}
