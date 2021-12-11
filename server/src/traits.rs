use std::{fmt, io};

pub trait Close {
    fn close(&mut self) -> io::Result<()>;
}

pub trait TryClone {
    fn try_clone(&self) -> Option<Self>
    where
        Self: Sized;
}

#[derive(Debug, PartialEq)]
pub enum LoginResult {
    UsernameNotFound,
    InvalidPassword,
    Accepted,
}

pub trait Login: fmt::Display + std::error::Error + Send + Sync + 'static {
    fn login(&mut self, user_name: &str, password: &str) -> io::Result<LoginResult>;
}
