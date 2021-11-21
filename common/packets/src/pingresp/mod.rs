mod decoding;
mod encoding;
#[cfg(test)]
mod tests;

#[doc(hidden)]
const RESERVED_BITS: u8 = 0b00000000;

/// A PingResp Packet is sent by the Server to the Client in response
/// to a PingReq Packet.
/// It indicates that the Server is alive.
pub struct PingResp;
