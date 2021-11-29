use std::{net::SocketAddr, thread::ThreadId};

use packets::helpers::PacketType;
use tracing::info;

use crate::server::{ClientIdArg, ServerError};

#[non_exhaustive]
pub enum LogKind<'a> {
    StartingServer(SocketAddr, u16),
    Connecting(ThreadId),
    Connected(ThreadId, &'a ClientIdArg),
    KeepAliveTimeout(&'a ClientIdArg),
    UnexpectedError(ThreadId, String),
    ConnectionRefusedError(&'a ClientIdArg, String),
    PacketProcessing(PacketType),
    SuccesfulClientEnd(ThreadId),
    Acknowledge(&'a ClientIdArg, u16),
    SendingUnacknowledged(&'a ClientIdArg, u16),
    Reconnecting(&'a ClientIdArg),
    ServerShutdown,
}

pub fn log(log_kind: LogKind) {
    match log_kind {
        LogKind::StartingServer(addr, port) => info!("Iniciando servidor: {}:{}", addr, port),
        LogKind::Connecting(_) => todo!(),
        LogKind::Connected(_, _) => todo!(),
        LogKind::KeepAliveTimeout(_) => todo!(),
        LogKind::UnexpectedError(_, _) => todo!(),
        LogKind::ConnectionRefusedError(_, _) => todo!(),
        LogKind::PacketProcessing(_) => todo!(),
        LogKind::SuccesfulClientEnd(_) => todo!(),
        LogKind::Acknowledge(_, _) => todo!(),
        LogKind::SendingUnacknowledged(_, _) => todo!(),
        LogKind::Reconnecting(_) => todo!(),
        LogKind::ServerShutdown => todo!(),
    }
}