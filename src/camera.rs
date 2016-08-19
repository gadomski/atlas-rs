//! We have several remote cameras scattered around the Helheim area that send images back to us.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use {Error, Result};

/// A remote camera.
///
/// These cameras send back pictures to one of our servers, where they are accessible through an
/// HTTP server. The picture files are named with patterns that include the camera name and time of
/// capture.
///
/// Creating a new camera is a matter of providing the directory that holds the camera's images:
///
/// ```
/// use atlas::camera::Camera;
/// let camera = Camera::new("data/ATLAS_CAM").unwrap();
/// ```
///
/// The camera's name is part of regular expression that processes the image names. By default, the
/// camera's name is assumed to be the same as the directory that holds the pictures. If that is
/// not the case, you can specify the camera's name:
///
/// ```
/// # use atlas::camera::Camera;
/// # let mut camera = Camera::new("data/ATLAS_CAM").unwrap();
/// camera.set_name("Atlas_Cam");
/// ```
#[derive(Debug)]
pub struct Camera {
    path: PathBuf,
    name: OsString,
}

impl Camera {
    /// Creates a new camera for the provided directory.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::camera::Camera;
    /// let camera = Camera::new("data/ATLAS_CAM").unwrap();
    /// ```
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Camera> {
        Ok(Camera {
            path: path.as_ref().to_path_buf(),
            name: try!(path.as_ref()
                .file_name()
                .map(|s| s.to_os_string())
                .ok_or(Error::InvalidCameraPath(path.as_ref().to_path_buf()))),
        })
    }

    /// Returns the camera's name.
    ///
    /// By default, this is the name of the directory that holds the camera's images. But this
    /// value can be set with `set_name`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::camera::Camera;
    /// let camera = Camera::new("data/ATLAS_CAM").unwrap();
    /// assert_eq!("ATLAS_CAM", camera.name());
    /// ```
    pub fn name(&self) -> &OsStr {
        &self.name
    }

    /// Sets the camera's name.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::camera::Camera;
    /// let mut camera = Camera::new("data/ATLAS_CAM").unwrap();
    /// camera.set_name("AtlasCam");
    /// assert_eq!("AtlasCam", camera.name());
    /// ```
    pub fn set_name<S: AsRef<OsStr>>(&mut self, name: S) {
        self.name = name.as_ref().to_os_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_name() {
        let mut camera = Camera::new("data/ATLAS_CAM").unwrap();
        assert_eq!("ATLAS_CAM", camera.name());
        camera.set_name("AtlasCam");
        assert_eq!("AtlasCam", camera.name());
    }
}
