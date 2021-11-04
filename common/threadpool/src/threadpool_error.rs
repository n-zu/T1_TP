use std::{error::Error, fmt::Display, sync::mpsc::SendError};

use super::Job;

#[derive(Debug)]
pub struct ThreadPoolError {
    msg: String,
}

impl Display for ThreadPoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl Error for ThreadPoolError {
    fn description(&self) -> &str {
        &self.msg
    }
}

impl ThreadPoolError {
    pub fn new() -> ThreadPoolError {
        ThreadPoolError {
            msg: "ThreadPoolError: Could not send job".to_string(),
        }
    }
}

impl From<SendError<Job>> for ThreadPoolError {
    fn from(error: SendError<Job>) -> ThreadPoolError {
        ThreadPoolError {
            msg: format!("ThreadPoolError: Could not send job ({})", error),
        }
    }
}

impl Default for ThreadPoolError {
    fn default() -> Self {
        Self::new()
    }
}
