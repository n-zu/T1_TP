use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{io::Write, net::TcpStream};
use std::{thread, time};

use packets::packet_reader::QoSLevel;

use crate::client_packets::{Connect, PingReq, Subscribe};
use crate::observer::{Message, Observer};
use packets::publish::{DupFlag, Publish};

use super::{ClientError, PendingAck};

// Cuanto esperar antes de checkear que
// haya que reenviar un paquete con QoS 1
const RESEND_TIMEOUT: Duration = Duration::from_millis(5000);

// Cada cuanto checkear si el listener ya recibió el ACK
const ACK_CHECK: Duration = Duration::from_millis(500);

// Cuantas veces reintentar reenviar un paquete
const MAX_RETRIES: u16 = 3;

pub struct ClientSender<T: Observer> {
    stream: Mutex<TcpStream>,
    pending_ack: Arc<Mutex<Option<PendingAck>>>,
    observer: Arc<T>,
}

impl<T: 'static + Observer> ClientSender<T> {
    #![allow(dead_code)]
    pub fn new(stream: TcpStream, observer: T) -> Self {
        Self {
            stream: Mutex::new(stream),
            pending_ack: Arc::new(Mutex::new(None)),
            observer: Arc::new(observer),
        }
    }

    pub fn stream(&self) -> &Mutex<TcpStream> {
        &self.stream
    }

    pub fn pending_ack(&self) -> Arc<Mutex<Option<PendingAck>>> {
        self.pending_ack.clone()
    }

    pub fn observer(&self) -> Arc<T> {
        self.observer.clone()
    }

    fn _connect(&self, connect: Connect) -> Result<(), ClientError> {
        let bytes = connect.encode();
        self.pending_ack
            .lock()?
            .replace(PendingAck::Connect(connect));

        self.stream.lock()?.write_all(&bytes)?;

        if !self.wait_for_ack()? {
            return Err(ClientError::new("No se pudo establecer la conexión"));
        }

        Ok(())
    }

    pub fn send_connect(&self, connect: Connect, failure_stop: Arc<AtomicBool>) {
        if let Err(err) = self._connect(connect) {
            failure_stop.store(true, std::sync::atomic::Ordering::Relaxed);
            self.observer.update(Message::Connected(Err(err)));
        }
    }

    pub fn _subscribe(&self, subscribe: Subscribe) -> Result<(), ClientError> {
        let bytes = subscribe.encode()?;
        self.pending_ack
            .lock()?
            .replace(PendingAck::Subscribe(subscribe));
        self.stream.lock()?.write_all(&bytes)?;
        if !self.wait_for_ack()? {
            ClientError::new("No se recibió paquete suback");
        }

        Ok(())
    }

    pub fn send_subscribe(&self, subscribe: Subscribe) {
        if let Err(err) = self._subscribe(subscribe) {
            self.observer.update(Message::Subscribed(Err(err)));
        }
    }

    pub fn _publish(&self, publish: Publish) -> Result<(), ClientError> {
        let bytes = publish.encode()?;
        if publish.qos() == QoSLevel::QoSLevel1 {
            *self.pending_ack.lock()? = Some(PendingAck::Publish(publish));
            if !self.wait_for_ack()? {
                return Err(ClientError::new("No se recibió paquete puback (QoS 1)"));
            }
        }
        self.stream.lock()?.write_all(&bytes)?;

        Ok(())
    }

    pub fn send_publish(&self, publish: Publish) {
        if let Err(err) = self._publish(publish) {
            self.observer.update(Message::Published(Err(err)));
        }
    }

    fn _pingreq(&self, pingreq: PingReq) -> Result<(), ClientError> {
        let bytes = pingreq.encode();
        self.pending_ack
            .lock()?
            .replace(PendingAck::PingReq(pingreq));
        self.stream.lock()?.write_all(&bytes)?;
        if !self.wait_for_ack()? {
            ClientError::new("El servidor no respondió al pingreq");
        }

        Ok(())
    }

    pub fn send_pingreq(&self) {
        let pingreq = PingReq::new();
        if let Err(err) = self._pingreq(pingreq) {
            self.observer.update(Message::InternalError(err));
        }
    }

    fn resend_pending(
        stream: &mut TcpStream,
        pending_ack: &mut PendingAck,
    ) -> Result<(), ClientError> {
        // TODO: realmente deberiamos hacer de una vez el trait de encode asi evitamos esto
        match pending_ack {
            PendingAck::Subscribe(subscribe) => {
                stream.write_all(&subscribe.encode()?)?;
            }
            PendingAck::PingReq(ping_req) => {
                stream.write_all(&ping_req.encode())?;
            }
            PendingAck::Publish(publish) => {
                publish.set_dup_flag(DupFlag::DupFlag1);
                stream.write_all(&publish.encode()?)?;
            }
            PendingAck::Connect(connect) => {
                stream.write_all(&connect.encode())?;
            }
        }
        Ok(())
    }

    // Devuelve verdadero si se pudo mandar, falso si no se recibió el ack
    fn wait_for_ack(&self) -> Result<bool, ClientError> {
        let mut retries = 0;
        let mut stream = self.stream.lock()?;
        let mut last = time::Instant::now();

        while retries < MAX_RETRIES {
            thread::sleep(ACK_CHECK);

            match self.pending_ack.lock()?.as_mut() {
                None => {
                    return Ok(true);
                }
                Some(pending_ack) => {
                    let now = time::Instant::now();
                    if last + RESEND_TIMEOUT < now {
                        Self::resend_pending(&mut stream, pending_ack)?;
                        last = time::Instant::now();
                    }
                }
            }
            retries += 1;
        }

        Ok(false)
    }
}
