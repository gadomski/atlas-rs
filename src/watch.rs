//! Tools for watching things to do things.
//!
//! E.g. watch a directory to trigger a re-read of the heartbeat messages.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::sync::mpsc::channel;

use chrono::UTC;

use notify::{self, RecommendedWatcher, Watcher};

use sbd::storage::FilesystemStorage;

use Result;
use heartbeat::{HeartbeatV1, IntoHeartbeats};

/// A trait that can be used to watch a directory.
///
/// This restarts the watcher if we get a new directory, to pick up on new files.
pub trait DirectoryWatcher {
    /// Enter the infinite watching loop.
    fn watch(&mut self) -> Result<()> {
        let (tx, rx) = channel();
        let mut watcher: RecommendedWatcher = try!(Watcher::new(tx));
        try!(watcher.watch(&self.directory()));
        loop {
            match rx.recv() {
                Ok(notify::Event { path: Some(path), op: Ok(_) }) => {
                    match path.metadata() {
                        Ok(metadata) => {
                            if metadata.is_dir() {
                                try!(watcher.unwatch(&self.directory()));
                                try!(watcher.watch(&self.directory()));
                                info!("Watcher on {} restarted due to activity at {}",
                                      self.directory().to_string_lossy(),
                                      path.to_string_lossy());
                            }
                            match self.refresh() {
                                Ok(()) => info!("Refresh: {}", path.to_string_lossy()),
                                Err(err) => {
                                    error!("Error while refreshing in {}: {}",
                                           self.directory().to_string_lossy(),
                                           err)
                                }
                            }
                        }
                        Err(err) => {
                            match err.kind() {
                                io::ErrorKind::NotFound => {}
                                _ => {
                                    error!("Error while retrieving path metadata for {}: {}",
                                           path.to_string_lossy(),
                                           err)
                                }
                            }
                        }
                    }
                }
                Err(e) => error!("Error while receiving notify message: {}", e),
                _ => (),
            }
        }
    }

    /// Returns the path of the directory to be watched.
    fn directory(&self) -> &Path;

    /// Called whenever changes happen in the watched directory.
    fn refresh(&mut self) -> Result<()>;
}

/// Watches a directory and refreshes a vector of heartbeats in a thread-safe way.
///
/// Use this watcher to get a `Arc<RwLock<Vec<HeartbeatV1>>>>` that you can trust will be
/// up-to-date.
#[derive(Debug)]
pub struct HeartbeatWatcher {
    directory: PathBuf,
    imeis: Vec<String>,
    heartbeats: Arc<RwLock<Vec<HeartbeatV1>>>,
}

impl HeartbeatWatcher {
    /// Creates a new watcher for a given directory and Iridium IMEI numbers.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::sync::{Arc, RwLock};
    /// # use atlas::watch::HeartbeatWatcher;
    /// let heartbeats = Arc::new(RwLock::new(Vec::new()));
    /// let watcher = HeartbeatWatcher::new("data",
    ///                                     vec!["300234063909200".to_string()],
    ///                                     heartbeats);
    /// ```
    pub fn new<P: AsRef<Path>>(directory: P,
                               imeis: Vec<String>,
                               heartbeats: Arc<RwLock<Vec<HeartbeatV1>>>)
                               -> HeartbeatWatcher {
        HeartbeatWatcher {
            directory: directory.as_ref().to_path_buf(),
            imeis: imeis,
            heartbeats: heartbeats,
        }
    }
}

impl DirectoryWatcher for HeartbeatWatcher {
    fn directory(&self) -> &Path {
        self.directory.as_path()
    }

    fn refresh(&mut self) -> Result<()> {
        let storage = try!(FilesystemStorage::open(&self.directory));
        let mut messages: HashMap<String, Vec<_>> = HashMap::new();
        for result in storage.iter() {
            let message = try!(result);
            let entry = messages.entry(message.imei().to_string()).or_insert(Vec::new());
            entry.push(message);
        }
        let mut heartbeats = self.heartbeats.write().unwrap();
        heartbeats.clear();
        for (_, mut messages) in messages {
            messages.sort();
            heartbeats.extend(try!(messages.into_heartbeats())
                .into_iter()
                .filter_map(|h| h.ok()));
        }
        heartbeats.sort_by_key(|h| {
            h.messages
                .get(0)
                .map(|m| m.time_of_session())
                .unwrap_or(UTC::now())
        });
        Ok(())
    }
}
