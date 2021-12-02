use std::io;

use crate::{
    server::{server_error::ServerErrorKind, ServerError, ServerResult},
    traits::{BidirectionalStream, Id},
};

/// Information related to the current session of
/// the client
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
    pub fn id(&self) -> I
    where
        I: Clone + Copy,
    {
        self.id
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

impl<S, I: Clone + Copy> Id for NetworkConnection<S, I> {
    type T = I;

    fn id(&self) -> Self::T {
        self.id
    }
}

impl<S, I> NetworkConnection<S, I>
where
    S: BidirectionalStream,
    I: Id,
{
    pub fn new(id: I, stream: S) -> Self {
        Self { id, stream }
    }

    pub fn close(&mut self) -> io::Result<()> {
        self.stream.close()
    }

    pub fn try_clone(&self) -> ServerResult<Self>
    where
        I: Clone + Copy,
    {
        if let Ok(stream_copy) = self.stream.try_clone() {
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
