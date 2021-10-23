use std::{
    io::{self, Read, Write},
    net::TcpStream,
};

use crate::connack::Connack;
use crate::connect::Connect;

pub struct Client {
    stream: TcpStream,
}

impl Client {
    pub fn new(address: &str) -> io::Result<Client> {
        Ok(Client {
            stream: TcpStream::connect(address)?,
        })
    }

    pub fn connect(&mut self, connect: Connect) -> Result<(), String> {
        self.stream
            .write_all(&connect.encode())
            .map_err(|err| -> String { err.to_string() })?;

        let mut buf = [0; 2];
        self.stream
            .read_exact(&mut buf)
            .map_err(|err| -> String { err.to_string() })?;

        Connack::new(buf, &mut self.stream)?;

        Ok(())
    }
}
