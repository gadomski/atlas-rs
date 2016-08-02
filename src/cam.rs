//! Manage images from remote cameras.
//!
//! These include the ATLAS cam and other remote cameras e.g. the Helheim terminus cam.

use std::ffi::OsString;
use std::fs::read_dir;
use std::path::{Path, PathBuf};

use chrono::{DateTime, TimeZone, UTC};

use {Error, Result};

/// A place where remote images are stored.
///
/// For now, this is just a local directory.
#[derive(Debug)]
pub struct Storage {
    path: PathBuf,
}

impl Storage {
    /// Creates a new storage for the given path.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::cam::Storage;
    /// let storage = Storage::new("data");
    /// ```
    pub fn new<P: AsRef<Path>>(path: P) -> Storage {
        Storage { path: path.as_ref().to_path_buf() }
    }

    /// Returns the file name of the latest image from this directory.
    ///
    /// This method will ignore all datetime parsing errors, in the interest of
    /// returning a value if one can be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::cam::Storage;
    /// let storage = Storage::new("data");
    /// let file_name = storage.latest_file_name().unwrap().unwrap();
    /// ```
    pub fn latest_file_name(&self) -> Result<Option<OsString>> {
        self.file_names().map(|v| v.into_iter().last())
    }

    /// Returns all the image file names in this directory.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::cam::Storage;
    /// let storage = Storage::new("data");
    /// let file_names = storage.file_names().unwrap();
    /// ```
    pub fn file_names(&self) -> Result<Vec<OsString>> {
        self.paths().map(|v| v.into_iter().map(|p| p.file_name().unwrap().to_os_string()).collect())
    }

    /// Returns all file names taken since a certain date.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate chrono;
    /// # extern crate atlas;
    /// # use chrono::{UTC, TimeZone};
    /// # use atlas::cam::Storage;
    /// # fn main() {
    /// let storage = Storage::new("data");
    /// let ref date = UTC.ymd(2016, 7, 25).and_hms(14, 15, 00);
    /// let file_names = storage.file_names_since(date).unwrap();
    /// # }
    /// ```
    pub fn file_names_since(&self, datetime: &DateTime<UTC>) -> Result<Vec<OsString>> {
        self.paths_since(datetime)
            .map(|v| v.into_iter().map(|p| p.file_name().unwrap().to_os_string()).collect())
    }

    /// Returns all paths taken since a certain date.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate chrono;
    /// # extern crate atlas;
    /// # use chrono::{UTC, TimeZone};
    /// # use atlas::cam::Storage;
    /// # fn main() {
    /// let storage = Storage::new("data");
    /// let paths = storage.paths_since(&UTC.ymd(2016, 7, 25).and_hms(14, 15, 00)).unwrap();
    /// # }
    /// ```
    pub fn paths_since(&self, datetime: &DateTime<UTC>) -> Result<Vec<PathBuf>> {
        self.paths().map(|v| {
            v.into_iter()
                .filter(|f| datetime_from_path(f).map(|d| &d > datetime).unwrap_or(false))
                .collect()
        })
    }

    /// Returns all paths in this storage.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::cam::Storage;
    /// let storage = Storage::new("data");
    /// let paths = storage.paths().unwrap();
    /// ```
    pub fn paths(&self) -> Result<Vec<PathBuf>> {
        Ok(try!(read_dir(&self.path))
            .filter_map(|r| r.map(|d| d.path()).ok())
            .filter(|p| p.extension() == Some("jpg".as_ref()))
            .collect::<Vec<_>>())
    }
}

/// Returns the UTC datetime coded in the path.
pub fn datetime_from_path<P: AsRef<Path>>(path: P) -> Result<DateTime<UTC>> {
    UTC.datetime_from_str(path.as_ref().file_name().and_then(|p| p.to_str()).unwrap_or(""),
                           "ATLAS_CAM_%Y%m%d_%H%M%S.jpg")
        .map_err(|e| Error::from(e))
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::{TimeZone, UTC};

    #[test]
    fn latest_image() {
        let storage = Storage::new("data");
        let file_name = storage.latest_file_name().unwrap().unwrap();
        assert_eq!("ATLAS_CAM_20160725_141500.jpg", file_name.to_string_lossy());
    }

    #[test]
    fn file_names() {
        let storage = Storage::new("data");
        assert_eq!(vec!["ATLAS_CAM_20160725_121500.jpg", "ATLAS_CAM_20160725_141500.jpg"],
                   storage.file_names()
                       .unwrap()
                       .iter()
                       .map(|f| f.as_os_str())
                       .collect::<Vec<_>>());
    }

    #[test]
    fn datetime_from_path_ok() {
        assert_eq!(UTC.ymd(2016, 7, 25).and_hms(14, 15, 0),
                   datetime_from_path("ATLAS_CAM_20160725_141500.jpg").unwrap());
        assert_eq!(UTC.ymd(2016, 7, 25).and_hms(14, 15, 0),
                   datetime_from_path("data/ATLAS_CAM_20160725_141500.jpg").unwrap());
    }

    #[test]
    fn file_names_since() {
        let storage = Storage::new("data");
        let file_names = storage.file_names_since(&UTC.ymd(2016, 7, 25).and_hms(10, 0, 0)).unwrap();
        assert_eq!(2, file_names.len());
        let file_names = storage.file_names_since(&UTC.ymd(2016, 7, 25).and_hms(13, 0, 0)).unwrap();
        assert_eq!(1, file_names.len());
    }
}
