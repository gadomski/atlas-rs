//! Utitlies for working with Sutron files, such as log files.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::str::FromStr;

use chrono::{DateTime, TimeZone, UTC};

use {Error, Result};

/// A Sutron log file.
///
/// By default these files have the name `ssp.txt`, but other names can be used.
#[derive(Debug)]
pub struct Log {
    station_name: String,
    records: Vec<Record>,
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
        let mut records = Vec::new();
        for line in lines {
            records.push(try!(try!(line).parse()));
        }
        Ok(Log {
            station_name: station_name,
            records: records,
        })
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

    /// Returns the records in this log file.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::sutron::Log;
    /// let log = Log::from_path("data/ssp.txt").unwrap();
    /// assert_eq!(49, log.records().len());
    /// ```
    pub fn records(&self) -> &Vec<Record> {
        &self.records
    }
}

/// A Sutron log record.
///
/// We keep this simple as possible, with a datetime and some text data.
#[derive(Debug)]
pub struct Record {
    /// The date and time that the record was laid down.
    pub datetime: DateTime<UTC>,
    /// The data in the record, as a string.
    ///
    /// I suppose this could be binary data, but for now it's only strings.
    pub data: String,
}

impl FromStr for Record {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        if s.chars().count() < 20 {
            return Err(Error::SutronRecordTooShort(s.len()));
        }
        let comma = s.chars().skip(19).next().unwrap();
        if comma != ',' {
            return Err(Error::SutronRecordMissingComma(comma.to_string()));
        }
        let datetime = try!(UTC.datetime_from_str(&s[0..19], "%m/%d/%Y,%H:%M:%S"));
        let data = s[20..].to_string();
        Ok(Record {
            datetime: datetime,
            data: data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::str::FromStr;

    use chrono::{TimeZone, UTC};

    #[test]
    fn station_name() {
        let logfile = Log::from_path("data/ssp.txt").unwrap();
        assert_eq!("HEL_ATLAS", logfile.station_name());
    }

    #[test]
    fn records() {
        let logfile = Log::from_path("data/ssp.txt").unwrap();
        assert_eq!(49, logfile.records().len());
    }

    #[test]
    fn record_from_string() {
        let r = Record::from_str("06/11/2015,11:59:13,the data");
        assert!(r.is_ok());
        assert_eq!(UTC.ymd(2015, 6, 11).and_hms(11, 59, 13),
                   r.as_ref().unwrap().datetime);
        assert_eq!("the data", r.unwrap().data);
    }

    #[test]
    fn record_too_short() {
        let r = Record::from_str("too short");
        assert!(r.is_err());
    }

    #[test]
    fn not_a_comma() {
        let r = Record::from_str("06/11/2015,11:59:13~the data");
        assert!(r.is_err());
    }

    #[test]
    fn empty_record() {
        let r = Record::from_str("06/11/2015,11:59:13,");
        assert!(r.is_ok());
        assert_eq!("", r.unwrap().data);
    }
}
