//! Tools for watching things to do things.
//!
//! E.g. watch a directory to trigger a re-read of the heartbeat messages.

use std::sync::{Arc, RwLock};
use std::sync::mpsc::channel;

use notify::{self, RecommendedWatcher, Watcher};

use sbd::storage::FilesystemStorage;

use heartbeat::{Heartbeat, IntoHeartbeats};

/// Watches a directory and refreshes a vector of heartbeats in a thread-safe way.
///
/// Use this watcher to get a `Arc<RwLock<Vec<Heartbeat>>>>` that you can trust will be up-to-date.
#[derive(Debug)]
pub struct HeartbeatWatcher {
    directory: String,
    imei: String,
    heartbeats: Arc<RwLock<Vec<Heartbeat>>>,
}

impl HeartbeatWatcher {
    /// Creates a new watcher for a given directory and Iridium IMEI number.
    pub fn new(directory: &str, imei: &str) -> HeartbeatWatcher {
        let heartbeats = Vec::new();
        let mut watcher = HeartbeatWatcher {
            directory: directory.to_string(),
            imei: imei.to_string(),
            heartbeats: Arc::new(RwLock::new(heartbeats)),
        };
        watcher.fill();
        watcher
    }

    /// Gets a new clone of the `Arc<RwLock<>>` around the heartbeats vector.
    pub fn heartbeats(&self) -> Arc<RwLock<Vec<Heartbeat>>> {
        self.heartbeats.clone()
    }

    /// Enter the infinite watching loop.
    pub fn watch(&mut self) {
        let (tx, rx) = channel();
        let mut watcher: RecommendedWatcher = Watcher::new(tx).unwrap();
        watcher.watch(&self.directory).unwrap();
        loop {
            match rx.recv() {
                Ok(notify::Event { path: Some(_), op: Ok(_) }) => {
                    self.fill();
                }
                // FIXME do better
                Err(e) => println!("Error yo! {}", e),
                _ => (),
            }
            while let Ok(_) = rx.try_recv() {
                // pass, clear out the buffer
            }
        }
    }

    fn fill(&mut self) {
        let storage = FilesystemStorage::open(&self.directory).unwrap();
        let mut messages: Vec<_> = storage.iter().map(|r| r.unwrap()).collect();
        messages.retain(|m| m.imei() == self.imei);
        messages.sort();
        let mut heartbeats = self.heartbeats.write().unwrap();
        heartbeats.clear();
        heartbeats.extend(messages.into_heartbeats()
            .unwrap()
            .into_iter()
            .filter_map(|h| h.ok()))
    }
}