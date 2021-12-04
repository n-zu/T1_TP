use std::{
    error::Error,
    fmt::Display,
    sync::{mpsc::SendError, PoisonError, RwLockReadGuard, RwLockWriteGuard},
};

use super::Message;

#[derive(Debug)]
pub struct TopicHandlerError {
    msg: String,
}

impl Display for TopicHandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl Error for TopicHandlerError {
    fn description(&self) -> &str {
        &self.msg
    }
}

impl TopicHandlerError {
    pub fn new(msg: &str) -> TopicHandlerError {
        TopicHandlerError {
            msg: msg.to_string(),
        }
    }
}

const DEFAULT_MSG: &str = "TopicHandlerError: No se pudo desbloquear contenido del Topic";

impl<T> From<PoisonError<RwLockReadGuard<'_, T>>> for TopicHandlerError {
    fn from(err: PoisonError<RwLockReadGuard<T>>) -> TopicHandlerError {
        TopicHandlerError::new(&format!("{} ({})", DEFAULT_MSG, err))
    }
}

impl<T> From<PoisonError<RwLockWriteGuard<'_, T>>> for TopicHandlerError {
    fn from(err: PoisonError<RwLockWriteGuard<T>>) -> TopicHandlerError {
        TopicHandlerError::new(&format!("{} ({})", DEFAULT_MSG, err))
    }
}

impl From<SendError<Message>> for TopicHandlerError {
    fn from(err: SendError<Message>) -> TopicHandlerError {
        TopicHandlerError::new(&format!("No se pudo enviar paquete al servidor ({})", err))
    }
}