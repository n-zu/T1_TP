use std::{
    io::{self, Write},
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

        match Connack::read_from(&mut self.stream) {
            Ok(connack_packet) => {
                println!("Llego bien el connack packet, {:?}", connack_packet);
            }
            Err(err) => {
                println!("Error: {:?}", err);
            }
        }

        Ok(())
    }
}
