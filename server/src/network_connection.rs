use std::{
    io::{self},
    time::Duration,
};

use crate::{
    server::{server_error::ServerErrorKind, ServerError, ServerResult},
    traits::{Close, Interrupt, TryClone},
};

/// Information related to the current session of
/// the client
#[derive(Debug)]
pub struct NetworkConnection<S, I> {
    id: I,
    stream: S,
}

impl<S, I> NetworkConnection<S, I> {
    pub fn stream(&self) -> &S {
        &self.stream
    }
    pub fn stream_mut(&mut self) -> &mut S {
        &mut self.stream
    }
    pub fn id(&self) -> &I {
        &self.id
    }
}

impl<S: io::Read, I> io::Read for NetworkConnection<S, I> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buf)
    }
}

impl<S: io::Write, I> io::Write for NetworkConnection<S, I> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

impl<S: Interrupt, I> Interrupt for NetworkConnection<S, I> {
    fn alert(&mut self, when: Duration) -> io::Result<()> {
        self.stream.alert(when)
    }

    fn sleep(&mut self) -> io::Result<()> {
        self.stream.sleep()
    }
}

impl<S, I> NetworkConnection<S, I> {
    pub fn new(id: I, stream: S) -> Self {
        Self { id, stream }
    }

    pub fn close(&mut self) -> io::Result<()>
    where
        S: Close,
    {
        self.stream.close()
    }

    pub fn try_clone(&self) -> ServerResult<Self>
    where
        I: Clone + Copy,
        S: TryClone,
    {
        let stream = self.stream.try_clone().map_err(|e| {
            ServerError::new_kind(
                &format!("Error clonando stream de network_connection: {}", e),
                ServerErrorKind::Irrecoverable,
            )
        })?;
        Ok(NetworkConnection {
            id: self.id,
            stream,
        })
    }
}
