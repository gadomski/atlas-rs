//! Home-side code for the remote glacier monitoring system in southeast Greenland.

#![deny(missing_docs,
        missing_debug_implementations, missing_copy_implementations,
        trivial_casts, trivial_numeric_casts,
        unsafe_code,
        unstable_features,
        unused_import_braces, unused_qualifications)]

extern crate chrono;
#[macro_use]
extern crate lazy_static;
extern crate notify;
extern crate regex;
extern crate sbd;
#[cfg(test)]
extern crate tempdir;

pub mod camera;
pub mod heartbeat;
pub mod units;

use std::num;
use std::path::PathBuf;

/// Crate-specific errors.
///
/// TODO implement `std::error::Error`.
#[derive(Debug)]
pub enum Error {
    /// Wrapper around `chrono::ParseError`.
    ChronoParse(chrono::ParseError),
    /// This path cannot be used with a camera.
    InvalidCameraPath(PathBuf),
    /// Wrapper around `notify::Error`.
    Notify(notify::Error),
    /// Wrapper around `std::num::ParseFloatError`.
    ParseFloat(num::ParseFloatError),
    /// Wrapper around `std::num::ParseIntError`.
    ParseInt(num::ParseIntError),
    /// Wrapper around `regex::Error`.
    Regex(regex::Error),
    /// This message couldn't be used, so here it is back.
    RejectedMessage(sbd::mo::Message),
    /// Wrapper around `sbd::Error`.
    Sbd(sbd::Error),
    /// The efoy action word wasn't a known value.
    UnknownEfoyAction(String),
    /// The skip reason code wasn't a known value.
    UnknownSkipReason(String, String),
}

impl From<num::ParseFloatError> for Error {
    fn from(err: num::ParseFloatError) -> Error {
        Error::ParseFloat(err)
    }
}

impl From<num::ParseIntError> for Error {
    fn from(err: num::ParseIntError) -> Error {
        Error::ParseInt(err)
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

/// Crate-specific result.
pub type Result<T> = std::result::Result<T, Error>;
