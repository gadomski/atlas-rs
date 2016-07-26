//! ATLAS is a remote monitoring system at the Helheim Glacier in southeast Greenland.

#![deny(missing_docs, missing_debug_implementations, missing_copy_implementations,
        trivial_casts, trivial_numeric_casts, unsafe_code, unstable_features,
        unused_import_braces, unused_qualifications)]

extern crate chrono;
extern crate iron;
extern crate handlebars_iron;
extern crate notify;
extern crate rustc_serialize;
extern crate sbd;
extern crate url;

pub mod heartbeat;
pub mod cam;
pub mod server;
pub mod sutron;
pub mod watch;

#[derive(Debug)]
/// Our error enum.
pub enum Error {
    /// The sutron log has a bad header, not what we'd expected.
    BadSutronLogHeader(String),
    /// Wrapper around a `chrono::ParseError`.
    ChronoParse(chrono::ParseError),
    /// Wrapper around `std::io::Error`.
    Io(std::io::Error),
    /// Wrapper around `atlas::heartbeat::ParseHeartbeatError`.
    ParseHeartbeat(heartbeat::ParseHeartbeatError),
    /// Wrapper around `notify::Error`.
    Notify(notify::Error),
    /// Wrapper around `sbd::Error`.
    Sbd(sbd::Error),
    /// The Sutron record was too short -- the length is provided.
    SutronRecordTooShort(usize),
    /// The Sutron record is missing a comma in the correct place (right after the timestamp).
    SutronRecordMissingComma(String),
    /// The Sutron log is too short.
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

impl From<notify::Error> for Error {
    fn from(err: notify::Error) -> Error {
        Error::Notify(err)
    }
}

impl From<sbd::Error> for Error {
    fn from(err: sbd::Error) -> Error {
        Error::Sbd(err)
    }
}

impl From<heartbeat::ParseHeartbeatError> for Error {
    fn from(err: heartbeat::ParseHeartbeatError) -> Error {
        Error::ParseHeartbeat(err)
    }
}

/// Our custom result type.
pub type Result<T> = std::result::Result<T, Error>;
