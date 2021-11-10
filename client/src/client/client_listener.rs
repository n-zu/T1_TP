use std::{
    io::{self, Read, Write},
    net::TcpStream,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use packets::{
    packet_reader::{ErrorKind, PacketError},
    puback::Puback,
    publish::Publish,
    suback::Suback,
};

use crate::{
    client::PendingAck,
    client_packets::{Connack, ConnackError},
    observer::Observer,
};

use crate::observer::Message;

use super::{ClientError, STOP_TIMEOUT};

pub struct Listener<T: Observer> {
    stream: TcpStream,
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

impl<T: Observer> Listener<T> {
    pub fn new(
        stream: TcpStream,
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
            Ok(packet) => {
                match packet {
                    PacketType::Publish => self.handle_publish(header),
                    PacketType::Puback => self.handle_puback(header),
                    PacketType::Suback => self.handle_suback(header),
                    /*PacketType::Unsuback => self.handle_unsuback(),
                    PacketType::Pingresp => self.handle_pingresp(),*/
                    PacketType::Connack => self.handle_connack(header),
                    _ => Err(ClientError::new("Received an unsupported packet type")),
                }
            }
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
        if let Some(id) = id_opt {
            self.stream.write_all(&Puback::new(id)?.encode())?;
        }

        Ok(())
    }

    fn handle_connack(&mut self, header: u8) -> Result<(), ClientError> {
        let connack = Connack::read_from(&mut self.stream, header);
        if let Err(ConnackError::WrongEncoding(str)) = connack {
            return Err(ClientError::new(&format!(
                "Error parseando Connack: {}",
                str
            )));
        }

        // Si no estoy esperando un connack lo ignoro
        let mut lock = self.pending_ack.lock()?;
        if let Some(PendingAck::Connect(_)) = lock.as_ref() {
            match connack {
                Err(err) => {
                    self.observer
                        .update(Message::Connected(Err(ClientError::from(err))));
                    self.stop.store(true, Ordering::Relaxed);
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
        // Si no estoy esperando un suback lo ignoro
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
        // Si no estoy esperando un puback lo ignoro
        if let Some(PendingAck::Publish(publish)) = lock.as_ref() {
            // Este unwrap no falla ya que lo checkeo al ponerlo en el lock
            if *publish.packet_id().unwrap() == puback.packet_id() {
                lock.take();
                self.observer.update(Message::Published(Ok(Some(puback))));
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
