use crate::packet_reader::{ErrorKind, PacketError, RemainingLength};

#[derive(Debug, Eq, PartialEq)]
struct Suback {
    return_codes: Vec<u8>,
    subscribe_packet_id: u16,
}

const CONTROL_BYTE: u8 = 0b10010000;
const MSG_INVALID_RETURN_CODE: &str = "Allowed return codes are 0x00, 0x01, 0x80";
const SUCCESS_MAXIMUM_QOS_0: u8 = 0;
const SUCCESS_MAXIMUM_QOS_1: u8 = 1;
const FAILURE: u8 = 0x80;

impl Suback {
    pub fn new(subscribe_packet_id: u16) -> Self {
        Self {
            return_codes: Vec::new(),
            subscribe_packet_id,
        }
    }

    pub fn new_from_vec(
        return_codes: Vec<u8>,
        subscribe_packet_id: u16,
    ) -> Result<Self, PacketError> {
        Self::verify_return_codes_from_vec(&return_codes)?;
        Ok(Self {
            return_codes,
            subscribe_packet_id,
        })
    }

    pub fn encode(&mut self) -> Result<Vec<u8>, PacketError> {
        let mut bytes = vec![];
        bytes.append(&mut self.fixed_header()?);
        bytes.append(&mut self.variable_header());
        bytes.append(&mut self.return_codes);
        Ok(bytes)
    }

    pub fn add_return_code(&mut self, return_code: u8) -> Result<(), PacketError> {
        Self::verify_return_code(&return_code)?;
        self.return_codes.push(return_code);
        Ok(())
    }

    fn verify_return_codes_from_vec(return_codes: &Vec<u8>) -> Result<(), PacketError> {
        for code in return_codes {
            Self::verify_return_code(code)?;
        }
        Ok(())
    }

    fn verify_return_code(return_code: &u8) -> Result<(), PacketError> {
        if !Self::is_return_code_valid(&return_code) {
            return Err(PacketError::new_kind(
                MSG_INVALID_RETURN_CODE,
                ErrorKind::InvalidReturnCode,
            ));
        }
        Ok(())
    }

    fn is_return_code_valid(return_code: &u8) -> bool {
        *return_code == SUCCESS_MAXIMUM_QOS_0
            || *return_code == SUCCESS_MAXIMUM_QOS_1
            || *return_code == FAILURE
    }

    fn fixed_header(&self) -> Result<Vec<u8>, PacketError> {
        let mut fixed_header: Vec<u8> = vec![CONTROL_BYTE];
        let remaining_length =
            RemainingLength::from_uncoded(self.return_codes.len() + self.variable_header().len())?;
        let mut remaining_length_buff = remaining_length.encode();
        fixed_header.append(&mut remaining_length_buff);
        Ok(fixed_header)
    }

    fn variable_header(&self) -> Vec<u8> {
        let mut variable_header: Vec<u8> = vec![];
        variable_header.append(&mut self.subscribe_packet_id.to_be_bytes().to_vec());
        variable_header
    }
}

#[cfg(test)]
mod tests {
    use crate::packet_reader::{ErrorKind, PacketError};
    use crate::suback::{Suback, CONTROL_BYTE, MSG_INVALID_RETURN_CODE};

    #[test]
    fn test_valid_suback_with_return_codes_0_1_1_1() {
        let return_codes: Vec<u8> = vec![0, 1, 1, 1];
        let mut suback = Suback::new_from_vec(return_codes, 1).unwrap();
        let encoded_suback = suback.encode().unwrap();
        let expected: Vec<u8> = vec![CONTROL_BYTE, 6, 0, 1, 0, 1, 1, 1];
        assert_eq!(encoded_suback, expected)
    }

    #[test]
    fn test_valid_suback_with_return_codes_0_0_0_0() {
        let return_codes: Vec<u8> = vec![0, 0, 0, 0];
        let mut suback = Suback::new_from_vec(return_codes, 2).unwrap();
        let expected_suback = suback.encode().unwrap();
        let expected: Vec<u8> = vec![CONTROL_BYTE, 6, 0, 2, 0, 0, 0, 0];
        assert_eq!(expected_suback, expected)
    }
    #[test]
    fn test_suback_with_return_code_65_should_raise_invalid_return_code_error() {
        let return_codes: Vec<u8> = vec![0, 65, 0, 0];
        let result = Suback::new_from_vec(return_codes, 3).unwrap_err();
        let expected_error =
            PacketError::new_kind(MSG_INVALID_RETURN_CODE, ErrorKind::InvalidReturnCode);
        assert_eq!(result, expected_error)
    }
}
