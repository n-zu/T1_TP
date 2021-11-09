use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::{io::Write, net::TcpStream};

use packets::packet_reader::QoSLevel;

use crate::client_listener::Listener;
use crate::client_packets::{Connect, PingReq, Subscribe};

use crate::client_error::ClientError;
use crate::observer::Observer;
use packets::publish::{DupFlag, Publish};
use threadpool::ThreadPool;

// Cuanto esperar antes de checkear que
// haya que reenviar un paquete con QoS 1
const RESEND_TIMEOUT: Duration = Duration::from_millis(500);

// Cuantas veces reintentar reenviar un paquete
const MAX_RETRIES: u16 = 3;

#[allow(dead_code)]
pub enum PendingAck {
    Subscribe(Subscribe),
    PingReq(PingReq),
    Publish(Publish),
    Connect(Connect),
}

pub struct Client<T: Observer> {
    stream: TcpStream,
    thread_pool: ThreadPool,
    pending_ack: Arc<Mutex<Option<PendingAck>>>,
    observer: T,
    stop: Arc<AtomicBool>,
}

impl<T: 'static + Observer> Client<T> {
    #![allow(dead_code)]
    pub fn new(address: &str, observer: T) -> Result<Client<T>, ClientError> {
        Ok(Client {
            stream: TcpStream::connect(address)?,
            thread_pool: ThreadPool::new(1),
            pending_ack: Arc::new(Mutex::new(None)),
            observer,
            stop: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn connect(&mut self, connect: Connect) -> Result<(), ClientError> {
        let mut listener = Listener::new(
            &mut self.stream,
            self.pending_ack.clone(),
            self.observer.clone(),
            self.stop.clone(),
        )?;

        let bytes = connect.encode();
        self.pending_ack
            .lock()?
            .replace(PendingAck::Connect(connect));

        self.stream.write_all(&bytes)?;
        println!("Enviando connect...");

        self.thread_pool.spawn(move || {
            listener.wait_for_packets();
        })?;

        if !self.wait_for_ack()? {
            ClientError::new("No se pudo establecer la conexión");
        }

        Ok(())
    }

    pub fn subscribe(&mut self, subscribe: Subscribe) -> Result<(), ClientError> {
        self.stream.write_all(&subscribe.encode()?)?;
        *self.pending_ack.lock()? = Some(PendingAck::Subscribe(subscribe));
        if !self.wait_for_ack()? {
            ClientError::new("No se recibió paquete suback");
        }

        Ok(())
    }

    pub fn publish(&mut self, publish: Publish) -> Result<(), ClientError> {
        self.stream.write_all(&publish.encode()?)?;
        if publish.qos() == QoSLevel::QoSLevel1 {
            *self.pending_ack.lock()? = Some(PendingAck::Publish(publish));
            if !self.wait_for_ack()? {
                return Err(ClientError::new("No se recibió paquete puback (QoS 1)"));
            }
        }

        Ok(())
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
    fn wait_for_ack(&mut self) -> Result<bool, ClientError> {
        let mut retries = 0;
        while retries < MAX_RETRIES {
            thread::sleep(RESEND_TIMEOUT);
            let mut pending = self.pending_ack.lock()?;
            match pending.as_mut() {
                None => {
                    return Ok(true);
                }
                Some(pending_ack) => {
                    Self::resend_pending(&mut self.stream, pending_ack)?;
                    retries += 1;
                }
            }
            drop(pending);
        }

        Ok(false)
    }
}

impl<T: Observer> Drop for Client<T> {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}
