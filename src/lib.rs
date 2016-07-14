//! ATLAS is a remote monitoring system at the Helheim Glacier in southeast Greenland.

extern crate chrono;

pub mod sutron;

#[derive(Debug)]
pub enum Error {
    BadSutronLogHeader(String),
    ChronoParse(chrono::ParseError),
    Io(std::io::Error),
    SutronRecordTooShort(usize),
    SutronRecordMissingComma(String),
    SutronLogTooShort,
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error::Io(err)
    }
}

impl From<chrono::ParseError> for Error {
    fn from(err: chrono::ParseError) -> Error {
        Error::ChronoParse(err)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
