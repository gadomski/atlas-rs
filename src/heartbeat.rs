//! Heartbeat messages are sent back from the ATLAS system via Iridium DirectIP.
//!
//! Each heartbeat contains all the information from the ATLAS system. There are multiple versions
//! (formats) of heartbeat messages, which we update as the system evolves and we want new data (or
//! realize some data is unused).
//!
//! Because of the limited payload size of Iridium SBD messages, heartbeats are often broken up
//! over multiple SBD messages. These can be non-trivial to reconstruct, especially since (in the
//! first version) we didn't have a per-heartbeat header on each message.
use std::error;
use std::fmt;
use std::num::{ParseFloatError, ParseIntError};
use std::result;
use std::str::FromStr;

use chrono;
use chrono::{DateTime, Duration, TimeZone, Timelike, UTC};

use sbd::mo::Message;

use {Error, Result};

const HEARTBEAT_FIELD_COUNT: usize = 49;

/// Newtype for Celsius degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Celsius(f32);

impl fmt::Display for Celsius {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:.1} °C", self.0)
    }
}

/// Newtype for millibar pressure measurements.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Millibar(f32);

impl fmt::Display for Millibar {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:.1} mBar", self.0)
    }
}

/// Newtype for a percentage.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Percentage(f32);

impl fmt::Display for Percentage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:.1} %", self.0)
    }
}

/// Riegl measurement programs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MeasurementProgram {
    /// Measurement program "0".
    FiftyKiloHertz,
    /// Measurement program "1".
    OneHundredKiloHertz,
    /// Measurement program "2".
    TwoHundredKiloHertz,
    /// Measurement program "3".
    ThreeHundredKiloHertz,
    /// Measurement program "4".
    Reflector,
}

impl FromStr for MeasurementProgram {
    type Err = ParseHeartbeatError;
    fn from_str(s: &str) -> result::Result<Self, Self::Err> {
        match s {
            "0" => Ok(MeasurementProgram::FiftyKiloHertz),
            "1" => Ok(MeasurementProgram::OneHundredKiloHertz),
            "2" => Ok(MeasurementProgram::TwoHundredKiloHertz),
            "3" => Ok(MeasurementProgram::ThreeHundredKiloHertz),
            "4" => Ok(MeasurementProgram::Reflector),
            _ => Err(ParseHeartbeatError::InvalidMeasurementProgram(s.to_string())),
        }
    }
}

/// Newtype for degrees (not radians).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Degrees(f32);

impl fmt::Display for Degrees {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:.1} °", self.0)
    }
}

/// Newtype for the Hass 50 current transducers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hass50Amps(f32);

/// Newtype for the Hass 100 current transducers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hass100Amps(f32);

/// Newtype for the Orion BMS percentages.
///
/// Orion BMS readings are voltages from zero to five that map onto a zero to one hundred percent
/// value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrionPercentage(f32);

impl OrionPercentage {
    /// Returns this percentage as a value from zero to one hundred.
    pub fn percentage(&self) -> f32 {
        100.0 * self.0 / 5.0
    }
}

impl fmt::Display for OrionPercentage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:.1} %", self.percentage())
    }
}

#[derive(Clone, Debug, PartialEq)]
/// The first version of the ATLAS heartbeats.
///
/// This version ran from system installation in July 2015 through the August 2016 revisit.
pub struct HeartbeatV1 {
    /// The SBD messages used to construct this heartbeat.
    pub messages: Vec<Message>,
    /// The external (outside) temperature, as measured by a temperature probe on the southern
    /// tower.
    pub temperature_external: Celsius,
    /// The atmospheric pressure.
    pub pressure: Millibar,
    /// The relative humidity.
    pub humidity: Percentage,
    /// The scanner's measurement program.
    ///
    /// This is basically the pulse rate of the scan.
    pub measurement_program: MeasurementProgram,
    /// The start phi angle (phi is the angle from vertical).
    pub phi_start: Degrees,
    /// The stop phi angle.
    pub phi_stop: Degrees,
    /// The increment of the phi angle for each pulse.
    ///
    /// This is related to pulse rate and mirror speed.
    pub phi_step: Degrees,
    /// The start theta angle (theta is the angle around the z axis).
    pub theta_start: Degrees,
    /// The stop theta angle.
    pub theta_stop: Degrees,
    /// The increment of the theta angle.
    ///
    /// This is controlled by the rotating scanner head.
    pub theta_step: Degrees,
    /// The date and time of the last scan start.
    pub scan_start_datetime: DateTime<UTC>,
    /// The temperature inside of the mount.
    pub temperature_mount: Celsius,
    /// The current into or out of the solar on tower 1.
    pub solar1: Hass50Amps,
    /// The current into or out of the wind generator on tower 1.
    pub wind1: Hass50Amps,
    /// The current into or out of the wind generator on tower 2.
    pub wind2: Hass50Amps,
    /// The current into or out of the solar on tower 2.
    pub solar2: Hass50Amps,
    /// The current into or out of EFOY 1.
    pub efoy1: Hass50Amps,
    /// The current into or out of EFOY 2.
    pub efoy2: Hass50Amps,
    /// The current into or out of battery 1.
    pub b1: Hass100Amps,
    /// The current into or out of battery 2.
    pub b2: Hass100Amps,
    /// The current into or out of battery 3.
    pub b3: Hass100Amps,
    /// The current into or out of battery 4.
    pub b4: Hass100Amps,
    /// The state of charge of battery 1.
    pub soc1: OrionPercentage,
    /// The charge current limit of battery 1.
    pub ccl1: OrionPercentage,
    /// The discharge current limit of battery 1.
    pub dcl1: OrionPercentage,
    /// The state of charge of battery 2.
    pub soc2: OrionPercentage,
    /// The charge current limit of battery 2.
    pub ccl2: OrionPercentage,
    /// The discharge current limit of battery 2.
    pub dcl2: OrionPercentage,
    /// The state of charge of battery 3.
    pub soc3: OrionPercentage,
    /// The charge current limit of battery 3.
    pub ccl3: OrionPercentage,
    /// The discharge current limit of battery 3.
    pub dcl3: OrionPercentage,
    /// The state of charge of battery 4.
    pub soc4: OrionPercentage,
    /// The charge current limit of battery 4.
    pub ccl4: OrionPercentage,
    /// The discharge current limit of battery 4.
    pub dcl4: OrionPercentage,
}


/// Trait for converting something into a vector of heartbeats.
pub trait IntoHeartbeats {
    /// Converts this into a vector of heartbeats.
    ///
    /// There is a double `Result` wrapper to allow the entire operation to fail, or for specific
    /// conversion components to fail.
    fn into_heartbeats(self) -> Result<Vec<Result<HeartbeatV1>>>;
}

impl IntoHeartbeats for Vec<Message> {
    fn into_heartbeats(self) -> Result<Vec<Result<HeartbeatV1>>> {
        let mut stack: Vec<(String, Vec<Message>)> = Vec::new();
        for message in self {
            let string = try!(message.payload_str()).to_string();
            if stack.is_empty() ||
               stack.last().unwrap().0.matches(',').count() + string.matches(',').count() >
               HEARTBEAT_FIELD_COUNT {
                if string.starts_with("0,") {
                    stack.push((string, vec![message]));
                } else {
                    // discard
                }
            } else {
                let mut last = stack.last_mut().unwrap();
                last.0.push_str(&string);
                last.1.push(message);
            }
        }
        Ok(stack.into_iter()
            .map(|(s, m)| HeartbeatV1::new(&s, m).map_err(|e| Error::from(e)))
            .collect())
    }
}

impl HeartbeatV1 {
    fn new(s: &str, messages: Vec<Message>) -> result::Result<HeartbeatV1, ParseHeartbeatError> {
        let d = s.split(',').collect::<Vec<_>>();
        if d.len() != HEARTBEAT_FIELD_COUNT {
            return Err(ParseHeartbeatError::FieldCount(d.len()));
        }
        let words = d[11].splitn(2, '/').collect::<Vec<_>>();
        if words.len() != 2 {
            return Err(ParseHeartbeatError::DatetimeFormat(d[11].to_string()));
        }
        let mut s = String::new();
        s.push_str(&format!("{:02}/", 1 + try!(words[0].parse::<u32>())));
        s.push_str(words[1]);
        let scan_start_datetime = try!(UTC.datetime_from_str(&s, "%m/%d/%y %H:%M:%S"));
        Ok(HeartbeatV1 {
            messages: messages,
            temperature_external: Celsius(try!(d[1].parse())),
            pressure: Millibar(try!(d[2].parse())),
            humidity: Percentage(try!(d[3].parse())),
            measurement_program: try!(d[4].parse()),
            phi_start: Degrees(try!(d[5].parse())),
            phi_stop: Degrees(try!(d[6].parse())),
            phi_step: Degrees(try!(d[7].parse())),
            theta_start: Degrees(try!(d[8].parse())),
            theta_stop: Degrees(try!(d[9].parse())),
            theta_step: Degrees(try!(d[10].parse())),
            scan_start_datetime: scan_start_datetime,
            temperature_mount: Celsius(try!(d[26].parse())),
            solar1: Hass50Amps(try!(d[27].parse())),
            wind1: Hass50Amps(try!(d[28].parse())),
            wind2: Hass50Amps(try!(d[29].parse())),
            solar2: Hass50Amps(try!(d[30].parse())),
            efoy1: Hass50Amps(try!(d[31].parse())),
            efoy2: Hass50Amps(try!(d[32].parse())),
            b1: Hass100Amps(try!(d[33].parse())),
            b2: Hass100Amps(try!(d[34].parse())),
            b3: Hass100Amps(try!(d[35].parse())),
            b4: Hass100Amps(try!(d[36].parse())),
            soc1: OrionPercentage(try!(d[37].parse())),
            ccl1: OrionPercentage(try!(d[38].parse())),
            dcl1: OrionPercentage(try!(d[39].parse())),
            soc2: OrionPercentage(try!(d[40].parse())),
            ccl2: OrionPercentage(try!(d[41].parse())),
            dcl2: OrionPercentage(try!(d[42].parse())),
            soc3: OrionPercentage(try!(d[43].parse())),
            ccl3: OrionPercentage(try!(d[44].parse())),
            dcl3: OrionPercentage(try!(d[45].parse())),
            soc4: OrionPercentage(try!(d[46].parse())),
            ccl4: OrionPercentage(try!(d[47].parse())),
            dcl4: OrionPercentage(try!(d[48].parse())),
        })
    }
}

#[derive(Debug)]
/// Error returned when trying to parse a heartbeat from a string.
pub enum ParseHeartbeatError {
    /// Wrapper around a `chrono::ParseError`.
    ChronoParse(chrono::ParseError),
    /// Incorrect datetime format caught on our side, not in chrono.
    DatetimeFormat(String),
    /// We can't get the measurement program out of the string.
    InvalidMeasurementProgram(String),
    /// The string had an incorrect number of fields.
    FieldCount(usize),
    /// Wrapper around `std::num::ParseFloatError`.
    ParseFloat(ParseFloatError),
    /// Wrapper around `std::num::ParseIntError`.
    ParseInt(ParseIntError),
}

impl error::Error for ParseHeartbeatError {
    fn description(&self) -> &str {
        match *self {
            ParseHeartbeatError::ChronoParse(ref err) => err.description(),
            ParseHeartbeatError::DatetimeFormat(_) => "the datetime format is incorrect",
            ParseHeartbeatError::InvalidMeasurementProgram(_) => "invalid measurement program",
            ParseHeartbeatError::FieldCount(_) => "incorrect number of fields",
            ParseHeartbeatError::ParseFloat(ref err) => err.description(),
            ParseHeartbeatError::ParseInt(ref err) => err.description(),
        }
    }
}

impl fmt::Display for ParseHeartbeatError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ParseHeartbeatError::ChronoParse(ref err) => write!(f, "chrono error: {}", err),
            ParseHeartbeatError::DatetimeFormat(ref s) => write!(f, "incorrect datetime: {}", s),
            ParseHeartbeatError::InvalidMeasurementProgram(ref s) => {
                write!(f, "invalid measurement program code: {}", s)
            }
            ParseHeartbeatError::FieldCount(n) => write!(f, "incorrect number of fields: {}", n),
            ParseHeartbeatError::ParseFloat(ref err) => write!(f, "parse float error: {}", err),
            ParseHeartbeatError::ParseInt(ref err) => write!(f, "parse int error: {}", err),
        }
    }
}

impl From<ParseFloatError> for ParseHeartbeatError {
    fn from(err: ParseFloatError) -> ParseHeartbeatError {
        ParseHeartbeatError::ParseFloat(err)
    }
}

impl From<ParseIntError> for ParseHeartbeatError {
    fn from(err: ParseIntError) -> ParseHeartbeatError {
        ParseHeartbeatError::ParseInt(err)
    }
}

impl From<chrono::ParseError> for ParseHeartbeatError {
    fn from(err: chrono::ParseError) -> ParseHeartbeatError {
        ParseHeartbeatError::ChronoParse(err)
    }
}

/// Calculates the expected start time of the next scan.
///
/// Right now we just operate on a 6-hour interval, so this calculates the next time we hit a
/// 6-hour interval.
pub fn expected_next_scan_time(datetime: &DateTime<UTC>) -> DateTime<UTC> {
    let hour = datetime.hour();
    let last_hour = hour - hour % 6;
    datetime.with_hour(last_hour)
        .and_then(|d| d.with_minute(0))
        .and_then(|d| d.with_second(0))
        .unwrap() + Duration::hours(6)
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::{TimeZone, UTC};

    use sbd::mo::Message;

    fn messages_from_paths(paths: &Vec<&str>) -> Vec<Message> {
        paths.iter().map(|p| Message::from_path(p).unwrap()).collect()
    }

    #[test]
    fn no_message_no_heartbeat() {
        let messages: Vec<Message> = vec![];
        let heartbeats = messages.into_heartbeats().unwrap();
        assert_eq!(0, heartbeats.len());
    }

    #[test]
    fn one_message_one_heartbeat() {
        let messages = messages_from_paths(&vec!["data/150729_020200.sbd"]);
        let mut heartbeats = messages.into_heartbeats().unwrap();
        assert_eq!(1, heartbeats.len());
        let heartbeat = heartbeats.pop().unwrap().unwrap();
        assert_eq!(UTC.ymd(2015, 7, 29).and_hms(2, 2, 0),
                   heartbeat.messages[0].time_of_session());
        assert_eq!(Celsius(6.181), heartbeat.temperature_external);
        assert_eq!(UTC.ymd(2015, 7, 29).and_hms(0, 2, 7),
                   heartbeat.scan_start_datetime);
        assert_eq!(OrionPercentage(-0.344048), heartbeat.dcl4);
    }

    #[test]
    fn two_messages_one_heartbeat() {
        let messages = messages_from_paths(&vec!["data/160714_000240.sbd",
                                                 "data/160714_000252.sbd"]);
        let mut heartbeats = messages.into_heartbeats().unwrap();
        assert_eq!(1, heartbeats.len());
        let heartbeat = heartbeats.pop().unwrap().unwrap();
        assert_eq!(Celsius(10.210), heartbeat.temperature_external);
        assert_eq!(OrionPercentage(-0.340767), heartbeat.dcl4);
    }

    #[test]
    fn two_messages_two_heartbeats() {
        let messages = messages_from_paths(&vec!["data/150729_020200.sbd",
                                                 "data/150729_020200.sbd"]);
        let mut heartbeats = messages.into_heartbeats().unwrap();
        assert_eq!(2, heartbeats.len());
        assert_eq!(heartbeats.pop().unwrap().unwrap(),
                   heartbeats.pop().unwrap().unwrap());
    }

    #[test]
    fn three_messages_one_heartbeat() {
        let messages = messages_from_paths(&vec!["data/160714_000252.sbd",
                                                 "data/160714_000240.sbd",
                                                 "data/160714_000252.sbd"]);
        let mut heartbeats = messages.into_heartbeats().unwrap();
        assert_eq!(1, heartbeats.len());
        let heartbeat = heartbeats.pop().unwrap().unwrap();
        assert_eq!(Celsius(10.210), heartbeat.temperature_external);
        assert_eq!(OrionPercentage(-0.340767), heartbeat.dcl4);
    }

    #[test]
    fn next_scan_in_an_hour() {
        assert_eq!(UTC.ymd(2016, 7, 22).and_hms(6, 0, 0),
                   expected_next_scan_time(&UTC.ymd(2016, 7, 22).and_hms(5, 0, 0)));
    }

    #[test]
    fn next_scan_tomorrow() {
        assert_eq!(UTC.ymd(2016, 7, 22).and_hms(0, 0, 0),
                   expected_next_scan_time(&UTC.ymd(2016, 7, 21).and_hms(23, 0, 0)));
    }
}
