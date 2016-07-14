use std::fs::File;
use std::io::{BufReader, BufRead};
use std::path::Path;

use {Error, Result};

/// A Sutron log file.
///
/// By default these files have the name `ssp.txt`, but other names can be used.
pub struct Log {
    station_name: String,
}

impl Log {
    /// Reads a log file from a path.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::sutron::Log;
    /// let log = Log::from_path("data/ssp.txt").unwrap();
    /// ```
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Log> {
        let mut lines = BufReader::new(try!(File::open(path))).lines();
        if let Some(r) = lines.next() {
            let s = try!(r);
            if s != "Station Name" {
                return Err(Error::BadSutronLogHeader(s));
            }
        } else {
            return Err(Error::SutronLogTooShort);
        }
        let station_name = if let Some(r) = lines.next() {
            try!(r)
        } else {
            return Err(Error::SutronLogTooShort);
        };
        Ok(Log { station_name: station_name })
    }

    /// Returns the station name.
    ///
    /// This is read from the log file.
    ///
    /// ```
    /// # use atlas::sutron::Log;
    /// let log = Log::from_path("data/ssp.txt").unwrap();
    /// assert_eq!("HEL_ATLAS", log.station_name());
    /// ```
    pub fn station_name(&self) -> &str {
        &self.station_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn station_name() {
        let logfile = Log::from_path("data/ssp.txt").unwrap();
        assert_eq!("HEL_ATLAS", logfile.station_name());
    }
}
