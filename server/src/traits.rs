use std::{io, time::Duration};

pub trait Close {
    fn close(&mut self) -> io::Result<()>;
}

pub trait TryClone {
    fn try_clone(&self) -> Option<Self>
    where
        Self: Sized;
}

/// Config trait for the server
pub trait Config: Send + Sync + Clone + 'static {
    /// Returns the port to be connected
    fn port(&self) -> u16;

    /// Returns dump info, if specified. This info is
    /// a tuple with the dump path and time interval.
    /// Otherwise, it returns None
    fn dump_info(&self) -> Option<(&str, Duration)>;

    /// Returns the path to the logs directory
    fn log_path(&self) -> &str;

    /// Returns the path to CSV file with the accounts
    /// Format: username,password
    fn accounts_path(&self) -> &str;

    /// Returns the IP address of the server
    fn ip(&self) -> &str;
}
