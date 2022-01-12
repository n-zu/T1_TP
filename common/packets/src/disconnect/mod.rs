mod decoding;
mod encoding;
#[cfg(test)]
mod tests;

const RESERVED_BITS: u8 = 0b00000000;

/// The Disconnect packet is the http_final packet sent from the Client to the Server.
/// It indicates that the Client is disconnecting cleanly.
pub struct Disconnect();
