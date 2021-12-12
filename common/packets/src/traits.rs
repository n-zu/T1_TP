use std::io::Read;

use crate::packet_error::PacketResult;

pub type MQTTBytes = Vec<u8>;

pub trait MQTTEncoding {
    fn encode(&self) -> PacketResult<MQTTBytes>;
}

pub trait MQTTDecoding {
    fn read_from<T: Read>(bytes: &mut T, control_byte: u8) -> PacketResult<Self>
    where
        Self: Sized;
}
