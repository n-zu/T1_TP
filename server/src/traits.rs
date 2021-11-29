use std::{
    io::{self, Read, Write},
    time::Duration,
};

pub trait Id {
    type T;
    fn id(&self) -> Self::T;
}

#[derive(Debug)]
pub struct StreamError {}

pub trait BidirectionalStream: Read + Write + Send + Sync + 'static {
    fn try_clone(&self) -> Result<Self, StreamError>
    where
        Self: Sized;

    fn close(&mut self) -> io::Result<()>;

    fn change_read_timeout(&mut self, dur: Option<Duration>) -> io::Result<()>;

    fn change_write_timeout(&mut self, dur: Option<Duration>) -> io::Result<()>;
}

pub trait ClientAccepter<S> {
    fn new(ip: &str, port: u16) -> io::Result<Self>
    where
        Self: Sized;

    fn accept_client(&self) -> io::Result<S>;
}
