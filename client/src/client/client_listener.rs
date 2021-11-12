use std::{
    io::{self, Read, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use packets::{
    packet_reader::{ErrorKind, PacketError},
    puback::Puback,
    publish::Publish,
    suback::Suback,
};

use crate::{
    client::PendingAck,
    client_packets::{Connack, PingResp, Unsuback},
    observer::Observer,
};

use crate::observer::Message;

use super::{ClientError, STOP_TIMEOUT};

pub trait Stream: Read + Write {
    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()>;
}

pub struct Listener<T: Observer, S: Stream> {
    stream: S,
    pending_ack: Arc<Mutex<Option<PendingAck>>>,
    observer: Arc<T>,
    stop: Arc<AtomicBool>,
}

pub enum PacketType {
    Connect,
    Connack,
    Publish,
    Puback,
    Subscribe,
    Suback,
    Unsubscribe,
    Unsuback,
    Pingreq,
    Pingresp,
    Disconnect,
}

// Bajo que errores no se reintenta conectar
const CONNECT_USER_ERRORS: [ErrorKind; 3] = [
    ErrorKind::BadUserNameOrPassword,
    ErrorKind::NotAuthorized,
    ErrorKind::IdentifierRejected,
];

impl<T: Observer, S: Stream> Listener<T, S> {
    pub fn new(
        stream: S,
        pending_ack: Arc<Mutex<Option<PendingAck>>>,
        observer: Arc<T>,
        stop: Arc<AtomicBool>,
    ) -> Result<Self, ClientError> {
        stream.set_read_timeout(Some(STOP_TIMEOUT))?;

        Ok(Self {
            stream,
            pending_ack,
            observer,
            stop,
        })
    }

    pub fn wait_for_packets(&mut self) {
        while !self.stop.load(Ordering::Relaxed) {
            if let Err(err) = self.try_read_packet() {
                self.stop.store(true, Ordering::Relaxed);
                self.observer.update(Message::InternalError(err));
            }
        }
    }

    fn try_read_packet(&mut self) -> Result<(), ClientError> {
        let mut buf = [0u8; 1];

        match self.stream.read_exact(&mut buf) {
            Ok(()) => {
                println!("Got header: {}", buf[0]);
                self.handle_packet(buf[0])?;
                Ok(())
            }
            Err(err)
                if (err.kind() == io::ErrorKind::TimedOut
                    || err.kind() == io::ErrorKind::WouldBlock) =>
            {
                Ok(())
            }
            Err(err) => Err(ClientError::from(err)),
        }
    }

    fn handle_packet(&mut self, header: u8) -> Result<(), ClientError> {
        match get_code_type(header >> 4) {
            Ok(packet) => match packet {
                PacketType::Publish => self.handle_publish(header),
                PacketType::Puback => self.handle_puback(header),
                PacketType::Suback => self.handle_suback(header),
                PacketType::Unsuback => self.handle_unsuback(header),
                PacketType::Pingresp => self.handle_pingresp(header),
                PacketType::Connack => self.handle_connack(header),
                _ => Err(ClientError::new("Received an unsupported packet type")),
            },
            Err(error) => {
                self.observer
                    .update(Message::InternalError(ClientError::from(error)));
                Ok(())
            }
        }
    }

    fn handle_publish(&mut self, header: u8) -> Result<(), ClientError> {
        let publish = Publish::read_from(&mut self.stream, header)?;
        let id_opt = publish.packet_id().cloned();
        self.observer.update(Message::Publish(publish));

        // Si tiene id no es QoS 0
        if let Some(id) = id_opt {
            self.stream.write_all(&Puback::new(id)?.encode())?;
        }

        Ok(())
    }

    fn handle_connack(&mut self, header: u8) -> Result<(), ClientError> {
        let connack = Connack::read_from(&mut self.stream, header);

        let mut lock = self.pending_ack.lock()?;
        if let Some(PendingAck::Connect(_)) = lock.as_ref() {
            match connack {
                Err(err) if CONNECT_USER_ERRORS.contains(&err.kind()) => {
                    lock.take();
                    self.stop.store(true, Ordering::Relaxed);
                    self.observer
                        .update(Message::Connected(Err(ClientError::from(err))));
                }
                Err(err) => {
                    return Err(ClientError::from(err));
                }
                Ok(packet) => {
                    lock.take();
                    self.observer.update(Message::Connected(Ok(packet)));
                }
            }
        }

        Ok(())
    }

    fn handle_suback(&mut self, header: u8) -> Result<(), ClientError> {
        let suback = Suback::read_from(&mut self.stream, header)?;

        let mut lock = self.pending_ack.lock()?;

        if let Some(PendingAck::Subscribe(subscribe)) = lock.as_ref() {
            if subscribe.packet_identifier() == suback.packet_id() {
                lock.take();
                self.observer.update(Message::Subscribed(Ok(suback)));
            }
        }

        Ok(())
    }

    fn handle_puback(&mut self, header: u8) -> Result<(), ClientError> {
        let puback = Puback::read_from(&mut self.stream, header)?;

        let mut lock = self.pending_ack.lock()?;

        if let Some(PendingAck::Publish(publish)) = lock.as_ref() {
            // Este unwrap no falla ya que lo checkeo al ponerlo en el lock
            if *publish.packet_id().unwrap() == puback.packet_id() {
                lock.take();
                self.observer.update(Message::Published(Ok(Some(puback))));
            }
        }

        Ok(())
    }

    fn handle_pingresp(&mut self, header: u8) -> Result<(), ClientError> {
        let _ = PingResp::read_from(&mut self.stream, header)?;

        let mut lock = self.pending_ack.lock()?;

        if let Some(PendingAck::PingReq(_)) = lock.as_ref() {
            lock.take();
        }

        Ok(())
    }

    fn handle_unsuback(&mut self, header: u8) -> Result<(), ClientError> {
        let unsuback = Unsuback::read_from(&mut self.stream, header)?;

        let mut lock = self.pending_ack.lock()?;

        if let Some(PendingAck::Unsubscribe(unsubscribe)) = lock.as_ref() {
            if unsubscribe.packet_id() == unsuback.packet_id() {
                lock.take();
                self.observer.update(Message::Unsubscribed(Ok(unsuback)));
            }
        }

        Ok(())
    }
}

fn get_code_type(code: u8) -> Result<PacketType, PacketError> {
    match code {
        1 => Ok(PacketType::Connect),
        2 => Ok(PacketType::Connack),
        3 => Ok(PacketType::Publish),
        4 => Ok(PacketType::Puback),
        8 => Ok(PacketType::Subscribe),
        9 => Ok(PacketType::Suback),
        10 => Ok(PacketType::Unsubscribe),
        11 => Ok(PacketType::Unsuback),
        12 => Ok(PacketType::Pingreq),
        13 => Ok(PacketType::Pingresp),
        14 => Ok(PacketType::Disconnect),
        _ => Err(PacketError::new_kind(
            &format!("Received invalid packet type: {}", code),
            ErrorKind::InvalidControlPacketType,
        )),
    }
}
