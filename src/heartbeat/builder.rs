use sbd::mo::Message;
use std::result;
use std::vec;

use chrono::{TimeZone, UTC};
use regex::Regex;

use {Error, Result};
use heartbeat::{EfoyAction, Heartbeat, Scan, ScanDetail, ScannerOn, SkipReason, SkippedScan};
use units::{Celsius, Degree, Kilobyte, Meter, Millibar, OrionPercentage, Percentage, Volt};

const V1_NUM_FIELDS: usize = 49;
// Yup, this is a super-crappy header. I didn't think about headers when we installed the system
// in 2015. Do'h.
const V1_HEADER: &'static str = "0,";
const V2_HEADER: &'static str = r"^(1,(?P<id>\d+),\d+,(?P<bytes>\d+):)|(0)ATHB02\d\d\d\r";
const V2_SECONDARY_HEADER: &'static str = r"^1,(?P<id>\d+),\d+:";

/// Creates heartbeat builders by extracting messages from a vector.
///
/// Any messages not extracted are left in the original `messages` vector. The messages are
/// processed in order, so make sure the vector is sorted the way you'd like before passing it in.
///
/// # Examples
///
/// ```
/// use atlas::heartbeat::{self, Message};
/// let mut messages = vec![Message::from_path("data/150731_230159.sbd").unwrap()];
/// let builders = heartbeat::extract_builders(&mut messages).unwrap();
/// assert_eq!(1, builders.len());
/// assert!(messages.is_empty());
/// ```
pub fn extract_builders(messages: &mut Vec<Message>) -> Result<Vec<Box<Builder>>> {
    let mut builders: Vec<Box<Builder>> = Vec::new();
    let mut leftovers = Vec::new();
    for message in messages.drain(..) {
        match create_builder(message) {
            Ok(builder) => {
                if builders.last().map_or(false, |b| !b.full()) {
                    leftovers.extend(builders.pop().unwrap().into_iter());
                }
                builders.push(builder);
            }
            Err(Error::RejectedMessage(message)) => {
                if let Some(mut builder) = builders.pop() {
                    match builder.push(message) {
                        Ok(()) => builders.push(builder),
                        Err(Error::RejectedMessage(message)) => {
                            if builder.full() {
                                builders.push(builder);
                            } else {
                                leftovers.extend(builder.into_iter());
                            }
                            leftovers.push(message);
                        }
                        Err(err) => return Err(err),
                    }
                } else {
                    leftovers.push(message);
                }
            }
            Err(err) => return Err(err),
        }
    }
    if builders.last().map_or(false, |b| !b.full()) {
        leftovers.extend(builders.pop().unwrap().into_iter());
    }
    messages.extend(leftovers.into_iter());
    Ok(builders)
}

/// Creates a new boxed builder and consumes the message.
///
/// Returns a `Error::RejectedMessage` with the provided message if the builder construction
/// fails due to an improper message. This allows the message to be re-used if necessary.
///
/// # Examples
///
/// ```
/// # use atlas::Error;
/// # use atlas::heartbeat::{self, Message};
/// let valid_starting_message = Message::from_path("data/150731_230159.sbd").unwrap();
/// let builder = heartbeat::create_builder(valid_starting_message).unwrap();
///
/// let invalid_starting_message = Message::from_path("data/160812_230111.sbd").unwrap();
/// match heartbeat::create_builder(invalid_starting_message.clone()) {
///     Err(Error::RejectedMessage(message)) => assert_eq!(invalid_starting_message, message),
///     _ => panic!("Should have rejected secondary message"),
/// };
/// ```
pub fn create_builder(message: Message) -> Result<Box<Builder>> {
    BuilderV2::new(message)
        .map(|b| {
            let b: Box<Builder> = Box::new(b);
            b
        })
        .or_else(|err| {
            match err {
                Error::RejectedMessage(message) => {
                    BuilderV1::new(message).map(|b| {
                        let b: Box<Builder> = Box::new(b);
                        b
                    })
                }
                _ => Err(err),
            }
        })
}


/// Heartbeat construction. Turns sbd messages into heartbeats.
///
/// Use `create_builder` to make boxed builders, which hide the underlying version-specific
/// implementations.
pub trait Builder {
    /// Consumes this builder and returns an iterator over its messages.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::heartbeat::{self, Message};
    /// let message = Message::from_path("data/150731_230159.sbd").unwrap();
    /// let builder = heartbeat::create_builder(message.clone()).unwrap();
    /// let messages = builder.into_iter().collect::<Vec<_>>();
    /// assert_eq!(vec![message], messages);
    /// ```
    fn into_iter(self: Box<Self>) -> vec::IntoIter<Message> {
        self.into_messages().into_iter()
    }

    /// Consumes this builder and returns its underlying message vector.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::heartbeat::{self, Message};
    /// let message = Message::from_path("data/150731_230159.sbd").unwrap();
    /// let builder = heartbeat::create_builder(message.clone()).unwrap();
    /// let messages = builder.into_messages();
    /// assert_eq!(vec![message], messages);
    /// ```
    fn into_messages(self: Box<Self>) -> Vec<Message>;

    /// Pushes a new message into the builder. If the operation fails, return the message.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::Error;
    /// # use atlas::heartbeat::{self, Message};
    /// let message1 = Message::from_path("data/160812_230048.sbd").unwrap();
    /// let message2 = Message::from_path("data/160812_230111.sbd").unwrap();
    ///
    /// let mut builder = heartbeat::create_builder(message1.clone()).unwrap();
    /// builder.push(message2).unwrap();
    ///
    /// let mut builder = heartbeat::create_builder(message1.clone()).unwrap();
    /// match builder.push(message1.clone()) {
    ///     Err(Error::RejectedMessage(message)) => assert_eq!(message1, message),
    ///     _ => panic!("Should have rejected message"),
    /// }
    /// ```
    fn push(&mut self, message: Message) -> Result<()>;

    /// Returns true if this builder is full.
    ///
    /// A full builder is one that *could* be turned into a heartbeat. For version 1 messages, this
    /// doesn't necessarily mean that the builder can't accept more messages — the last field of
    /// the heartbeat could have been cut halfway, meaning that we have the correct number of
    /// fields but the last number was truncated by the message split. PITA, I know. All subsequent
    /// versions of heartbeats don't have this problem.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::heartbeat::{self, Message};
    /// let message1 = Message::from_path("data/160812_230048.sbd").unwrap();
    /// let message2 = Message::from_path("data/160812_230111.sbd").unwrap();
    ///
    /// let mut builder = heartbeat::create_builder(message1.clone()).unwrap();
    /// assert!(!builder.full());
    /// builder.push(message2).unwrap();
    /// assert!(builder.full());
    /// ```
    fn full(&self) -> bool;

    /// Creates a heartbeat.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::heartbeat::{self, Message};
    /// let message = Message::from_path("data/150731_230159.sbd").unwrap();
    /// let builder = heartbeat::create_builder(message.clone()).unwrap();
    /// let heartbeat = builder.to_heartbeat().unwrap();
    /// ```
    fn to_heartbeat(&self) -> Result<Heartbeat>;
}

#[derive(Debug)]
struct BuilderV1 {
    messages: Vec<Message>,
}

impl BuilderV1 {
    fn new(message: Message) -> Result<BuilderV1> {
        if try!(message.payload_str()).starts_with(V1_HEADER) {
            Ok(BuilderV1 { messages: vec![message] })
        } else {
            Err(Error::RejectedMessage(message))
        }
    }

    fn field_count(&self) -> usize {
        self.payload().split(',').count()
    }

    fn payload(&self) -> String {
        self.messages.iter().fold(String::new(), |mut s, m| {
            s.push_str(m.payload_str().unwrap());
            s
        })
    }
}

impl Builder for BuilderV1 {
    fn into_messages(self: Box<Self>) -> Vec<Message> {
        self.messages
    }

    fn push(&mut self, message: Message) -> Result<()> {
        let _ = try!(message.payload_str());
        self.messages.push(message);
        if self.field_count() <= V1_NUM_FIELDS {
            Ok(())
        } else {
            Err(Error::RejectedMessage(self.messages.pop().unwrap()))
        }
    }

    fn full(&self) -> bool {
        self.field_count() == V1_NUM_FIELDS
    }

    fn to_heartbeat(&self) -> Result<Heartbeat> {
        let payload = self.payload();
        let fields = payload.split(',').collect::<Vec<_>>();
        let last_scan_start_month = try!(fields[11][0..2].parse::<u64>()) + 1;
        let last_scan_start =
            try!(UTC.datetime_from_str(&format!("{:02}{}", last_scan_start_month, &fields[11][2..]),
                                   "%m/%d/%y %H:%M:%S"));
        Ok(Heartbeat {
            start_time: self.messages[0].time_of_session(),
            external_temperature: Celsius(try!(fields[1].parse())),
            mount_temperature: Celsius(try!(fields[26].parse())),
            pressure: Millibar(try!(fields[2].parse())),
            humidity: Percentage(try!(fields[3].parse())),
            soc1: OrionPercentage(try!(fields[37].parse())),
            soc2: OrionPercentage(try!(fields[40].parse())),
            last_scan: Scan {
                start: last_scan_start,
                end: None,
                detail: None,
            },
            last_scan_on: None,
            last_scan_skip: None,
            last_efoy1_action: None,
            last_efoy2_action: None,
        })
    }
}

#[derive(Debug)]
struct BuilderV2 {
    header: Option<Header>,
    messages: Vec<Message>,
}

#[derive(Debug, Clone, Copy)]
struct Header {
    bytes: usize,
    id: u64,
}

impl BuilderV2 {
    fn new(message: Message) -> Result<BuilderV2> {
        match Self::extract_header(try!(message.payload_str())) {
            Ok(header) => {
                Ok(BuilderV2 {
                    header: header,
                    messages: vec![message],
                })
            }
            Err(()) => Err(Error::RejectedMessage(message)),
        }
    }

    fn extract_header(payload: &str) -> result::Result<Option<Header>, ()> {
        lazy_static! {
            static ref RE: Regex = Regex::new(V2_HEADER).unwrap();
        }
        if let Some(captures) = RE.captures(payload) {
            Ok(captures.name("id")
                .and_then(|id| {
                    captures.name("bytes").map(|bytes| {
                        Header {
                            id: id.parse().unwrap(),
                            bytes: bytes.parse().unwrap(),
                        }
                    })
                }))
        } else {
            Err(())
        }
    }

    fn extract_secondary_header(payload: &str) -> Option<u64> {
        lazy_static! {
            static ref RE: Regex = Regex::new(V2_SECONDARY_HEADER).unwrap();
        }
        RE.captures(payload).and_then(|c| c.name("id").map(|id| id.parse().unwrap()))
    }

    fn bytes(&self) -> usize {
        self.body().len()
    }

    fn body(&self) -> String {
        self.messages.iter().fold(String::new(), |mut s, m| {
            let payload = m.payload_str().unwrap();
            if self.header.is_some() {
                let idx = payload.find(':').unwrap() + 1;
                s.push_str(&payload[idx..]);
            } else {
                // There's just a zero at the start of the message.
                s.push_str(&payload[1..]);
            }
            s
        })
    }
}

impl Builder for BuilderV2 {
    fn into_messages(self: Box<Self>) -> Vec<Message> {
        self.messages
    }

    fn push(&mut self, message: Message) -> Result<()> {
        if self.full() {
            return Err(Error::RejectedMessage(message));
        }
        match Self::extract_secondary_header(try!(message.payload_str())) {
            Some(id) => {
                // We can trust the header exists b/c all non-full builders have a header.
                if self.header.unwrap().id != id {
                    Err(Error::RejectedMessage(message))
                } else {
                    self.messages.push(message);
                    Ok(())
                }
            }
            None => Err(Error::RejectedMessage(message)),
        }
    }

    fn full(&self) -> bool {
        self.header.map_or(true, |h| self.bytes() == h.bytes)
    }

    fn to_heartbeat(&self) -> Result<Heartbeat> {
        let datetime_fmt = "%m/%d/%y %H:%M:%S";
        let body = self.body();
        let mut lines = body.lines().skip(1);
        let mut next_row = || lines.next().unwrap().split(',').collect::<Vec<_>>();

        let row = next_row();
        let scan_on = ScannerOn {
            datetime: try!(UTC.datetime_from_str(row[0], datetime_fmt)),
            scanner_voltage: Volt(try!(row[1].parse())),
            scanner_temperature: Celsius(try!(row[2].parse())),
            memory_external: Kilobyte(try!(row[3].parse())),
            memory_internal: Kilobyte(try!(row[4].parse())),
        };

        let row = next_row();
        let external_temperature = Celsius(try!(row[0].parse()));
        let pressure = Millibar(try!(row[1].parse()));
        let humidity = Percentage(try!(row[2].parse()));

        let row = next_row();
        let scan_start = try!(UTC.datetime_from_str(row[0], datetime_fmt));

        let row = next_row();
        let detail = ScanDetail {
            num_points: try!(row[1].parse()),
            minimum_range: Meter(try!(row[2].parse())),
            maximum_range: Meter(try!(row[3].parse())),
            file_size: Kilobyte(try!(row[4].parse())),
            minimum_amplitude: try!(row[5].parse()),
            maximum_amplitude: try!(row[6].parse()),
            roll: Degree(try!(row[7].parse())),
            pitch: Degree(try!(row[8].parse())),
            latitude: Degree(try!(row[9].parse())),
            longitude: Degree(try!(row[10].parse())),
        };
        let scan = Scan {
            start: scan_start,
            end: Some(try!(UTC.datetime_from_str(row[0], datetime_fmt))),
            detail: Some(detail),
        };

        let row = next_row();
        let scan_skip = SkippedScan {
            datetime: try!(UTC.datetime_from_str(row[0], datetime_fmt)),
            reason: try!(SkipReason::new(row[1], row[2])),
        };

        let row = next_row();
        let efoy1 = try!(EfoyAction::new(row[0], row[1]));
        let _ = next_row();

        let row = next_row();
        let efoy2 = try!(EfoyAction::new(row[0], row[1]));
        let _ = next_row();

        let row = next_row();
        Ok(Heartbeat {
            start_time: self.messages[0].time_of_session(),
            external_temperature: external_temperature,
            mount_temperature: Celsius(try!(row[0].parse())),
            pressure: pressure,
            humidity: humidity,
            soc1: OrionPercentage(try!(row[1].parse())),
            soc2: OrionPercentage(try!(row[2].parse())),
            last_scan_on: Some(scan_on),
            last_scan: scan,
            last_scan_skip: Some(scan_skip),
            last_efoy1_action: Some(efoy1),
            last_efoy2_action: Some(efoy2),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use sbd::mo::Message;

    use Error;
    use heartbeat::tests::{one_v1_message, one_v2_message, two_v1_messages, two_v2_messages};

    #[test]
    fn extract_builders_empty_vector() {
        let mut messages = Vec::new();
        let builders = extract_builders(&mut messages).unwrap();
        assert!(builders.is_empty());
        assert!(messages.is_empty());
    }

    #[test]
    fn extract_builders_one_v1() {
        let mut messages = vec![one_v1_message()];
        let builders = extract_builders(&mut messages).unwrap();
        assert_eq!(1, builders.len());
        assert!(messages.is_empty());
    }

    #[test]
    fn extract_builders_two_v1_one_heartbeat() {
        let mut messages = two_v1_messages();
        let builders = extract_builders(&mut messages).unwrap();
        assert_eq!(1, builders.len());
        assert!(messages.is_empty());
    }

    #[test]
    fn extract_builders_two_v1_two_heartbeats() {
        let mut messages = vec![one_v1_message(), one_v1_message()];
        let builders = extract_builders(&mut messages).unwrap();
        assert_eq!(2, builders.len());
        assert!(messages.is_empty());
    }

    #[test]
    fn extract_builders_two_v1_one_heartbeat_one_leftover() {
        let leftover = two_v1_messages()[1].clone();
        let mut messages = vec![one_v1_message(), leftover.clone()];
        let builders = extract_builders(&mut messages).unwrap();
        assert_eq!(1, builders.len());
        assert_eq!(vec![leftover], messages);
    }

    #[test]
    fn extract_builders_junk_message() {
        let leftover = two_v1_messages()[1].clone();
        let mut messages = vec![leftover.clone()];
        let builders = extract_builders(&mut messages).unwrap();
        assert!(builders.is_empty());
        assert_eq!(vec![leftover], messages);
    }

    #[test]
    fn extract_builders_incomplete_message() {
        let leftover = two_v1_messages()[0].clone();
        let mut messages = vec![leftover.clone()];
        let builders = extract_builders(&mut messages).unwrap();
        assert!(builders.is_empty());
        assert_eq!(vec![leftover], messages);
    }

    #[test]
    fn extract_builders_incomplete_message_then_complete_message() {
        let leftover = two_v1_messages()[0].clone();
        let mut messages = two_v1_messages();
        messages.insert(0, leftover.clone());
        let builders = extract_builders(&mut messages).unwrap();
        assert_eq!(1, builders.len());
        assert_eq!(vec![leftover], messages);
    }

    #[test]
    fn extract_builders_one_v2() {
        let mut messages = vec![one_v2_message()];
        let builders = extract_builders(&mut messages).unwrap();
        assert_eq!(1, builders.len());
        assert!(messages.is_empty());
    }

    #[test]
    fn extract_builders_two_v2_one_heartbeat() {
        let mut messages = two_v2_messages();
        let builders = extract_builders(&mut messages).unwrap();
        assert_eq!(1, builders.len());
        assert!(messages.is_empty());
    }

    #[test]
    fn extract_builders_two_v2_second_message_different_number() {
        let mut messages = vec![two_v2_messages()[0].clone(),
                                Message::from_path("data/160812_220103.sbd").unwrap()];
        let messages_orig = messages.clone();
        let builders = extract_builders(&mut messages).unwrap();
        assert!(builders.is_empty());
        assert_eq!(messages_orig, messages);
    }

    #[test]
    fn extract_builders_two_v2_one_heartbeat_one_leftover() {
        let leftover = two_v2_messages()[1].clone();
        let mut messages = vec![one_v2_message(), leftover.clone()];
        let builders = extract_builders(&mut messages).unwrap();
        assert_eq!(1, builders.len());
        assert_eq!(vec![leftover], messages);
    }

    #[test]
    fn extract_builders_three_v2_one_heartbeat_one_leftover() {
        let leftover = Message::from_path("data/160812_220103.sbd").unwrap();
        let mut messages = two_v2_messages();
        messages.push(leftover.clone());
        let builders = extract_builders(&mut messages).unwrap();
        assert_eq!(1, builders.len());
        assert_eq!(vec![leftover], messages);
    }

    #[test]
    fn create_builder_v1_ok() {
        let _ = create_builder(one_v1_message()).unwrap();
    }

    #[test]
    fn create_builder_v1_not_ok() {
        let message = two_v1_messages()[1].clone();
        match create_builder(message.clone()) {
            Err(Error::RejectedMessage(reject)) => assert_eq!(message, reject),
            _ => panic!("Should have rejected message"),
        }
    }

    #[test]
    fn builder_push_ok_v1() {
        let messages = two_v1_messages();
        let mut builder = create_builder(messages[0].clone()).unwrap();
        assert!(builder.push(messages[1].clone()).is_ok());
    }

    #[test]
    fn builder_push_full_v1() {
        let messages = two_v1_messages();
        let mut builder = create_builder(messages[0].clone()).unwrap();
        builder.push(messages[1].clone()).unwrap();
        match builder.push(messages[1].clone()) {
            Err(Error::RejectedMessage(message)) => assert_eq!(messages[1], message),
            _ => panic!("Should have rejected message"),
        }
        assert!(builder.full());
    }

    #[test]
    fn builder_push_full_v2() {
        let messages = two_v2_messages();
        let mut builder = create_builder(messages[0].clone()).unwrap();
        builder.push(messages[1].clone()).unwrap();
        assert!(builder.full());
        match builder.push(messages[1].clone()) {
            Err(Error::RejectedMessage(message)) => assert_eq!(messages[1], message),
            _ => panic!("Should have rejected message"),
        }
        assert!(builder.full());
    }

}
