//! Heartbeats are messages sent from the remote system via the Iridium network.
//!
//! These messages contain information about the system status, including last scan time,
//! temperature, and battery charge. Heartbeat messages may be broken up over several Iridium SBD
//! messages, and so need to be re-assembled using `Builder`s and `extract_builders`. There are
//! several versions of heartbeats to handle, but the details are hidden from the public interface.
//!
//! # Examples
//!
//! If you have many sbd messages and you want to create heartbeats, use `extract_heartbeats`,
//! which consumes the provided vector of messages:
//!
//! ```
//! use atlas::heartbeat::{self, Message};
//! let mut messages = vec![Message::from_path("data/150731_230159.sbd").unwrap()];
//! let heartbeats = heartbeat::extract_heartbeats(&mut messages).unwrap();
//! assert_eq!(1, heartbeats.len());
//! assert!(messages.is_empty());
//! ```
//!
//! If you have a `sbd::Storage`, you can use that to feed a heartbeat source. This source can be
//! configured to only use messages from certain Iridium IMEI numbers (specific modems):
//!
//! ```
//! # extern crate atlas;
//! # extern crate sbd;
//! # use sbd::storage::Storage;
//! # use sbd::mo::Message;
//! # use atlas::heartbeat::Source;
//! # fn main() {
//! let mut storage = sbd::storage::MemoryStorage::new();
//! storage.store(Message::from_path("data/150731_230159.sbd").unwrap()).unwrap();
//! storage.store(Message::from_path("data/160816_180158.sbd").unwrap()).unwrap();
//! let mut source = Source::new(storage);
//! let heartbeats = source.heartbeats().unwrap();
//! assert_eq!(2, heartbeats.len());
//! # }
//! ```
//!
//! `Storage::heartbeats` and `extract_heartbeats` will return an error if any of the heartbeat
//! extractions fail. For finer-grained error handling, you can use a `Builder`. The easiest way to
//! create builders is to use the convenience method `extract_builders`:
//!
//! ```
//! # use atlas::heartbeat::{self, Message};
//! let mut messages = vec!["data/160809_010502.sbd", "data/160809_010515.sbd"]
//!     .into_iter()
//!     .map(|s| Message::from_path(s).unwrap())
//!     .collect();
//! let mut builders = heartbeat::extract_builders(&mut messages).unwrap();
//! assert_eq!(1, builders.len());
//! ```
//!
//! Each builder creates a single heartbeat:
//!
//! ```
//! # use atlas::heartbeat::{self, Message};
//! # let mut messages = vec!["data/160809_010502.sbd", "data/160809_010515.sbd"]
//! #     .into_iter()
//! #     .map(|s| Message::from_path(s).unwrap())
//! #     .collect();
//! # let mut builders = heartbeat::extract_builders(&mut messages).unwrap();
//! let heartbeats = builders.into_iter().map(|b| b.to_heartbeat()).collect::<Vec<_>>();
//! assert_eq!(1, heartbeats.len());
//! assert!(heartbeats[0].is_ok());
//! ```

mod builder;
mod source;

pub use sbd::mo::Message;
pub use self::builder::{Builder, create_builder, extract_builders};
pub use self::source::Source;

use chrono::{DateTime, TimeZone, UTC};

use {Error, Result};
use units::{Celsius, Degree, Kilobyte, Meter, Millibar, OrionPercentage, Percentage, Volt};

/// Extracts heartbeats from a vector of messages.
///
/// Unused messages are left in the original message vector. Used messages are consumed. If there
/// is an error in creating any message, the entire function returns an error. If you need
/// finer-grained control, use `extract_builders` and `to_heartbeat` seperately.
///
/// # Examples
///
/// ```
/// use atlas::heartbeat::{self, Message};
/// let mut messages = vec![Message::from_path("data/150731_230159.sbd").unwrap()];
/// let heartbeats = heartbeat::extract_heartbeats(&mut messages).unwrap();
/// assert_eq!(1, heartbeats.len());
/// ```
pub fn extract_heartbeats(messages: &mut Vec<Message>) -> Result<Vec<Heartbeat>> {
    extract_builders(messages).and_then(|v| v.into_iter().map(|b| b.to_heartbeat()).collect())
}

/// Status report from the ATLAS system.
#[derive(Debug)]
pub struct Heartbeat {
    /// The time of the first constituent heartbeat message.
    pub start_time: DateTime<UTC>,
    /// The external (outside) temperature, as measured by a temperature probe on the southern
    /// tower.
    pub external_temperature: Celsius,
    /// The temperature inside of the mount.
    pub mount_temperature: Celsius,
    /// The atmospheric pressure.
    pub pressure: Millibar,
    /// The relative humidity.
    pub humidity: Percentage,
    /// The state of charge of battery 1.
    pub soc1: OrionPercentage,
    /// The state of charge of battery 2.
    pub soc2: OrionPercentage,
    /// The date and time of the last scanner power on.
    pub last_scan_on: Option<ScannerOn>,
    /// The date and time of the last scan start.
    pub last_scan: Scan,
    /// The date and time of the last scanner skip.
    pub last_scan_skip: Option<SkippedScan>,
    /// The last thing EFOY 1 did.
    pub last_efoy1_action: Option<EfoyAction>,
    /// The last thing EFOY 2 did.
    pub last_efoy2_action: Option<EfoyAction>,
}

impl Heartbeat {
    /// Creates a heartbeat from a single message.
    ///
    /// If the heartbeat is spread over more than one message, you need to use a `Builder`.
    ///
    /// # Examples
    ///
    /// ```
    /// use atlas::heartbeat::{Heartbeat, Message};
    /// let message = Message::from_path("data/150731_230159.sbd").unwrap();
    /// let heartbeat = Heartbeat::from_message(message).unwrap();
    /// ```
    pub fn from_message(message: Message) -> Result<Heartbeat> {
        create_builder(message).and_then(|builder| {
            if builder.full() {
                builder.to_heartbeat()
            } else {
                Err(Error::RejectedMessage(builder.into_messages().pop().unwrap()))
            }
        })
    }
}

/// A scanner power on, with information.
#[derive(Clone, Copy, Debug)]
pub struct ScannerOn {
    /// The time of scanner power on.
    pub datetime: DateTime<UTC>,
    /// The scanner temperature.
    pub scanner_temperature: Celsius,
    /// The voltage of the scanner at the time of power on.
    pub scanner_voltage: Volt,
    /// Available external memory (USB storage).
    pub memory_external: Kilobyte,
    /// Available internal scanner memory.
    pub memory_internal: Kilobyte,
}

/// A successful scan.
#[derive(Clone, Copy, Debug)]
pub struct Scan {
    /// The time of scan start.
    pub start: DateTime<UTC>,
    /// The time of scan end, not provided in version 1.
    pub end: Option<DateTime<UTC>>,
    /// Detailed information about the scan, not provided in version 1.
    pub detail: Option<ScanDetail>,
}

/// A slew of information about a scan, none of which came through in version 1.
#[derive(Clone, Copy, Debug)]
pub struct ScanDetail {
    /// The number of points scanned.
    pub num_points: u64,
    /// The minimum range of the scan.
    pub minimum_range: Meter,
    /// The maximum range of the scan.
    pub maximum_range: Meter,
    /// The file size.
    pub file_size: Kilobyte,
    /// The minimum amplitude.
    pub minimum_amplitude: u16,
    /// The maximum amplitude.
    pub maximum_amplitude: u16,
    /// The roll of the scanner (from inclination sensors).
    pub roll: Degree,
    /// The pitch of the scanner (from inclination sensors).
    pub pitch: Degree,
    /// The latitude of the scanner.
    pub latitude: Degree,
    /// The longitude of the scanner.
    pub longitude: Degree,
}

/// The information we get when the scanner skips a scan.
#[derive(Debug)]
pub struct SkippedScan {
    /// The time of the scanner skip.
    pub datetime: DateTime<UTC>,
    /// The reason the scan was skipped.
    pub reason: SkipReason,
}

/// We know why a scan skips via a returned reason code and some text.
#[derive(Debug, PartialEq)]
pub enum SkipReason {
    /// The scanner could not connect to the housing to report back information.
    CouldNotConnectToHousing,
    /// The housing scheduler is not enabled.
    SchedulerNotEnabled,
    /// There was a scanner error, more information in the string.
    ScannerError(String),
    /// The scanner tried to start too many times.
    TooManyRetries,
}

impl SkipReason {
    fn new(code: &str, description: &str) -> Result<SkipReason> {
        match code {
            "1" => Ok(SkipReason::CouldNotConnectToHousing),
            "2" => Ok(SkipReason::SchedulerNotEnabled),
            "3" => Ok(SkipReason::ScannerError(description.to_string())),
            "4" => Ok(SkipReason::TooManyRetries),
            _ => Err(Error::UnknownSkipReason(code.to_string(), description.to_string())),
        }
    }
}

/// The EFOY fuel cell systems do things and tell us about what they do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EfoyAction {
    /// The EFOY started charging.
    Start(DateTime<UTC>),
    /// The EFOY failed at charging.
    Failure(DateTime<UTC>),
    /// The EFOY succeeded at charging.
    Success(DateTime<UTC>),
}

impl EfoyAction {
    fn new(datetime: &str, word: &str) -> Result<EfoyAction> {
        let datetime = try!(UTC.datetime_from_str(&datetime[0..19], "%m/%d/%Y %H:%M:%S"));
        match word {
            "start" => Ok(EfoyAction::Start(datetime)),
            "fail" => Ok(EfoyAction::Failure(datetime)),
            "success" => Ok(EfoyAction::Success(datetime)),
            _ => Err(Error::UnknownEfoyAction(word.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, UTC};

    use super::*;
    use units::*;

    pub fn one_v1_message() -> Message {
        Message::from_path("data/150731_230159.sbd").unwrap()
    }

    pub fn one_v2_message() -> Message {
        Message::from_path("data/160816_180158.sbd").unwrap()
    }

    pub fn two_v1_messages() -> Vec<Message> {
        vec!["data/160809_010502.sbd", "data/160809_010515.sbd"]
            .into_iter()
            .map(|s| Message::from_path(s).unwrap())
            .collect()
    }

    pub fn two_v2_messages() -> Vec<Message> {
        vec!["data/160812_230048.sbd", "data/160812_230111.sbd"]
            .into_iter()
            .map(|s| Message::from_path(s).unwrap())
            .collect()
    }

    #[test]
    fn heartbeat_v1() {
        let heartbeat = Heartbeat::from_message(one_v1_message()).unwrap();
        assert_eq!(UTC.ymd(2015, 7, 31).and_hms(23, 1, 59),
                   heartbeat.start_time);
        assert_eq!(Celsius(11.095), heartbeat.external_temperature);
        assert_eq!(Celsius(16.1175), heartbeat.mount_temperature);
        assert_eq!(Millibar(962.690), heartbeat.pressure);
        assert_eq!(Percentage(36.487), heartbeat.humidity);
        assert_eq!(OrionPercentage(4.68509), heartbeat.soc1);
        assert_eq!(OrionPercentage(4.69742), heartbeat.soc2);
        assert_eq!(UTC.ymd(2015, 7, 31).and_hms(18, 2, 18),
                   heartbeat.last_scan.start);
    }

    #[test]
    fn heartbeat_from_message_too_short() {
        assert!(Heartbeat::from_message(two_v2_messages()[0].clone()).is_err());
    }

    #[test]
    fn heartbeat_v2() {
        let heartbeat = Heartbeat::from_message(one_v2_message()).unwrap();
        assert_eq!(UTC.ymd(2016, 8, 16).and_hms(18, 1, 58),
                   heartbeat.start_time);
        assert_eq!(Celsius(9.915), heartbeat.external_temperature);
        assert_eq!(Celsius(12.4), heartbeat.mount_temperature);
        assert_eq!(Millibar(942.240), heartbeat.pressure);
        assert_eq!(Percentage(40.932), heartbeat.humidity);
        assert_eq!(OrionPercentage(4.097), heartbeat.soc1);
        assert_eq!(OrionPercentage(4.132), heartbeat.soc2);
        let scan_on = heartbeat.last_scan_on.unwrap();
        assert_eq!(UTC.ymd(2016, 8, 16).and_hms(12, 01, 47), scan_on.datetime);
        assert_eq!(Celsius(11.8), scan_on.scanner_temperature);
        assert_eq!(Volt(23.4), scan_on.scanner_voltage);
        assert_eq!(Kilobyte(740991025.152), scan_on.memory_external);
        assert_eq!(Kilobyte(995349954.56), scan_on.memory_internal);
        let scan = heartbeat.last_scan;
        assert_eq!(UTC.ymd(2016, 8, 16).and_hms(12, 01, 58), scan.start);
        assert_eq!(UTC.ymd(2016, 8, 16).and_hms(12, 40, 24), scan.end.unwrap());
        let detail = scan.detail.unwrap();
        assert_eq!(20035104, detail.num_points);
        assert_eq!(Meter(-40.277), detail.minimum_range);
        assert_eq!(Meter(5164.539), detail.maximum_range);
        assert_eq!(Kilobyte(282005.084), detail.file_size);
        assert_eq!(0, detail.minimum_amplitude);
        assert_eq!(42, detail.maximum_amplitude);
        assert_eq!(Degree(-0.488), detail.roll);
        assert_eq!(Degree(-0.108), detail.pitch);
        assert_eq!(Degree(66.329918), detail.latitude);
        assert_eq!(Degree(-38.174053), detail.longitude);
        let scan_skip = heartbeat.last_scan_skip.unwrap();
        assert_eq!(UTC.ymd(2016, 8, 11).and_hms(18, 25, 35), scan_skip.datetime);
        assert_eq!(SkipReason::CouldNotConnectToHousing, scan_skip.reason);
        let efoy1 = heartbeat.last_efoy1_action.unwrap();
        assert_eq!(EfoyAction::Start(UTC.ymd(2016, 8, 11).and_hms(19, 00, 00)),
                   efoy1);
        let efoy2 = heartbeat.last_efoy2_action.unwrap();
        assert_eq!(EfoyAction::Start(UTC.ymd(2016, 8, 12).and_hms(11, 00, 00)),
                   efoy2);
    }

    #[test]
    fn extract_heartbeats_four() {
        let mut messages = vec![one_v1_message()];
        messages.extend(two_v1_messages().into_iter());
        messages.push(one_v2_message());
        messages.extend(two_v2_messages().into_iter());
        let heartbeats = extract_heartbeats(&mut messages).unwrap();
        assert_eq!(4, heartbeats.len());
    }
}
