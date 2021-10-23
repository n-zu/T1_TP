const MAX_PAYLOAD_FIELD_LEN: usize = 65535;
const CONNECT_FIXED_HEADER_TYPE: u8 = 0x01;
const CONNECT_FIXED_HEADER_FLAGS: u8 = 0x00;
const SHIFT: u8 = 4;
const PROTOCOL_LEVEL_3_1_1: u8 = 0x04;
 
 
enum QoSLevel {
    QoSLevel0,
    QoSLevel1
}
 
struct LastWill {
    retain: bool,
    qos: QoSLevel,
    message: String
}
 
pub struct Connect {
    client_id: String,
    clean_session: bool,
    user_name: Option<String>,
    password: Option<String>,
    last_will: Option<LastWill>,
    keep_alive: u16,
}
 
pub struct ConnectBuilder {
    connect: Connect
}
 
impl Connect {
    pub fn new(headers : &[u8], stream : Read) {
        
    }

    fn protocol_name(&self) -> Vec<u8> {
        //MQTT
    }
 
    fn control_byte(&self) -> u8 {
        0x10
    }
 
    fn protocol_level(&self) -> u8 {
        PROTOCOL_LEVEL_3_1_1
    }
 
    fn flags(&self) -> u8 {
        let mut flags = 0;
        if self.user_name.is_some() {
            flags |= 0x80;
        }
        if self.password.is_some() {
            flags |= 0x40;
        }
        if self.last_will.is_some() {
            if let Some(_) = self.last_will {
                flags |= 0x20;
            }
            match self.last_will.as_ref().unwrap().qos {
                QoSLevel::QoSLevel0 => flags |= 0x00,
                QoSLevel::QoSLevel1 => flags |= 0x08,
            }
            flags |= 0x04;
        }
        if self.clean_session {
            flags |= 0x02;
        }
 
        flags
    }
}
 
fn check_payload_field_length(field: &str) -> Result<(), CustomError> {
    if field.as_bytes().len() > MAX_PAYLOAD_FIELD_LEN {
        return Err(CustomError { kind: ErrorKind::ExceededPayloadFieldLength });
    }
    Ok(())
}
 
impl ConnectBuilder {
    fn new(client_id: &str, keep_alive: u16, clean_session: bool) -> Result<Self, CustomError> {
        check_payload_field_length(&client_id)?;
        Ok(ConnectBuilder{
            connect: Connect {
                client_id: client_id.to_owned(),
                clean_session,
                user_name: None,
                password: None,
                last_will: None,
                keep_alive
            }
        })
    }
 
    fn user_name(mut self, user_name: &str) -> Result<Self, CustomError> {
        check_payload_field_length(&user_name)?;
        self.connect.user_name = Some(user_name.to_owned());
        Ok(self)
    }
 
    fn password(mut self, password: &str) -> Result<Self, CustomError> {
        check_payload_field_length(&password)?;
        self.connect.password = Some(password.to_owned());
        Ok(self)
    }
 
    fn last_will(mut self, last_will: LastWill) -> Result<Self, CustomError> {
        self.connect.last_will = Some(last_will);
        Ok(self)
    }
 
    fn build(self) -> Result<Connect, CustomError> {
        //TODO: todos los chequeos
        if self.connect.password.is_some() && self.connect.user_name.is_none() {
            return Err(CustomError { kind: ErrorKind::TmpErrorKind});
        }
        Ok(self.connect)
    }
}
 