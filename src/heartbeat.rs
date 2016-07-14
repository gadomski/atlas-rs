//! Heartbeat messages are sent back from the ATLAS system via Iridium DirectIP.

use std::num::{ParseFloatError, ParseIntError};
use std::result;

use chrono;
use chrono::{DateTime, TimeZone, UTC};

use sbd::mo::Message;

use {Error, Result};

const HEARTBEAT_FIELD_COUNT: usize = 49;

#[derive(Debug, PartialEq)]
#[allow(missing_docs)]
pub struct Heartbeat {
    pub messages: Vec<Message>,
    pub temperature_external: f32,
    pub pressure: f32,
    pub humidity: f32,
    pub measurement_program: u8,
    pub phi_start: f32,
    pub phi_stop: f32,
    pub phi_step: f32,
    pub theta_start: f32,
    pub theta_stop: f32,
    pub theta_step: f32,
    pub scan_start_datetime: DateTime<UTC>,
    pub temperature_mount: f32,
    pub solar1: f32,
    pub wind1: f32,
    pub wind2: f32,
    pub solar2: f32,
    pub efoy1: f32,
    pub efoy2: f32,
    pub b1: f32,
    pub b2: f32,
    pub b3: f32,
    pub b4: f32,
    pub soc1: f32,
    pub ccl1: f32,
    pub dcl1: f32,
    pub soc2: f32,
    pub ccl2: f32,
    pub dcl2: f32,
    pub soc3: f32,
    pub ccl3: f32,
    pub dcl3: f32,
    pub soc4: f32,
    pub ccl4: f32,
    pub dcl4: f32,
}


/// Trait for converting something into a vector of heartbeats.
pub trait IntoHeartbeats {
    /// Converts this into a vector of heartbeats.
    ///
    /// There is a double `Result` wrapper to allow the entire operation to fail, or for specific
    /// conversion components to fail.
    fn into_heartbeats(self) -> Result<Vec<Result<Heartbeat>>>;
}

impl IntoHeartbeats for Vec<Message> {
    fn into_heartbeats(self) -> Result<Vec<Result<Heartbeat>>> {
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
            .map(|(s, m)| Heartbeat::new(&s, m).map_err(|e| Error::from(e)))
            .collect())
    }
}

impl Heartbeat {
    fn new(s: &str, messages: Vec<Message>) -> result::Result<Heartbeat, ParseHeartbeatError> {
        let d = s.split(',').collect::<Vec<_>>();
        if d.len() != HEARTBEAT_FIELD_COUNT {
            return Err(ParseHeartbeatError::FieldCount(d.len()));
        }
        Ok(Heartbeat {
            messages: messages,
            temperature_external: try!(d[1].parse()),
            pressure: try!(d[2].parse()),
            humidity: try!(d[3].parse()),
            measurement_program: try!(d[4].parse()),
            phi_start: try!(d[5].parse()),
            phi_stop: try!(d[6].parse()),
            phi_step: try!(d[7].parse()),
            theta_start: try!(d[8].parse()),
            theta_stop: try!(d[9].parse()),
            theta_step: try!(d[10].parse()),
            scan_start_datetime: try!(UTC.datetime_from_str(d[11], "%m/%d/%y %H:%M:%S")),
            temperature_mount: try!(d[26].parse()),
            solar1: try!(d[27].parse()),
            wind1: try!(d[28].parse()),
            wind2: try!(d[29].parse()),
            solar2: try!(d[30].parse()),
            efoy1: try!(d[31].parse()),
            efoy2: try!(d[32].parse()),
            b1: try!(d[33].parse()),
            b2: try!(d[34].parse()),
            b3: try!(d[35].parse()),
            b4: try!(d[36].parse()),
            soc1: try!(d[37].parse()),
            ccl1: try!(d[38].parse()),
            dcl1: try!(d[39].parse()),
            soc2: try!(d[40].parse()),
            ccl2: try!(d[41].parse()),
            dcl2: try!(d[42].parse()),
            soc3: try!(d[43].parse()),
            ccl3: try!(d[44].parse()),
            dcl3: try!(d[45].parse()),
            soc4: try!(d[46].parse()),
            ccl4: try!(d[47].parse()),
            dcl4: try!(d[48].parse()),
        })
    }
}

#[derive(Debug)]
/// Error returned when trying to parse a heartbeat from a string.
pub enum ParseHeartbeatError {
    /// Wrapper around a `chrono::ParseError`.
    ChronoParse(chrono::ParseError),
    /// The string had an incorrect number of fields.
    FieldCount(usize),
    /// Wrapper around `std::num::ParseFloatError`.
    ParseFloat(ParseFloatError),
    /// Wrapper around `std::num::ParseIntError`.
    ParseInt(ParseIntError),
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
        assert_eq!(6.181, heartbeat.temperature_external);
        assert_eq!(-0.344048, heartbeat.dcl4);
    }

    #[test]
    fn two_messages_one_heartbeat() {
        let messages = messages_from_paths(&vec!["data/160714_000240.sbd",
                                                 "data/160714_000252.sbd"]);
        let mut heartbeats = messages.into_heartbeats().unwrap();
        assert_eq!(1, heartbeats.len());
        let heartbeat = heartbeats.pop().unwrap().unwrap();
        assert_eq!(10.210, heartbeat.temperature_external);
        assert_eq!(-0.340767, heartbeat.dcl4);
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
        assert_eq!(10.210, heartbeat.temperature_external);
        assert_eq!(-0.340767, heartbeat.dcl4);
    }
}
