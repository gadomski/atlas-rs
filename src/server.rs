//! Serve data using Iron.

use std::ascii::AsciiExt;
use std::collections::BTreeMap;
#[cfg(feature = "magick_rust")]
use std::collections::HashMap;
use std::fmt::Write;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::thread;

#[cfg(feature = "magick_rust")]
use chrono::Duration;
use chrono::UTC;

use handlebars_iron::{DirectorySource, HandlebarsEngine, Template};

use iron::{Handler, Listening, status};
use iron::error::HttpResult;
use iron::prelude::*;
use iron::headers::ContentType;
use iron::mime::{Mime, SubLevel, TopLevel};

use logger;

use mount::Mount;

use router::Router;

use rustc_serialize::json::{Json, ToJson};
use rustc_serialize::Decodable;

use staticfile::Static;

use toml;

use url::Url;

use {Error, Result};
use cam::Camera;
use heartbeat::{HeartbeatV1, expected_next_scan_time};
use watch::{DirectoryWatcher, HeartbeatWatcher};
#[cfg(feature = "magick_rust")]
use magick::{self, GifHandler, GifWatcher};

/// The ATLAS status server.
///
/// The server is configured with a toml file. See `data/config.toml` in this repository for an
/// example of a config file.
#[derive(Debug)]
pub struct Server {
    config: Configuration,
    heartbeats: Arc<RwLock<Vec<HeartbeatV1>>>,
    #[cfg(feature = "magick_rust")]
    gifs: HashMap<String, Arc<RwLock<Vec<u8>>>>,
}

#[derive(Debug, RustcDecodable)]
struct Configuration {
    server: ServerConfig,
    camera: Vec<CameraConfig>,
    #[cfg(feature = "magick_rust")]
    gif: GifConfig,
}

#[derive(Debug, RustcDecodable)]
struct ServerConfig {
    ip: String,
    port: u16,
    resource_dir: String,
    iridium_dir: String,
    imei: String,
    img_url: String,
    active_camera: String,
}

#[derive(Debug, RustcDecodable)]
struct CameraConfig {
    directory: String,
    name: Option<String>,
}

#[cfg(feature = "magick_rust")]
#[derive(Debug, RustcDecodable)]
struct GifConfig {
    days: i64,
    delay: i64,
    width: u64,
    height: u64,
    names: Vec<String>,
}

impl Server {
    /// Creates a new server from the provided toml configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::server::Server;
    /// let server = Server::new("data/config.toml").unwrap();
    /// ```
    #[cfg(feature = "magick_rust")]
    pub fn new<P: AsRef<Path>>(config_file: P) -> Result<Server> {
        let config = try!(Server::config_from_file(config_file));
        Ok(Server {
            gifs: config.gif
                .names
                .iter()
                .map(|n| (n.to_string(), Arc::new(RwLock::new(Vec::new()))))
                .collect(),
            config: config,
            heartbeats: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Creates a new server from the provided toml configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::server::Server;
    /// let server = Server::new("data/config.toml").unwrap();
    /// ```
    #[cfg(not(feature = "magick_rust"))]
    pub fn new<P: AsRef<Path>>(config_file: P) -> Result<Server> {
        let config = try!(Server::config_from_file(config_file));
        Ok(Server {
            config: config,
            heartbeats: Arc::new(RwLock::new(Vec::new())),
        })
    }

    fn config_from_file<P: AsRef<Path>>(config_file: P) -> Result<Configuration> {
        let mut config = String::new();
        {
            let mut file = try!(File::open(config_file));
            try!(file.read_to_string(&mut config));
        }
        let mut parser = toml::Parser::new(&config);
        let toml = match parser.parse() {
            Some(t) => t,
            None => return Err(Error::TomlParse(parser.errors.clone())),
        };
        let mut decoder = toml::Decoder::new(toml::Value::Table(toml));
        Configuration::decode(&mut decoder).map_err(|e| Error::from(e))
    }

    /// Starts the atlas server.
    ///
    /// This method should run forever.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use atlas::server::Server;
    /// let mut server = Server::new("data/config.toml").unwrap();
    /// server.serve().unwrap().unwrap();
    /// ```
    pub fn serve(&mut self) -> Result<HttpResult<Listening>> {
        let mut mount = Mount::new();
        mount.mount("/static/", self.staticfiles());
        mount.mount("/", try!(self.router()));
        let mut chain = Chain::new(mount);
        chain.link_after(try!(self.handlebars_engine()));
        chain.link(self.logger());

        self.start_heartbeat_watcher();
        try!(self.start_gif_watcher());
        Ok(Iron::new(chain).http(self.addr()))
    }

    /// Returns this server's address as an (ip, port) pair.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::server::Server;
    /// let server = Server::new("data/config.toml").unwrap();
    /// let (ip, port) = server.addr();
    /// ```
    pub fn addr(&self) -> (&str, u16) {
        (&self.config.server.ip, self.config.server.port)
    }

    /// Creates and returns new image url.
    ///
    /// This is the url that's used to construct image src attributes on the webpage.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::server::Server;
    /// let server = Server::new("data/config.toml").unwrap();
    /// let url = server.img_url().unwrap();
    /// ```
    pub fn img_url(&self) -> Result<Url> {
        Url::parse(&self.config.server.img_url).map_err(|e| Error::from(e))
    }

    /// Returns iridium directory.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::server::Server;
    /// let server = Server::new("data/config.toml").unwrap();
    /// let dir = server.iridium_dir();
    /// ```
    pub fn iridium_dir(&self) -> &Path {
        Path::new(&self.config.server.iridium_dir)
    }

    /// Returns the IMEI number.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::server::Server;
    /// let server = Server::new("data/config.toml").unwrap();
    /// let imei = server.imei();
    /// ```
    pub fn imei(&self) -> &str {
        &self.config.server.imei
    }

    /// Returns a `PathBuf` to a resource directory.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::server::Server;
    /// let server = Server::new("data/config.toml").unwrap();
    /// let static_path = server.resource_path("static");
    /// ```
    pub fn resource_path<P: AsRef<Path>>(&self, other: P) -> PathBuf {
        let mut path = PathBuf::from(&self.config.server.resource_dir);
        path.push(other);
        path
    }

    /// Returns a vector of this server's cameras.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::server::Server;
    /// let server = Server::new("data/config.toml").unwrap();
    /// let cameras = server.cameras();
    /// ```
    pub fn cameras(&self) -> Result<Vec<Camera>> {
        self.config
            .camera
            .iter()
            .map(|c| {
                let directory = Path::new(&c.directory);
                let name = match c.name {
                    Some(ref name) => name.to_string(),
                    None => {
                        try!(directory.file_name()
                                .ok_or(Error::InvalidCameraPath("no file name".to_string(),
                                                                directory.to_path_buf())))
                            .to_string_lossy()
                            .into_owned()
                    }
                };
                Camera::new(&name, directory)
            })
            .collect()
    }

    #[cfg(feature = "magick_rust")]
    fn camera_map(&self) -> Result<HashMap<String, Camera>> {
        self.cameras().map(|v| {
            v.into_iter()
                .map(|c| (c.name().to_string(), c))
                .collect::<HashMap<String, Camera>>()
        })
    }

    fn router(&self) -> Result<Router> {
        let mut router = Router::new();
        router.get("/",
                   try!(IndexHandler::new(self.heartbeats.clone(),
                                          try!(self.cameras()),
                                          &self.config.server.active_camera,
                                          try!(self.img_url()))));
        router.get("/soc.csv",
                   CsvHandler::new(self.heartbeats.clone(), SocCsvProvider));
        router.get("/temperature.csv",
                   CsvHandler::new(self.heartbeats.clone(), TemperatureCsvProvider));

        try!(self.add_gif_handler(&mut router));
        Ok(router)
    }

    fn staticfiles(&self) -> Static {
        Static::new(self.resource_path("static"))
    }

    fn handlebars_engine(&self) -> Result<HandlebarsEngine> {
        let mut hbse = HandlebarsEngine::new();
        let template_path = self.resource_path("templates");
        hbse.add(Box::new(DirectorySource::new(&template_path.to_string_lossy(), ".hbs")));
        // FIXME
        hbse.reload().unwrap();
        Ok(hbse)
    }

    fn logger(&self) -> (logger::Logger, logger::Logger) {
        let format = logger::format::Format::new("{method} {uri} -> {status} ({response-time})",
                                                 vec![],
                                                 vec![]);
        logger::Logger::new(format)
    }

    fn start_heartbeat_watcher(&self) {
        let heartbeats = self.heartbeats.clone();
        let mut watcher = HeartbeatWatcher::new(self.iridium_dir(), self.imei(), heartbeats);
        thread::spawn(move || {
            watcher.refresh().unwrap();
            watcher.watch().unwrap();
        });
    }

    #[cfg(feature = "magick_rust")]
    fn add_gif_handler(&self, router: &mut Router) -> Result<()> {
        let mut cameras = try!(self.camera_map());
        for name in self.config.gif.names.iter() {
            match cameras.remove(name) {
                Some(camera) => {
                    router.get(format!("/{}.gif", camera.name().to_ascii_lowercase()),
                               GifHandler::new(self.gifs[camera.name()].clone()));
                }
                None => {
                    return Err(Error::ServerConfigError(format!("Invalid camera name in gif \
                                                                 config: {}",
                                                                name)))
                }
            }
        }
        Ok(())
    }

    #[cfg(not(feature = "magick_rust"))]
    fn add_gif_handler(&self, _: &mut Router) -> Result<()> {
        Ok(())
    }

    #[cfg(feature = "magick_rust")]
    fn start_gif_watcher(&self) -> Result<()> {
        let mut cameras = try!(self.camera_map());
        let gif_config = magick::GifConfig {
            width: self.config.gif.width,
            height: self.config.gif.height,
            delay: Duration::milliseconds(self.config.gif.delay),
        };
        for name in self.config.gif.names.iter() {
            match cameras.remove(name) {
                Some(camera) => {
                    let mut watcher = GifWatcher::new(camera,
                                                      Duration::days(self.config.gif.days),
                                                      gif_config,
                                                      self.gifs[name].clone());
                    thread::spawn(move || {
                        watcher.refresh().unwrap();
                        watcher.watch().unwrap();
                    });
                }
                None => {
                    return Err(Error::ServerConfigError(format!("Could not start gif watcher \
                                                                 for camera: {}",
                                                                name)))
                }
            }
        }
        Ok(())
    }

    #[cfg(not(feature = "magick_rust"))]
    fn start_gif_watcher(&self) -> Result<()> {
        Ok(())
    }
}

/// The main page for the atlas status site, http://atlas.lidar.io.
#[derive(Debug)]
pub struct IndexHandler {
    heartbeats: Arc<RwLock<Vec<HeartbeatV1>>>,
    cameras: Vec<Camera>,
    active_camera: String,
    url: Url,
}

impl IndexHandler {
    /// Creates a new index handler for the given heartbeats, images, and image url.
    ///
    /// This handler will use the provided heartbeats to build the index page, and will use the
    /// local image directory to create image tags that point at the image url.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate url;
    /// # extern crate atlas;
    /// # use std::sync::{Arc, RwLock};
    /// # use atlas::server::IndexHandler;
    /// use atlas::cam::Camera;
    /// # fn main() {
    /// let heartbeats = Arc::new(RwLock::new(Vec::new()));
    /// let url = url::Url::parse("http://iridiumcam.lidar.io").unwrap();
    /// let cameras = vec![Camera::new("ATLAS_CAM", "data").unwrap()];
    /// let handler = IndexHandler::new(heartbeats, cameras, "ATLAS_CAM", url).unwrap();
    /// # }
    /// ```
    pub fn new(heartbeats: Arc<RwLock<Vec<HeartbeatV1>>>,
               cameras: Vec<Camera>,
               active_camera: &str,
               img_url: Url)
               -> Result<IndexHandler> {
        let mut seen_active_camera = false;
        for camera in cameras.iter() {
            if try!(camera.latest_file_name()).is_none() {
                return Err(Error::ServerConfigError(format!("Could not find the latest image \
                                                             for camera {} (path: {})",
                                                            camera.name(),
                                                            camera.path().to_string_lossy())));
            }
            if camera.name() == active_camera {
                seen_active_camera = true;
            }
        }
        if !seen_active_camera {
            return Err(Error::ServerConfigError(format!("Invalid active camera name: {}",
                                                        active_camera)));
        }
        Ok(IndexHandler {
            heartbeats: heartbeats,
            cameras: cameras,
            active_camera: active_camera.to_string(),
            url: img_url,
        })
    }
}

impl Handler for IndexHandler {
    fn handle(&self, _: &mut Request) -> IronResult<Response> {
        let heartbeats = self.heartbeats.read().unwrap();
        let heartbeat = iexpect!(heartbeats.last(),
                                 (status::NotFound, "No heartbeats available."));
        let mut data = BTreeMap::<String, Json>::new();
        data.insert("last_heartbeat".to_string(),
                    iexpect!(heartbeat.messages.first()).time_of_session().to_string().to_json());
        data.insert("last_scan_start".to_string(),
                    heartbeat.scan_start_datetime.to_string().to_json());
        data.insert("next_scan_start".to_string(),
                    expected_next_scan_time(&heartbeat.scan_start_datetime).to_string().to_json());
        data.insert("temperature_external".to_string(),
                    format!("{}", heartbeat.temperature_external).to_json());
        data.insert("temperature_mount".to_string(),
                    format!("{}", heartbeat.temperature_mount).to_json());
        data.insert("pressure".to_string(),
                    format!("{}", heartbeat.pressure).to_json());
        data.insert("humidity".to_string(),
                    format!("{}", heartbeat.humidity).to_json());
        data.insert("soc1".to_string(), format!("{}", heartbeat.soc1).to_json());
        data.insert("soc2".to_string(), format!("{}", heartbeat.soc2).to_json());

        let images: Vec<_> = iexpect!(self.cameras
            .iter()
            .map(|c| {
                c.latest_file_name().ok().and_then(|o| o).and_then(|file_name| {
                    c.url(&self.url, &file_name).and_then(|url| {
                        c.datetime(file_name).ok().and_then(|datetime| {
                            let mut map: BTreeMap<String, Json> = BTreeMap::new();
                            map.insert("url".to_string(), url.as_str().to_json());
                            map.insert("datetime".to_string(), datetime.to_string().to_json());
                            map.insert("id".to_string(),
                                       format!("latest_image_{}", c.name().to_ascii_lowercase())
                                           .to_json());
                            map.insert("name".to_string(), c.name().to_json());
                            if c.name() == self.active_camera {
                                map.insert("active".to_string(), "active".to_json());
                            }
                            Some(map)
                        })
                    })
                })
            })
            .collect());
        data.insert("latest_images".to_string(), images.to_json());

        data.insert("now".to_string(),
                    format!("{}", UTC::now().format("%Y-%m-%d %H:%M:%S UTC")).to_json());

        let mut response = Response::new();
        response.set_mut(Template::new("index", data)).set_mut(status::Ok);
        Ok(response)
    }
}

/// A Iron handler that returns CSV data.
///
/// The CSV data is provided by a `CsvProvider`, which uses heartbeat information to return
/// formatted strings.
#[derive(Debug)]
pub struct CsvHandler<T: CsvProvider> {
    heartbeats: Arc<RwLock<Vec<HeartbeatV1>>>,
    provider: T,
}

impl<T: CsvProvider> CsvHandler<T> {
    /// Creates a new CsvHandler.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::sync::{RwLock, Arc};
    /// # use atlas::server::{CsvHandler, SocCsvProvider};
    /// let heartbeats = Arc::new(RwLock::new(Vec::new()));
    /// let handler = CsvHandler::new(heartbeats, SocCsvProvider);
    /// ```
    pub fn new(heartbeats: Arc<RwLock<Vec<HeartbeatV1>>>, provider: T) -> CsvHandler<T> {
        CsvHandler {
            heartbeats: heartbeats,
            provider: provider,
        }
    }
}

impl<T: CsvProvider + Send + Sync + 'static> Handler for CsvHandler<T> {
    fn handle(&self, _: &mut Request) -> IronResult<Response> {
        let mut response = Response::new();
        response.status = Some(status::Ok);
        response.headers
            .set(ContentType(Mime(TopLevel::Text, SubLevel::Ext("csv".to_string()), vec![])));
        let mut data = String::new();

        writeln!(&mut data, "Datetime,{}", self.provider.header().join(",")).unwrap();
        for heartbeat in self.heartbeats.read().unwrap().iter() {
            write!(&mut data,
                   "{},",
                   iexpect!(heartbeat.messages.first()).time_of_session())
                .unwrap();
            let fields = self.provider.fields(&heartbeat);
            writeln!(&mut data, "{}", fields.join(",")).unwrap();
        }
        response.body = Some(Box::new(data));
        Ok(response)
    }
}

/// A trait for things that can provide CSV data.
///
/// This is used with `CsvHandler` to define fixed CSV endpoints.
pub trait CsvProvider {
    /// Returns the csv header names.
    fn header(&self) -> Vec<&'static str>;
    /// Returns the csv data extracted from the heartbeat.
    fn fields(&self, heartbeat: &HeartbeatV1) -> Vec<String>;
}

/// Provides state of charge information about the batteries.
#[derive(Clone, Copy, Debug)]
pub struct SocCsvProvider;

impl CsvProvider for SocCsvProvider {
    fn header(&self) -> Vec<&'static str> {
        vec!["Battery #1", "Battery #2"]
    }
    fn fields(&self, heartbeat: &HeartbeatV1) -> Vec<String> {
        vec![format!("{:.1}", heartbeat.soc1.percentage()),
             format!("{:.1}", heartbeat.soc2.percentage())]
    }
}

/// Provides temperature information (external and mount temps).
#[derive(Clone, Copy, Debug)]
pub struct TemperatureCsvProvider;

impl CsvProvider for TemperatureCsvProvider {
    fn header(&self) -> Vec<&'static str> {
        vec!["External", "Mount"]
    }
    fn fields(&self, heartbeat: &HeartbeatV1) -> Vec<String> {
        vec![format!("{:.1}", heartbeat.temperature_external),
             format!("{:.1}", heartbeat.temperature_mount)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addr() {
        let server = Server::new("data/config.toml").unwrap();
        let (ip, port) = server.addr();
        assert_eq!(ip, "0.0.0.0");
        assert_eq!(port, 3000);
    }

    #[test]
    fn iridium_dir() {
        let server = Server::new("data/config.toml").unwrap();
        assert_eq!("/Users/gadomski/iridium",
                   server.iridium_dir().to_string_lossy());
    }

    #[test]
    fn imei() {
        let server = Server::new("data/config.toml").unwrap();
        assert_eq!("300234063909200", server.imei());
    }

    #[test]
    fn img_url() {
        let server = Server::new("data/config.toml").unwrap();
        assert_eq!("http://iridiumcam.lidar.io/",
                   server.img_url().unwrap().as_str());
    }

    #[test]
    fn resource_path() {
        let server = Server::new("data/config.toml").unwrap();
        assert_eq!("/Users/gadomski/Repos/atlas-rs/static",
                   server.resource_path("static").to_string_lossy());
    }

    #[test]
    fn cameras() {
        let server = Server::new("data/config.toml").unwrap();
        let mut cameras = server.cameras().unwrap();
        assert_eq!(3, cameras.len());
        let _ = cameras.pop().unwrap();
        let camera = cameras.pop().unwrap();
        assert_eq!("HEL_Terminus", camera.name());
        assert_eq!("/Users/gadomski/iridiumcam/HEL_TERMINUS",
                   camera.path().to_string_lossy());
        let camera = cameras.pop().unwrap();
        assert_eq!("ATLAS_CAM", camera.name());
        assert_eq!("/Users/gadomski/iridiumcam/ATLAS_CAM",
                   camera.path().to_string_lossy());
    }
}
