//! Tools for serving data via `iron`.

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
use heartbeat::{Heartbeat, expected_next_scan_time};

/// Return the main index page for the atlas status site.
#[derive(Debug)]
pub struct IndexHandler {
    heartbeats: Arc<RwLock<Vec<Heartbeat>>>,
    img_directory: cam::Directory,
    url: url::Url,
}

impl IndexHandler {
    /// Creates a new index handler for the given heartbeats, images, and image url.
    pub fn new(heartbeats: Arc<RwLock<Vec<Heartbeat>>>,
               img_dir: &str,
               img_url: &str)
               -> Result<IndexHandler> {
        Ok(IndexHandler {
            heartbeats: heartbeats,
            img_directory: cam::Directory::new(img_dir),
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
                    format!("{:.1}", heartbeat.temperature_external).to_json());
        data.insert("temperature_mount".to_string(),
                    format!("{:.1}", heartbeat.temperature_mount).to_json());
        data.insert("pressure".to_string(),
                    format!("{:.1}", heartbeat.pressure).to_json());
        data.insert("humidity".to_string(),
                    format!("{:.1}", heartbeat.humidity).to_json());
        data.insert("soc1".to_string(),
                    format!("{:.1}", 100.0 * heartbeat.soc1 / 5.0).to_json());
        data.insert("soc2".to_string(),
                    format!("{:.1}", 100.0 * heartbeat.soc2 / 5.0).to_json());
        let mut url = self.url.clone();
        let (filename, datetime) = itry!(self.img_directory.latest()).unwrap();
        url.path_segments_mut()
            .unwrap()
            .push(&filename.to_string_lossy());
        data.insert("latest_image_url".to_string(),
                    url.as_str()
                        .to_json());
        data.insert("latest_image_datetime".to_string(),
                    datetime.to_string().to_json());
        data.insert("now".to_string(),
                    format!("{}", UTC::now().format("%Y-%m-%d %H:%M:%S UTC")).to_json());

        let mut response = Response::new();
        response.set_mut(Template::new("index", data)).set_mut(status::Ok);
        Ok(response)
    }
}

/// Returns a csv file as a http response.
#[derive(Debug)]
pub struct CsvHandler<F>
    where F: Fn(&Heartbeat) -> Vec<String>
{
    heartbeats: Arc<RwLock<Vec<Heartbeat>>>,
    header: Vec<String>,
    func: F,
}

impl<F> CsvHandler<F>
    where F: Fn(&Heartbeat) -> Vec<String>
{
    /// Creates a new csv handler that uses the function to build csv results.
    pub fn new(heartbeats: Arc<RwLock<Vec<Heartbeat>>>,
               header_extra: &Vec<&str>,
               func: F)
               -> CsvHandler<F> {
        let mut header = vec!["Datetime".to_string()];
        header.extend(header_extra.iter().map(|s| s.to_string()));
        CsvHandler {
            heartbeats: heartbeats,
            header: header,
            func: func,
        }
    }
}

impl<F: 'static> Handler for CsvHandler<F>
    where F: Send + Sync + Fn(&Heartbeat) -> Vec<String>
{
    fn handle(&self, _: &mut Request) -> IronResult<Response> {
        let mut response = Response::new();
        response.status = Some(status::Ok);
        response.headers
            .set(ContentType(Mime(TopLevel::Text, SubLevel::Ext("csv".to_string()), vec![])));
        let mut csv = String::new();
        writeln!(&mut csv, "{}", self.header.join(",")).unwrap();

        let heartbeats = self.heartbeats.read().unwrap();
        for heartbeat in heartbeats.iter() {
            write!(&mut csv,
                   "{},",
                   heartbeat.messages.first().unwrap().time_of_session())
                .unwrap();
            let ref func = self.func;
            let fields = func(&heartbeat);
            writeln!(&mut csv, "{}", fields.join(",")).unwrap();
        }
        response.body = Some(Box::new(csv));
        Ok(response)
    }
}
