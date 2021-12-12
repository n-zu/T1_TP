use std::io::{self};

use crate::{
    server::{server_error::ServerErrorKind, ServerError, ServerResult},
    traits::{Close, TryClone},
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
        if let Some(stream_copy) = self.stream.try_clone() {
            Ok(Self {
                stream: stream_copy,
                id: self.id,
            })
        } else {
            Err(ServerError::new_kind(
                "Error clonando stream de network_connection",
                ServerErrorKind::Irrecoverable,
            ))
        }
    }
}
