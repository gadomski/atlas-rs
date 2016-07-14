//! ATLAS is a remote monitoring system at the Helheim Glacier in southeast Greenland.

pub mod sutron;

#[derive(Debug)]
pub enum Error {
    BadSutronLogHeader(String),
    Io(std::io::Error),
    SutronLogTooShort,
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error::Io(err)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
