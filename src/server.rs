//! Serve data using Iron.

use std::collections::BTreeMap;
use std::fmt::Write;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::thread;

use chrono::{Duration, UTC};

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
use magick::{GifHandler, GifWatcher};
use watch::{DirectoryWatcher, HeartbeatWatcher};

/// The ATLAS status server.
#[derive(Debug)]
pub struct Server {
    config: Configuration,
    heartbeats: Arc<RwLock<Vec<HeartbeatV1>>>,
    #[cfg(feature = "magick_rust")]
    gif: Arc<RwLock<Vec<u8>>>,
}

#[derive(Debug, RustcDecodable)]
struct Configuration {
    server: ServerConfig,
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
    img_dir: String,
    img_url: String,
}

#[cfg(feature = "magick_rust")]
#[derive(Debug, RustcDecodable)]
struct GifConfig {
    days: i64,
    delay: i64,
    width: u64,
    height: u64,
}

impl Server {
    /// Creates a new server from the provided toml configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::server::Server;
    /// let server = Server::new("data/server.toml").unwrap();
    /// ```
    pub fn new<P: AsRef<Path>>(config_file: P) -> Result<Server> {
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
        let config = try!(Configuration::decode(&mut decoder));
        Ok(Server {
            config: config,
            heartbeats: Arc::new(RwLock::new(Vec::new())),
            gif: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Starts the atlas server.
    ///
    /// This method should run forever.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use atlas::server::Server;
    /// let mut server = Server::new("data/server.toml").unwrap();
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
    /// let server = Server::new("data/server.toml").unwrap();
    /// let (ip, port) = server.addr();
    /// ```
    pub fn addr(&self) -> (&str, u16) {
        (&self.config.server.ip, self.config.server.port)
    }

    /// Returns the image directory for this server.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::server::Server;
    /// let server = Server::new("data/server.toml").unwrap();
    /// let dir = server.img_dir();
    /// ```
    pub fn img_dir(&self) -> &Path {
        Path::new(&self.config.server.img_dir)
    }

    /// Creates and returns new image url.
    ///
    /// This is the url that's used to construct image src attributes on the webpage.
    ///
    /// # Examples
    ///
    /// ```
    /// # use atlas::server::Server;
    /// let server = Server::new("data/server.toml").unwrap();
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
    /// let server = Server::new("data/server.toml").unwrap();
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
    /// let server = Server::new("data/server.toml").unwrap();
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
    /// let server = Server::new("data/server.toml").unwrap();
    /// let static_path = server.resource_path("static");
    /// ```
    pub fn resource_path<P: AsRef<Path>>(&self, other: P) -> PathBuf {
        let mut path = PathBuf::from(&self.config.server.resource_dir);
        path.push(other);
        path
    }

    fn router(&self) -> Result<Router> {
        let mut router = Router::new();
        router.get("/",
                   try!(IndexHandler::new(self.heartbeats.clone(),
                                          &self.img_dir(),
                                          try!(self.img_url()))));
        router.get("/soc.csv",
                   CsvHandler::new(self.heartbeats.clone(), SocCsvProvider));
        router.get("/temperature.csv",
                   CsvHandler::new(self.heartbeats.clone(), TemperatureCsvProvider));

        self.add_gif_handler(&mut router);
        Ok(router)
    }

    fn staticfiles(&self) -> Static {
        let static_path = self.resource_path("static");
        Static::new(static_path)
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
    fn add_gif_handler(&self, router: &mut Router) {
        router.get("/atlas-cam.gif", GifHandler::new(self.gif.clone()));
    }

    #[cfg(feature = "magick_rust")]
    fn start_gif_watcher(&self) -> Result<()> {
        // FIXME adapt to other things, maybe
        let camera = try!(Camera::new("ATLAS_CAM", self.img_dir()));
        let mut watcher = GifWatcher::new(camera,
                                          Duration::days(self.config.gif.days),
                                          Duration::milliseconds(self.config.gif.delay),
                                          self.config.gif.width,
                                          self.config.gif.height,
                                          self.gif.clone());
        thread::spawn(move || {
            watcher.refresh().unwrap();
            watcher.watch().unwrap();
        });
        Ok(())
    }
}

/// The main page for the atlas status site, http://atlas.lidar.io.
#[derive(Debug)]
pub struct IndexHandler {
    heartbeats: Arc<RwLock<Vec<HeartbeatV1>>>,
    img_directory: Camera,
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
    /// # fn main() {
    /// let heartbeats = Arc::new(RwLock::new(Vec::new()));
    /// let url = url::Url::parse("http://iridiumcam.lidar.io").unwrap();
    /// let handler = IndexHandler::new(heartbeats, "data", url).unwrap();
    /// # }
    /// ```
    pub fn new<P: AsRef<Path>>(heartbeats: Arc<RwLock<Vec<HeartbeatV1>>>,
                               img_dir: P,
                               img_url: Url)
                               -> Result<IndexHandler> {
        Ok(IndexHandler {
            heartbeats: heartbeats,
            // FIXME we don't just want ATLAS_CAM
            img_directory: Camera::new("ATLAS_CAM", img_dir).unwrap(),
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
        let mut url = self.url.clone();
        let file_name = iexpect!(itry!(self.img_directory.latest_file_name()),
                                 (status::NotFound, "No images available."));
        iexpect!(url.path_segments_mut().ok()).push(&file_name.to_string_lossy());
        data.insert("latest_image_url".to_string(),
                    url.as_str()
                        .to_json());
        data.insert("latest_image_datetime".to_string(),
                    itry!(self.img_directory.datetime(file_name)).to_string().to_json());
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
        let server = Server::new("data/server.toml").unwrap();
        let (ip, port) = server.addr();
        assert_eq!(ip, "0.0.0.0");
        assert_eq!(port, 3000);
    }

    #[test]
    fn img_dir() {
        let server = Server::new("data/server.toml").unwrap();
        assert_eq!("/Users/gadomski/iridiumcam/ATLAS_CAM",
                   server.img_dir().to_string_lossy());
    }

    #[test]
    fn iridium_dir() {
        let server = Server::new("data/server.toml").unwrap();
        assert_eq!("/Users/gadomski/iridium",
                   server.iridium_dir().to_string_lossy());
    }

    #[test]
    fn imei() {
        let server = Server::new("data/server.toml").unwrap();
        assert_eq!("300234063909200", server.imei());
    }

    #[test]
    fn img_url() {
        let server = Server::new("data/server.toml").unwrap();
        assert_eq!("http://iridiumcam.lidar.io/ATLAS_CAM",
                   server.img_url().unwrap().as_str());
    }

    #[test]
    fn resource_path() {
        let server = Server::new("data/server.toml").unwrap();
        assert_eq!("/Users/gadomski/Repos/atlas-rs/static",
                   server.resource_path("static").to_string_lossy());
    }
}
