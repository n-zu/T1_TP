use std::{
    io::{self, Read},
    time::Duration,
};

use serde::{Deserialize, Serialize};

use crate::traits::{Close, Interrupt, TryClone};

#[derive(Debug, Serialize, Deserialize)]
pub struct IOMock {
    pub closed: bool,
    pub buf: Vec<u8>,
}

impl io::Read for IOMock {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let bytes = Read::read(&mut &self.buf[..], buf)?;
        self.buf.drain(..bytes);
        Ok(bytes)
    }
}

impl io::Write for IOMock {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if !self.closed {
            self.buf.extend(buf);
            Ok(buf.len())
        } else {
            Err(io::Error::from(io::ErrorKind::UnexpectedEof))
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if !self.closed {
            Ok(())
        } else {
            Err(io::Error::from(io::ErrorKind::UnexpectedEof))
        }
    }
}

impl Close for IOMock {
    fn close(&mut self) -> io::Result<()> {
        self.closed = true;
        Ok(())
    }
}

impl TryClone for IOMock {
    fn try_clone(&self) -> io::Result<Self>
    where
        Self: Sized,
    {
        Ok(Self {
            closed: self.closed,
            buf: self.buf.clone(),
        })
    }
}

impl Interrupt for IOMock {
    fn alert(&mut self, _when: Duration) -> io::Result<()> {
        Ok(())
    }

    fn sleep(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl IOMock {
    pub fn new() -> Self {
        Self {
            closed: false,
            buf: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    use super::IOMock;

    #[test]
    fn test_read_write_one_byte() {
        let mut iomock = IOMock::new();

        let byte = [5u8];

        iomock.write_all(&byte).unwrap();
        let mut buf = [0u8];
        iomock.read_exact(&mut buf).unwrap();

        assert_eq!(buf[0], 5);
    }

    #[test]
    fn test_read_write_multiple_bytes() {
        let mut iomock = IOMock::new();

        let byte = [5u8, 10u8];

        iomock.write_all(&byte).unwrap();
        let mut buf = [0u8; 2];
        iomock.read_exact(&mut buf).unwrap();

        assert_eq!(buf[0], 5);
        assert_eq!(buf[1], 10);
    }

    #[test]
    fn test_read_write_multiple_bytes_separately() {
        let mut iomock = IOMock::new();

        let byte = [5u8, 10u8];

        iomock.write_all(&byte).unwrap();
        let mut buf = [0u8];

        iomock.read_exact(&mut buf).unwrap();
        assert_eq!(buf[0], 5);
        iomock.read_exact(&mut buf).unwrap();
        assert_eq!(buf[0], 10);
    }
}
