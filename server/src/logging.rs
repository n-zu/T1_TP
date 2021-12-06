#![allow(dead_code)]

use std::{fmt, thread::ThreadId};

use packets::helpers::PacketType;
use tracing::{debug, error, info, warn};

use crate::server::ServerError;

#[non_exhaustive]
pub enum LogKind<'a, U>
where
    U: fmt::Display + ?Sized,
{
    StartingServer(&'a str, u16),
    AcceptedIncoming(&'a U),
    IncomingConnectionError(&'a str),
    Connecting(&'a U),
    Connected(&'a U),
    KeepAliveTimeout(&'a U),
    UnexpectedError(&'a U, String),
    UnhandledError(ServerError),
    ConnectionRefusedError(&'a U, String),
    PacketProcessing(&'a U, PacketType),
    Publishing(&'a U),
    SuccesfulClientEnd(&'a U),
    Acknowledge(&'a U, u16),
    SendingUnacknowledged(&'a U, u16),
    Reconnecting(&'a U),
    ServerShutdown,
    ThreadStart(ThreadId),
    ThreadEndOk(ThreadId),
    ThreadEndErr(ThreadId, &'a str),
}

pub fn log<U>(log_kind: LogKind<U>)
where
    U: fmt::Display + ?Sized,
{
    match log_kind {
        // En este contexto, id no solo significa client_id,
        // sino que es cualquier identificador que el servidor
        // use en ese momento para el cliente (por ejemplo, el
        // SocketAddr cuando el cliente aun no envio el Connect)
        LogKind::StartingServer(addr, port) => info!("Iniciando servidor: {}:{}", addr, port),
        LogKind::Connecting(id) => info!("<{}>: Conectando", id),
        LogKind::Connected(id) => info!("<{}>: Conectado", id),
        LogKind::AcceptedIncoming(id) => info!("<{}>: Aceptada conexion TCP", id),
        LogKind::IncomingConnectionError(err) => error!("No se pudo aceptar conexion TCP: {}", err),
        LogKind::KeepAliveTimeout(id) => warn!("<{}>: Keep Alive Timeout", id),
        LogKind::UnexpectedError(id, err) => {
            error!("<{}>: Error inesperado: {}", id, err)
        }
        LogKind::UnhandledError(err) => error!("Error no manejado: {:?}", err),
        LogKind::ConnectionRefusedError(id, err) => error!("<{}>: Error de conexion: {}", id, err),
        LogKind::PacketProcessing(id, packet_type) => match packet_type {
            PacketType::Connect => debug!("<{}>: Procesado CONNECT", id),
            PacketType::Publish => debug!("<{}>: Procesado PUBLISH", id),
            PacketType::Puback => debug!("<{}>: Procesado PUBACK", id),
            PacketType::Subscribe => debug!("<{}>: Procesado SUBSCRIBE", id),
            PacketType::Unsubscribe => debug!("<{}>: Procesado UNSUBSCRIBE", id),
            PacketType::PingReq => debug!("<{}>: Procesado PINGREQ", id),
            PacketType::Disconnect => debug!("<{}>: Procesado DISCONNECT", id),
            // Cualquier otro paquete corresponde a una violacion del protocolo,
            // por lo tanto no los procesa el servidor
            _ => unreachable!(),
        },
        LogKind::Publishing(id) => debug!("<{}>: Enviando PUBLISH", id),
        LogKind::SuccesfulClientEnd(id) => debug!("<{}>: Procesado con exito", id),
        LogKind::Acknowledge(id, packet_id) => debug!("<{}>: Acknowledge {}", id, packet_id),
        LogKind::SendingUnacknowledged(id, packet_id) => {
            debug!("<{}>: Reenviando paquete con id <{}>", id, packet_id)
        }
        LogKind::Reconnecting(id) => info!("<{}>: Reconectando", id),
        LogKind::ServerShutdown => info!("Apagando el servidor"),
        LogKind::ThreadStart(thread_id) => debug!("Thread <{:?}> spawneado", thread_id),
        LogKind::ThreadEndOk(thread_id) => debug!("Thread <{:?}> joineado normalmente", thread_id),
        LogKind::ThreadEndErr(thread_id, err) => {
            error!("Thread <{:?}> joineado con panic: {}", thread_id, err)
        }
    }
}
