use std::sync::{Arc, RwLock};
use std::sync::mpsc::channel;
use std::path::{Path, PathBuf};

use notify;
use notify::Watcher as NotifyWatcher;
use sbd::mo::Message;
use sbd::storage::FilesystemStorage;

use Result;
use heartbeat::{Heartbeat, Source};

/// Use changes under a directory to trigger a refresh of a heartbeat vector.
///
/// This is a multi-threaded way to keep a vector of heartbeats up-to-date.
///
/// # Examples
///
/// ```
/// use std::thread;
/// # use atlas::heartbeat::Watcher;
///
/// let watcher = Watcher::new("data").unwrap();
///
/// // The watcher keeps a `Arc<RwLock<Vec<Heartbeat>>>` to store the heartbeat vector.
/// // Use `Watcher::heartbeats` to get a clone of the `Arc`.
/// let heartbeats = watcher.heartbeats();
///
/// // The watcher will monitor the directory for changes in an infinite loop, so that should take
/// // place in another thread.
/// thread::spawn(move || watcher.watch().unwrap());
///
/// // Use the heartbeats vector
/// let heartbeats = heartbeats.read().unwrap();
/// ```
#[derive(Debug)]
pub struct Watcher {
    heartbeats: Arc<RwLock<Vec<Heartbeat>>>,
    root: PathBuf,
    source: Source<FilesystemStorage>,
}

impl Watcher {
    /// Creates a new watcher for the given directory.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::heartbeat::Watcher;
    /// let watcher = Watcher::new("data").unwrap();
    /// ```
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Watcher> {
        let source = Source::new(try!(FilesystemStorage::open(&path)));
        Ok(Watcher {
            heartbeats: Arc::new(RwLock::new(try!(source.heartbeats()))),
            root: path.as_ref().to_path_buf(),
            source: source,
        })
    }

    /// Clones the underlying `Arc` around the heartbeats vector and returns the clone.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::heartbeat::Watcher;
    /// let watcher = Watcher::new("data").unwrap();
    /// let heartbeats = watcher.heartbeats();
    /// ```
    pub fn heartbeats(&self) -> Arc<RwLock<Vec<Heartbeat>>> {
        self.heartbeats.clone()
    }

    /// Enters an infinite loop, watching the directory for changes and refilling the heartbeats.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    /// # use atlas::heartbeat::Watcher;
    /// let watcher = Watcher::new("data").unwrap();
    /// thread::spawn(move || watcher.watch().unwrap());
    /// ```
    pub fn watch(&self) -> Result<()> {
        let (tx, rx) = channel();
        let mut watcher: notify::RecommendedWatcher = try!(notify::Watcher::new(tx));
        try!(watcher.watch(&self.root));
        loop {
            match rx.recv() {
                Ok(notify::Event { path: Some(path), op: Ok(_) }) => {
                    if let Ok(metadata) = path.metadata() {
                        if metadata.is_dir() {
                            try!(watcher.unwatch(&self.root));
                            try!(watcher.watch(&self.root));
                        }
                    }
                    if Message::from_path(path).is_ok() {
                        let new_heartbeats = try!(self.source.heartbeats());
                        let mut heartbeats = self.heartbeats.write().unwrap();
                        heartbeats.clear();
                        heartbeats.extend(new_heartbeats.into_iter());
                    }
                }
                Err(_) => unimplemented!(),
                _ => (),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use sbd::storage::{FilesystemStorage, Storage};
    use tempdir::TempDir;

    use heartbeat::tests::one_v1_message;

    #[test]
    fn no_messages() {
        let dir = TempDir::new("atlas_heartbeat_watcher").unwrap();
        let watcher = Watcher::new(dir.path()).unwrap();
        let heartbeats = watcher.heartbeats();
        assert!(heartbeats.read().unwrap().is_empty());
    }

    #[test]
    fn one_message_start() {
        let dir = TempDir::new("atlas_heartbeat_watcher").unwrap();
        let mut storage = FilesystemStorage::open(dir.path()).unwrap();
        storage.store(one_v1_message()).unwrap();
        let watcher = Watcher::new(dir.path()).unwrap();
        let heartbeats = watcher.heartbeats();
        assert_eq!(1, heartbeats.read().unwrap().len());
    }
}
