use std::{
    fmt, io,
    net::{Shutdown, TcpStream},
    time::Duration,
};

pub trait Close {
    fn close(&mut self) -> io::Result<()>;
}

pub trait TryClone {
    fn try_clone(&self) -> io::Result<Self>
    where
        Self: Sized;
}

pub trait Interrupt {
    fn alert(&mut self, when: Duration) -> io::Result<()>;

    fn sleep(&mut self) -> io::Result<()>;
}

#[derive(Debug, PartialEq)]
pub enum LoginResult {
    UsernameNotFound,
    InvalidPassword,
    Accepted,
}

pub trait Login: fmt::Debug + Send + Sync + 'static {
    fn login(&mut self, user_name: &str, password: &str) -> io::Result<LoginResult>;
}

impl TryClone for TcpStream {
    fn try_clone(&self) -> io::Result<Self>
    where
        Self: Sized,
    {
        TcpStream::try_clone(self)
    }
}

impl Interrupt for TcpStream {
    #[inline(always)]
    fn alert(&mut self, when: Duration) -> io::Result<()> {
        self.set_nonblocking(false)?;
        self.set_read_timeout(Some(when))
    }
    #[inline(always)]
    fn sleep(&mut self) -> io::Result<()> {
        self.set_nonblocking(true)?;
        self.set_read_timeout(None)
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

    /// Returns the IP address of the server
    fn ip(&self) -> &str;

    fn authenticator(&self) -> Option<Box<dyn Login>>;
}
