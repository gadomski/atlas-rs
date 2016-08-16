use std::collections::HashMap;

use sbd::storage::Storage;

use Result;
use heartbeat::{Heartbeat, extract_heartbeats};

/// Creates heartbeats from an Iridium storage.
#[derive(Debug)]
pub struct Source<S: Storage> {
    storage: S,
    whitelist: Vec<String>,
}

impl<S: Storage> Source<S> {
    /// Creates a new source from an Iridium storage.
    ///
    /// This source starts with an empty whitelist, so all IMEIs are allowed.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate atlas;
    /// # extern crate sbd;
    /// # use sbd::storage::MemoryStorage;
    /// # use atlas::heartbeat::Source;
    /// # fn main() {
    /// let source = Source::new(MemoryStorage::new());
    /// # }
    /// ```
    pub fn new(storage: S) -> Source<S> {
        Source {
            storage: storage,
            whitelist: Vec::new(),
        }
    }

    /// Returns the heartbeats in this storage, possibly filtered by IMEI numbers (see `whitelist`).
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate atlas;
    /// # extern crate sbd;
    /// # use sbd::mo::Message;
    /// # use sbd::storage::{Storage, MemoryStorage};
    /// # use atlas::heartbeat::Source;
    /// # fn main() {
    /// let message = Message::from_path("data/150731_230159.sbd").unwrap();
    /// let mut storage = MemoryStorage::new();
    /// storage.store(message).unwrap();
    /// let source = Source::new(storage);
    /// let heartbeats = source.heartbeats().unwrap();
    /// assert_eq!(1, heartbeats.len());
    /// # }
    pub fn heartbeats(&self) -> Result<Vec<Heartbeat>> {
        let mut messages = HashMap::new();
        if self.whitelist.is_empty() {
            for message in try!(self.storage.messages()) {
                messages.entry(message.imei().to_string()).or_insert_with(Vec::new).push(message);
            }
        } else {
            for imei in &self.whitelist {
                messages.insert(imei.to_string(),
                                try!(self.storage.messages_from_imei(imei)));
            }
        }
        let mut heartbeats = Vec::new();
        for mut messages in messages.values_mut() {
            messages.sort();
            heartbeats.extend(try!(extract_heartbeats(&mut messages)).into_iter());
        }
        heartbeats.sort_by_key(|h| h.start_time);
        Ok(heartbeats)
    }

    /// Adds an IMEI number to the whitelist.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate atlas;
    /// # extern crate sbd;
    /// # use sbd::storage::MemoryStorage;
    /// # use atlas::heartbeat::Source;
    /// # fn main() {
    /// let mut source = Source::new(MemoryStorage::new());
    /// source.whitelist("300234063909200");
    /// # }
    /// ```
    pub fn whitelist(&mut self, imei: &str) {
        self.whitelist.push(imei.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use sbd::storage::{MemoryStorage, Storage};

    use heartbeat::tests::{one_v1_message, two_v1_messages};

    #[test]
    fn empty_storage() {
        let storage = MemoryStorage::new();
        let source = Source::new(storage);
        let heartbeats = source.heartbeats().unwrap();
        assert!(heartbeats.is_empty());
    }

    #[test]
    fn one_message() {
        let mut storage = MemoryStorage::new();
        storage.store(one_v1_message()).unwrap();
        let source = Source::new(storage);
        let heartbeats = source.heartbeats().unwrap();
        assert_eq!(1, heartbeats.len());
    }

    #[test]
    fn filter() {
        let mut storage = MemoryStorage::new();
        storage.store(one_v1_message()).unwrap();
        let mut source = Source::new(storage);
        source.whitelist("300234063909201");
        let heartbeats = source.heartbeats().unwrap();
        assert!(heartbeats.is_empty());
    }

    #[test]
    fn sort_by_time() {
        let mut storage = MemoryStorage::new();
        let mut messages = two_v1_messages();
        storage.store(messages.pop().unwrap()).unwrap();
        storage.store(messages.pop().unwrap()).unwrap();
        let source = Source::new(storage);
        let heartbeats = source.heartbeats().unwrap();
        assert_eq!(1, heartbeats.len());
    }
}
