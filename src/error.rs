//! Package-wide error handling.
use std::error;
use std::fmt;
use std::io;

use chrono;
use notify;
use sbd;
use url;

use heartbeat;
use sutron;

#[derive(Debug)]
/// Our error enum.
pub enum Error {
    /// Wrapper around a `chrono::ParseError`.
    ChronoParse(chrono::ParseError),
    /// Wrapper around `std::io::Error`.
    Io(io::Error),
    #[cfg(feature = "magick_rust")]
    /// An imagemagick error.
    ///
    /// These errors are returned from `magick_rust` as `&str`, so we wrap those strings in this
    /// error type.
    Magick(String),
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

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::ChronoParse(ref err) => err.description(),
            Error::Io(ref err) => err.description(),
            #[cfg(feature = "magick_rust")]
            Error::Magick(_) => "imagemagick error",
            Error::ParseHeartbeat(ref err) => err.description(),
            Error::Notify(ref err) => err.description(),
            Error::Sbd(ref err) => err.description(),
            Error::Sutron(ref err) => err.description(),
            Error::UrlParse(ref err) => err.description(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::ChronoParse(ref err) => write!(f, "chrono error: {}", err),
            Error::Io(ref err) => write!(f, "io error: {}", err),
            #[cfg(feature = "magick_rust")]
            Error::Magick(ref s) => write!(f, "imagemagick error: {}", s),
            Error::ParseHeartbeat(ref err) => write!(f, "heartbeat parsing error: {}", err),
            Error::Notify(ref err) => write!(f, "notify error: {}", err),
            Error::Sbd(ref err) => write!(f, "sbd error: {}", err),
            Error::Sutron(ref err) => write!(f, "sutron error: {}", err),
            Error::UrlParse(ref err) => write!(f, "url parsing error: {}", err),
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
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
