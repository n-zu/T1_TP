use std::io;

use crate::traits::Id;

/// Information related to the current session of
/// the client
pub struct ConnectionStream<S, I> {
    id: I,
    stream: S,
}

impl<S, I> ConnectionStream<S, I> {
    pub fn stream(&self) -> &S {
        &self.stream
    }
    pub fn stream_mut(&mut self) -> &mut S {
        &mut self.stream
    }
}

impl<S: io::Read, I> io::Read for ConnectionStream<S, I> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buf)
    }
}

impl<S: io::Write, I> io::Write for ConnectionStream<S, I> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

impl<S, I: Clone + Copy> Id for ConnectionStream<S, I> {
    type T = I;

    fn id(&self) -> Self::T {
        self.id
    }
}

impl<S, I> ConnectionStream<S, I> {
    pub fn new(id: I, stream: S) -> Self {
        Self { id, stream }
    }
}
