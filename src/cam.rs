//! Images from the ATLAS camera.

use std::fs::read_dir;
use std::path::{Path, PathBuf};

use chrono::{DateTime, TimeZone, UTC};

use {Error, Result};

/// A directory full of ATLAS images.
#[derive(Debug)]
pub struct Directory {
    path: PathBuf,
}

impl Directory {
    /// Creates a new directory for the given path.
    pub fn new<P: AsRef<Path>>(path: P) -> Directory {
        Directory { path: path.as_ref().to_path_buf() }
    }

    /// Returns the filename of the latest image from this directory.
    ///
    /// This method will ignore all datetime parsing errors, in the interest of
    /// returning a value if one can be returned.
    pub fn latest(&self) -> Result<Option<(PathBuf, DateTime<UTC>)>> {
        let mut pairs: Vec<_> = try!(self.filenames())
            .into_iter()
            .filter_map(|p| datetime_from_path(&p).map(|d| (p, d)).ok())
            .collect();
        pairs.sort_by(|&(_, a), &(_, b)| a.cmp(&b));
        Ok(pairs.into_iter().last())
    }

    /// Returns all the image filenames in this directory.
    pub fn filenames(&self) -> Result<Vec<PathBuf>> {
        Ok(try!(read_dir(&self.path))
            .filter_map(|r| {
                r.map(|d| d.path()).ok().and_then(|p| p.file_name().map(|s| PathBuf::from(s)))
            })
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
        let directory = Directory::new("data");
        let (filename, datetime) = directory.latest().unwrap().unwrap();
        assert_eq!("ATLAS_CAM_20160725_141500.jpg", filename);
        assert_eq!(UTC.ymd(2016, 7, 25).and_hms(14, 15, 00), datetime);
    }

    #[test]
    fn filenames() {
        let directory = Directory::new("data");
        assert_eq!(vec!["ATLAS_CAM_20160725_121500.jpg", "ATLAS_CAM_20160725_141500.jpg"],
                   directory.filenames()
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
}
