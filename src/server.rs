//! Serve data using Iron.

use std::collections::BTreeMap;
use std::fmt::Write;
use std::sync::{Arc, RwLock};

use chrono::UTC;

use handlebars_iron::Template;

use iron::{Handler, status};
use iron::prelude::*;
use iron::headers::ContentType;
use iron::mime::{Mime, SubLevel, TopLevel};

use rustc_serialize::json::{Json, ToJson};

use url;

use Result;
use cam;
use heartbeat::{HeartbeatV1, expected_next_scan_time};

/// The main page for the atlas status site, http://atlas.lidar.io.
#[derive(Debug)]
pub struct IndexHandler {
    heartbeats: Arc<RwLock<Vec<HeartbeatV1>>>,
    img_directory: cam::Storage,
    url: url::Url,
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
    /// # use std::sync::{Arc, RwLock};
    /// # use atlas::server::IndexHandler;
    /// let heartbeats = Arc::new(RwLock::new(Vec::new()));
    /// let handler = IndexHandler::new(heartbeats, "data", "http://iridiumcam.lidar.io").unwrap();
    /// ```
    pub fn new(heartbeats: Arc<RwLock<Vec<HeartbeatV1>>>,
               img_dir: &str,
               img_url: &str)
               -> Result<IndexHandler> {
        Ok(IndexHandler {
            heartbeats: heartbeats,
            img_directory: cam::Storage::new(img_dir),
            url: try!(url::Url::parse(img_url)),
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
                    itry!(cam::datetime_from_path(file_name)).to_string().to_json());
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
