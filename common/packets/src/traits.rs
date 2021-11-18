use std::io::Read;

use crate::packet_error::PacketResult;

pub type MQTTBytes = Vec<u8>;

pub trait MQTTEncoding {
    fn encode(&self) -> PacketResult<MQTTBytes>;
}

pub trait MQTTDecoding {
    fn read_from(bytes: &mut impl Read, control_byte: u8) -> PacketResult<Self>
    where
        Self: Sized;
}

/*
pub trait PacketVisitor {
    fn visit(self, server: impl PacketAccepter);
}

pub trait PacketAccepter {
    fn accept_publish(&self, publish: Publish);
    fn accept_puback(&self, puback: Puback);
    fn accept_subscribe(&self, subscribe: Subscribe);
    fn accept_suback(&self, suback: Suback);
}
*/
