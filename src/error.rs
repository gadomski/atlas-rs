//! Package-wide error handling.
use std::error;
use std::fmt;
use std::io;
use std::path::PathBuf;

use chrono;
use notify;
use regex;
use sbd;
use toml;
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
    /// A camera can't handle the given path.
    InvalidCameraPath(String, PathBuf),
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
    /// Wrapper around `regex::Error`.
    Regex(regex::Error),
    /// Wrapper around `sbd::Error`.
    Sbd(sbd::Error),
    /// Wrapper around `atlas::sutron::Error`.
    Sutron(sutron::Error),
    /// There was one or more errors when parsing some toml.
    TomlParse(Vec<toml::ParserError>),
    /// Wrapper around `toml::DecodeError`.
    TomlDecode(toml::DecodeError),
    /// Wrapper around `url::ParseError`.
    UrlParse(url::ParseError),
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::ChronoParse(ref err) => err.description(),
            Error::InvalidCameraPath(_, _) => "invalid camera path",
            Error::Io(ref err) => err.description(),
            #[cfg(feature = "magick_rust")]
            Error::Magick(_) => "imagemagick error",
            Error::ParseHeartbeat(ref err) => err.description(),
            Error::Regex(ref err) => err.description(),
            Error::Notify(ref err) => err.description(),
            Error::Sbd(ref err) => err.description(),
            Error::Sutron(ref err) => err.description(),
            Error::TomlDecode(ref err) => err.description(),
            Error::TomlParse(_) => "toml parse error(s)",
            Error::UrlParse(ref err) => err.description(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::ChronoParse(ref err) => write!(f, "chrono error: {}", err),
            Error::InvalidCameraPath(ref s, ref p) => {
                write!(f, "camera {} can't handle path: {}", s, p.to_string_lossy())
            }
            Error::Io(ref err) => write!(f, "io error: {}", err),
            #[cfg(feature = "magick_rust")]
            Error::Magick(ref s) => write!(f, "imagemagick error: {}", s),
            Error::ParseHeartbeat(ref err) => write!(f, "heartbeat parsing error: {}", err),
            Error::Notify(ref err) => write!(f, "notify error: {}", err),
            Error::Sbd(ref err) => write!(f, "sbd error: {}", err),
            Error::Regex(ref err) => write!(f, "regex error: {}", err),
            Error::Sutron(ref err) => write!(f, "sutron error: {}", err),
            Error::TomlDecode(ref err) => write!(f, "toml decode error: {}", err),
            Error::TomlParse(ref errors) => {
                write!(f,
                       "toml parse error(s): {}",
                       errors.iter()
                           .map(|e| format!("[{},{}] {}", e.lo, e.hi, e.desc))
                           .collect::<Vec<_>>()
                           .join("; "))
            }
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

impl From<regex::Error> for Error {
    fn from(err: regex::Error) -> Error {
        Error::Regex(err)
    }
}

impl From<sbd::Error> for Error {
    fn from(err: sbd::Error) -> Error {
        Error::Sbd(err)
    }
}

impl From<toml::DecodeError> for Error {
    fn from(err: toml::DecodeError) -> Error {
        Error::TomlDecode(err)
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
