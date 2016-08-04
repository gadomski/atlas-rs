//! Tools that use ImageMagick.
//!
//! ImageMagick isn't available on all systems, and [the bindings we
//! use](https://github.com/nlfiedler/magick-rust) don't always build out right (e.g. on Travis),
//! so we quarentine all ImageMagick stuff in this module.

use std::path::{Path, PathBuf};
use std::sync::{Arc, ONCE_INIT, Once, RwLock};

use chrono::{DateTime, Duration, UTC};

use iron::{Handler, status};
use iron::prelude::*;
use iron::mime::Mime;

use magick_rust::{MagickWand, magick_wand_genesis};

use {Error, Result};
use cam::Camera;
use watch::DirectoryWatcher;

static START: Once = ONCE_INIT;
const DEFAULT_LOOP: bool = true;

macro_rules! try_magick{ ($x:expr) => {{
    match $x {
        Ok(result) => result,
        Err(s) => return Err(Error::Magick(s.to_string())),
    }
}};
}

/// A simple structure to hold common gif configuration values.
#[derive(Copy, Clone, Debug)]
pub struct GifConfig {
    /// The length of time between frames of the gif.
    pub delay: Duration,
    /// The height of the gif.
    pub height: u64,
    /// The width of the gif.
    pub width: u64,
}

impl Default for GifConfig {
    fn default() -> GifConfig {
        GifConfig {
            width: 512,
            height: 384,
            delay: Duration::milliseconds(500),
        }
    }
}

/// A structure that creates a gif from a directory of images.
#[derive(Debug)]
pub struct GifMaker {
    camera: Camera,
    config: GifConfig,
}

impl GifMaker {
    /// Creates a new `GifMaker`.
    ///
    /// The path is to a directory full of gif-able images, and the height and width define the
    /// size of the gif.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::magick::GifMaker;
    /// let gif_maker = GifMaker::new(atlas::cam::Camera::new("ATLAS_CAM", "data").unwrap(),
    ///                               Default::default());
    /// ```
    pub fn new(camera: Camera, config: GifConfig) -> GifMaker {
        GifMaker {
            camera: camera,
            config: config,
        }
    }

    /// Returns a gif, as a `Vec<u8>`, of all images since the given date time.
    ///
    /// ```
    /// # extern crate chrono;
    /// # extern crate atlas;
    /// # use chrono::{Duration, UTC, TimeZone};
    /// # use atlas::magick::GifMaker;
    /// # fn main() {
    /// let gif_maker = GifMaker::new(atlas::cam::Camera::new("ATLAS_CAM", "data").unwrap(),
    ///                               Default::default());
    /// let ref datetime = UTC.ymd(2016, 7, 25).and_hms(0, 0, 0);
    /// let gif = gif_maker.since(datetime).unwrap();
    /// # }
    pub fn since(&self, since: &DateTime<UTC>) -> Result<Vec<u8>> {
        START.call_once(|| magick_wand_genesis());
        let filenames = try!(self.camera.paths_since(since))
            .into_iter()
            .collect::<Vec<_>>();
        let mut wand = MagickWand::new();
        for filename in filenames {
            try_magick!(wand.read_image(&filename.to_string_lossy()));
        }
        try_magick!(wand.set_image_delay((self.config.delay.num_milliseconds() / 10) as u64));
        wand.fit(self.config.width, self.config.height);
        let loop_str = if DEFAULT_LOOP {
            "0"
        } else {
            "1"
        };
        try_magick!(wand.set_option("loop", loop_str));
        Ok(try_magick!(wand.write_images_blob("gif")))
    }
}
/// Watches a directory and refreshes a gif.
#[derive(Debug)]
pub struct GifWatcher {
    directory: PathBuf,
    gif_maker: GifMaker,
    gif: Arc<RwLock<Vec<u8>>>,
    duration: Duration,
}

impl GifWatcher {
    /// Creates a new watcher.
    ///
    /// This wacher, when started with `watch`, will react to any changes to that directory. When
    /// it detects a change (e.g. a new image file) it will re-create a the gif using GifMaker,
    /// using all images between now and `duration` ago. The width, height, and delay arguments are
    /// passed on to the underlying `GifMaker`.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate chrono;
    /// # extern crate atlas;
    /// use chrono::Duration;
    /// # use std::sync::{RwLock, Arc};
    /// # use atlas::magick::GifWatcher;
    /// # fn main() {
    /// let gif = Arc::new(RwLock::new(Vec::new()));
    /// let watcher = GifWatcher::new(atlas::cam::Camera::new("ATLAS_CAM", "data").unwrap(),
    ///                               Duration::days(2),
    ///                               Default::default(),
    ///                               gif);
    /// # }
    /// ```
    pub fn new(camera: Camera,
               duration: Duration,
               config: GifConfig,
               gif: Arc<RwLock<Vec<u8>>>)
               -> GifWatcher {
        GifWatcher {
            directory: camera.path().to_path_buf(),
            gif_maker: GifMaker::new(camera, config),
            gif: gif,
            duration: duration,
        }
    }
}

impl DirectoryWatcher for GifWatcher {
    fn directory(&self) -> &Path {
        self.directory.as_path()
    }

    fn refresh(&mut self) -> Result<()> {
        let new_gif = try!(self.gif_maker.since(&(UTC::now() - self.duration)));
        let mut gif = self.gif.write().unwrap();
        gif.clear();
        gif.extend(new_gif.into_iter());
        Ok(())
    }
}

/// Iron `Handler` that serves up a gif of the ATLAS system.
#[derive(Debug)]
pub struct GifHandler {
    gif: Arc<RwLock<Vec<u8>>>,
}

impl GifHandler {
    /// Creates a new gif handler that will serve the provided gif.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate chrono;
    /// # extern crate atlas;
    /// use chrono::Duration;
    /// # use std::sync::{Arc, RwLock};
    /// # use atlas::magick::GifHandler;
    /// # fn main() {
    /// let gif = Arc::new(RwLock::new(Vec::new()));
    /// let handler = GifHandler::new(gif.clone());
    /// # }
    /// ```
    pub fn new(gif: Arc<RwLock<Vec<u8>>>) -> GifHandler {
        GifHandler { gif: gif }
    }
}

impl Handler for GifHandler {
    fn handle(&self, _: &mut Request) -> IronResult<Response> {
        let gif = self.gif.read().unwrap();
        if gif.is_empty() {
            return Ok(Response::with((status::ServiceUnavailable, "gif is empty")));
        }
        let content_type = "image/gif".parse::<Mime>().unwrap();
        Ok(Response::with((content_type, status::Ok, gif.clone())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::{Duration, TimeZone, UTC};

    use cam::Camera;

    #[test]
    fn makes_gif() {
        let gifmaker = GifMaker::new(Camera::new("ATLAS_CAM", "data").unwrap(),
                                     GifConfig {
                                         width: 512,
                                         height: 282,
                                         delay: Duration::milliseconds(200),
                                     });
        let _ = gifmaker.since(&UTC.ymd(2016, 1, 1).and_hms(0, 0, 0)).unwrap();
    }
}
