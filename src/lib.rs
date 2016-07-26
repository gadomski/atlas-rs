//! ATLAS is a remote monitoring system at the Helheim Glacier in southeast Greenland.

#![deny(missing_docs, missing_debug_implementations, missing_copy_implementations,
        trivial_casts, trivial_numeric_casts, unsafe_code, unstable_features,
        unused_import_braces, unused_qualifications)]

extern crate chrono;
#[macro_use]
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
    /// Wrapper around `atlas::sutron::Error`.
    Sutron(sutron::Error),
    /// Wrapper around `url::ParseError`.
    UrlParse(url::ParseError),
}

impl std::error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::ChronoParse(ref err) => err.description(),
            Error::Io(ref err) => err.description(),
            Error::ParseHeartbeat(ref err) => err.description(),
            Error::Notify(ref err) => err.description(),
            Error::Sbd(ref err) => err.description(),
            Error::Sutron(ref err) => err.description(),
            Error::UrlParse(ref err) => err.description(),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            Error::ChronoParse(ref err) => write!(f, "chrono error: {}", err),
            Error::Io(ref err) => write!(f, "io error: {}", err),
            Error::ParseHeartbeat(ref err) => write!(f, "heartbeat parsing error: {}", err),
            Error::Notify(ref err) => write!(f, "notify error: {}", err),
            Error::Sbd(ref err) => write!(f, "sbd error: {}", err),
            Error::Sutron(ref err) => write!(f, "sutron error: {}", err),
            Error::UrlParse(ref err) => write!(f, "url parsing error: {}", err),
        }
    }
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

impl From<url::ParseError> for Error {
    fn from(err: url::ParseError) -> Error {
        Error::UrlParse(err)
    }
}

impl From<heartbeat::ParseHeartbeatError> for Error {
    fn from(err: heartbeat::ParseHeartbeatError) -> Error {
        Error::ParseHeartbeat(err)
    }
}

/// Our custom result type.
pub type Result<T> = std::result::Result<T, Error>;
