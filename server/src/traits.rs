use std::io;

pub trait Close {
    fn close(&mut self) -> io::Result<()>;
}

pub trait TryClone {
    fn try_clone(&self) -> Option<Self>
    where
        Self: Sized;
}
