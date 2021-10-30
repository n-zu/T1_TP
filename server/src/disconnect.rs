use std::io::Read;

use packets::packet_reader::PacketError;

const DISCONNECT_PACKET_TYPE: u8 = 0b11100000;
const RESERVED_MASK: u8 = 0b00001111;

pub struct Disconnect {}

impl Disconnect {
    pub fn new(stream: &mut impl Read) -> Result<Disconnect, PacketError> {

    }
}