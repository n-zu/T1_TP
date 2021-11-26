mod decoding;
mod encoding;
#[cfg(test)]
mod tests;

#[doc(hidden)]
const RESERVED_BITS: u8 = 0b00000000;

/// The PingReq packet is sent from a Client to the Server,
/// during the Keep Alive process
#[derive(Debug)]
pub struct PingReq;
