use std::{
    error::Error,
    fmt::Display,
    sync::{mpsc::SendError, PoisonError, RwLockReadGuard, RwLockWriteGuard},
};

use super::{Message, Subscribers, Subtopics};

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

impl From<PoisonError<RwLockReadGuard<'_, Subscribers>>> for TopicHandlerError {
    fn from(err: PoisonError<RwLockReadGuard<Subscribers>>) -> TopicHandlerError {
        TopicHandlerError::new(&format!("{} ({})", DEFAULT_MSG, err))
    }
}

impl From<PoisonError<RwLockReadGuard<'_, Subtopics>>> for TopicHandlerError {
    fn from(err: PoisonError<RwLockReadGuard<Subtopics>>) -> TopicHandlerError {
        TopicHandlerError::new(&format!("{} ({})", DEFAULT_MSG, err))
    }
}

impl From<PoisonError<RwLockWriteGuard<'_, Subscribers>>> for TopicHandlerError {
    fn from(err: PoisonError<RwLockWriteGuard<Subscribers>>) -> TopicHandlerError {
        TopicHandlerError::new(&format!("{} ({})", DEFAULT_MSG, err))
    }
}

impl From<PoisonError<RwLockWriteGuard<'_, Subtopics>>> for TopicHandlerError {
    fn from(err: PoisonError<RwLockWriteGuard<Subtopics>>) -> TopicHandlerError {
        TopicHandlerError::new(&format!("{} ({})", DEFAULT_MSG, err))
    }
}

impl From<SendError<Message>> for TopicHandlerError {
    fn from(err: SendError<Message>) -> TopicHandlerError {
        TopicHandlerError::new(&format!("No se pudo enviar paquete al servidor ({})", err))
    }
}
