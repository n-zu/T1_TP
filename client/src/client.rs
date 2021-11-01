use std::thread;
use std::time::Duration;
use std::{io::Write, net::TcpStream};

use packets::packet_reader::PacketError;

use crate::connack::Connack;
use crate::connect::Connect;
use crate::publish::{Publish};
use crate::subscribe::Subscribe;

pub struct Client {
    stream: TcpStream,
}

impl Client {
    pub fn new(address: &str) -> Result<Client, PacketError> {
        Ok(Client {
            stream: TcpStream::connect(address)?,
        })
    }

    pub fn connect(&mut self, connect: Connect) -> Result<(), PacketError> {
        self.stream.write_all(&connect.encode())?;
        println!("Mandando connect");
        thread::sleep(Duration::from_millis(5));
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

    pub fn subscribe(&mut self, subscribe: Subscribe) -> Result<(), PacketError> {
        self.stream.write_all(&subscribe.encode()?)?;
        Ok(())
    }

    pub fn publish(&mut self, publish: Publish) -> Result<(), PacketError> {
        self.stream.write_all(&publish.encode()?)?;
        Ok(())
    }
}
