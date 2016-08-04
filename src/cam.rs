//! Manage images from remote cameras.
//!
//! These include the ATLAS cam and other remote cameras e.g. the Helheim terminus cam.

use std::ffi::{OsStr, OsString};
use std::fs::read_dir;
use std::path::{Path, PathBuf};

use chrono::{DateTime, TimeZone, UTC};

use regex::Regex;

use url::Url;

use {Error, Result};

/// A remote camera, e.g. `ATLAS_CAM` or `HEL_TERMINUS`.
#[derive(Debug)]
pub struct Camera {
    name: String,
    path: PathBuf,
    regex: Regex,
}

impl Camera {
    /// Creates a new named camera that stores images in the given path.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::cam::Camera;
    /// let camera = Camera::new("ATLAS_CAM", "data").unwrap();
    /// ```
    pub fn new<P: AsRef<Path>>(name: &str, path: P) -> Result<Camera> {
        let mut regex_string = String::from(name);
        regex_string.push_str(r"_(?P<datetime>\d{4}\d{2}\d{2}_\d{2}\d{2}\d{2}).jpg");
        Ok(Camera {
            name: name.to_string(),
            path: path.as_ref().to_path_buf(),
            regex: try!(Regex::new(&regex_string)),
        })
    }

    /// Returns the file name of the latest image from this camera.
    ///
    /// This method will ignore all datetime parsing errors, in the interest of
    /// returning a value if one can be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::cam::Camera;
    /// let camera = Camera::new("ATLAS_CAM", "data").unwrap();
    /// let file_name = camera.latest_file_name().unwrap().unwrap();
    /// ```
    pub fn latest_file_name(&self) -> Result<Option<OsString>> {
        self.file_names().map(|v| v.into_iter().last())
    }

    /// Returns all the image file names for this camera.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::cam::Camera;
    /// let camera = Camera::new("ATLAS_CAM", "data").unwrap();
    /// let file_names = camera.file_names().unwrap();
    /// ```
    pub fn file_names(&self) -> Result<Vec<OsString>> {
        self.paths().map(|v| v.into_iter().map(|p| p.file_name().unwrap().to_os_string()).collect())
    }

    /// Returns all file names of images taken since a certain date.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate chrono;
    /// # extern crate atlas;
    /// # use chrono::{UTC, TimeZone};
    /// # use atlas::cam::Camera;
    /// # fn main() {
    /// let camera = Camera::new("ATLAS_CAM", "data").unwrap();
    /// let ref date = UTC.ymd(2016, 7, 25).and_hms(14, 15, 00);
    /// let file_names = camera.file_names_since(date).unwrap();
    /// # }
    /// ```
    pub fn file_names_since(&self, datetime: &DateTime<UTC>) -> Result<Vec<OsString>> {
        self.paths_since(datetime)
            .map(|v| v.into_iter().map(|p| p.file_name().unwrap().to_os_string()).collect())
    }

    /// Returns all paths of images taken since a certain date.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate chrono;
    /// # extern crate atlas;
    /// # use chrono::{UTC, TimeZone};
    /// # use atlas::cam::Camera;
    /// # fn main() {
    /// let camera = Camera::new("ATLAS_CAM", "data").unwrap();
    /// let paths = camera.paths_since(&UTC.ymd(2016, 7, 25).and_hms(14, 15, 00)).unwrap();
    /// # }
    /// ```
    pub fn paths_since(&self, datetime: &DateTime<UTC>) -> Result<Vec<PathBuf>> {
        self.paths().map(|v| {
            v.into_iter()
                .filter(|f| self.datetime(f).map(|d| &d > datetime).unwrap_or(false))
                .collect()
        })
    }

    /// Returns all paths of images taken by this camera.
    ///
    /// # Panics
    ///
    /// If the regular expression used for matching filenames fails for some reason, this will
    /// panic.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::cam::Camera;
    /// let camera = Camera::new("ATLAS_CAM", "data").unwrap();
    /// let paths = camera.paths().unwrap();
    /// ```
    pub fn paths(&self) -> Result<Vec<PathBuf>> {
        Ok(try!(read_dir(&self.path))
            .filter_map(|r| {
                r.ok().and_then(|d| if self.regex
                    .is_match(&d.file_name().to_string_lossy()) {
                    Some(d.path())
                } else {
                    None
                })
            })
            .collect::<Vec<_>>())
    }

    /// Returns the path of this camera.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::cam::Camera;
    /// let camera = Camera::new("ATLAS_CAM", "data").unwrap();
    /// assert_eq!("data", camera.path().to_string_lossy());
    /// ```
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the UTC datetime coded in an image path.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate chrono;
    /// # extern crate atlas;
    /// use chrono::{UTC, TimeZone};
    /// # use atlas::cam::Camera;
    /// # fn main() {
    /// let camera = Camera::new("ATLAS_CAM", "data").unwrap();
    /// let datetime = camera.datetime("data/ATLAS_CAM_20160725_121500.jpg").unwrap();
    /// assert_eq!(UTC.ymd(2016, 7, 25).and_hms(12, 15, 00), datetime);
    /// # }
    /// ```
    pub fn datetime<P: AsRef<Path>>(&self, path: P) -> Result<DateTime<UTC>> {
        if let Some(f) = path.as_ref().file_name() {
            if let Some(c) = self.regex.captures(&f.to_string_lossy()) {
                return UTC.datetime_from_str(&c["datetime"], "%Y%m%d_%H%M%S")
                    .map_err(|e| Error::from(e));
            }
        }
        Err(Error::InvalidCameraPath(self.name.clone(), path.as_ref().to_path_buf()))
    }

    /// Returns this camera's name.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::cam::Camera;
    /// let camera = Camera::new("ATLAS_CAM", "data").unwrap();
    /// assert_eq!("ATLAS_CAM", camera.name());
    /// ```
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Constructs a url using the given base url and the filename.
    ///
    /// The url is constructed by taking the base, adding the name of the parent directory of all
    /// of the images, then appending the image filename.
    ///
    /// Returns `None` if the provided url cannot be a base.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate url;
    /// # extern crate atlas;
    /// use url::Url;
    /// # use atlas::cam::Camera;
    /// fn main() {
    /// let url = Url::parse("http://iridiumcam.lidar.io").unwrap();
    /// let camera = Camera::new("", "data").unwrap();
    /// assert_eq!("http://iridiumcam.lidar.io/data/foobar.jpg",
    ///     camera.url(&url, "foobar.jpg").unwrap().as_str());
    /// # }
    /// ```
    pub fn url<S: AsRef<OsStr>>(&self, url: &Url, file_name: S) -> Option<Url> {
        let mut url = url.clone();
        self.path.file_name().and_then(|directory| {
            match url.path_segments_mut() {
                Ok(mut segments) => {
                    segments.push(&directory.to_string_lossy());
                    segments.push(&file_name.as_ref().to_string_lossy());
                }
                Err(_) => return None,
            }
            Some(url)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::{TimeZone, UTC};

    use url::Url;

    #[test]
    fn latest_image() {
        let camera = Camera::new("ATLAS_CAM", "data").unwrap();
        let file_name = camera.latest_file_name().unwrap().unwrap();
        assert_eq!("ATLAS_CAM_20160725_141500.jpg", file_name.to_string_lossy());
    }

    #[test]
    fn file_names() {
        let camera = Camera::new("ATLAS_CAM", "data").unwrap();
        assert_eq!(vec!["ATLAS_CAM_20160725_121500.jpg", "ATLAS_CAM_20160725_141500.jpg"],
                   camera.file_names()
                       .unwrap()
                       .iter()
                       .map(|f| f.as_os_str())
                       .collect::<Vec<_>>());
    }

    #[test]
    fn datetime_from_path_ok() {
        let camera = Camera::new("ATLAS_CAM", "data").unwrap();
        assert_eq!(UTC.ymd(2016, 7, 25).and_hms(14, 15, 0),
                   camera.datetime("ATLAS_CAM_20160725_141500.jpg").unwrap());
        assert_eq!(UTC.ymd(2016, 7, 25).and_hms(14, 15, 0),
                   camera.datetime("data/ATLAS_CAM_20160725_141500.jpg").unwrap());
    }

    #[test]
    fn file_names_since() {
        let camera = Camera::new("ATLAS_CAM", "data").unwrap();
        let file_names = camera.file_names_since(&UTC.ymd(2016, 7, 25).and_hms(10, 0, 0)).unwrap();
        assert_eq!(2, file_names.len());
        let file_names = camera.file_names_since(&UTC.ymd(2016, 7, 25).and_hms(13, 0, 0)).unwrap();
        assert_eq!(1, file_names.len());
    }

    #[test]
    fn hel_terminus_paths() {
        let camera = Camera::new("HEL_Terminus", "data").unwrap();
        let paths = camera.paths().unwrap();
        assert_eq!(2, paths.len());
    }

    #[test]
    fn hel_terminus_datetime() {
        let camera = Camera::new("HEL_Terminus", "data").unwrap();
        let file_name = camera.latest_file_name().unwrap().unwrap();
        assert_eq!(UTC.ymd(2016, 8, 3).and_hms(18, 0, 0),
                   camera.datetime(file_name).unwrap());
    }

    #[test]
    fn url() {
        let url = Url::parse("http://iridiumcam.lidar.io").unwrap();
        let camera = Camera::new("ATLAS_CAM", "data").unwrap();
        assert_eq!("http://iridiumcam.lidar.io/data/foobar.jpg",
                   camera.url(&url, "foobar.jpg").unwrap().as_str());
    }
}
