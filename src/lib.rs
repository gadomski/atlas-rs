//! ATLAS is a remote monitoring system at the Helheim Glacier in southeast Greenland.

#![deny(missing_docs, missing_debug_implementations, missing_copy_implementations,
        trivial_casts, trivial_numeric_casts, unsafe_code, unstable_features,
        unused_import_braces, unused_qualifications)]

extern crate chrono;
extern crate handlebars_iron;
#[macro_use]
extern crate iron;
#[macro_use]
extern crate log;
extern crate notify;
extern crate rustc_serialize;
extern crate sbd;
extern crate url;
#[cfg(feature = "magick_rust")]
extern crate magick_rust;

pub mod cam;
pub mod error;
pub mod heartbeat;
pub mod server;
pub mod sutron;
pub mod watch;
#[cfg(feature = "magick_rust")]
pub mod magick;

pub use error::Error;

/// Our custom result type.
pub type Result<T> = std::result::Result<T, Error>;
