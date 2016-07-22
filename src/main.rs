extern crate atlas;
extern crate docopt;
extern crate handlebars_iron;
extern crate iron;
extern crate logger;
extern crate mount;
extern crate notify;
extern crate router;
extern crate rustc_serialize;
extern crate sbd;
extern crate staticfile;

use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::sync::mpsc::channel;
use std::thread;

use atlas::heartbeat::{Heartbeat, IntoHeartbeats, expected_next_scan_time};

use docopt::Docopt;

use handlebars_iron::{DirectorySource, HandlebarsEngine, Template};

use iron::prelude::*;
use iron::Handler;
use iron::headers::ContentType;
use iron::mime::{Mime, SubLevel, TopLevel};
use iron::status;
use iron::typemap::Key;

use logger::Logger;

use mount::Mount;

use notify::{RecommendedWatcher, Watcher};

use router::Router;

use rustc_serialize::json::{Json, ToJson};

use sbd::storage::FilesystemStorage;

use staticfile::Static;

const USAGE: &'static str =
    "
ATLAS command-line utility.

Usage:
    atlas serve <addr> <sbd-dir> [--imei=<string>] \
     [--resource-dir=<dir>]
    atlas (-h | --help)
    atlas --version

Options:
    -h --help               \
     Show this screen.
    --version               Show version.
    --imei=<string>         Set \
     the IMEI number of the transmitting SBD unit [default: 300234063909200].
    \
     --resource-dir=<dir>   Set the root directory for static web resources, e.g. templates and \
     javascript files [default: .].
";

#[derive(Debug, RustcDecodable)]
struct Args {
    cmd_serve: bool,
    arg_addr: String,
    arg_sbd_dir: String,
    flag_imei: String,
    flag_resource_dir: String,
}

#[derive(Copy, Clone)]
struct Heartbeats;

impl Key for Heartbeats {
    type Value = Vec<Heartbeat>;
}

fn main() {
    let args: Args = Docopt::new(USAGE)
        .map(|d| d.version(option_env!("CARGO_PKG_VERSION").map(|s| s.to_string())))
        .and_then(|d| d.decode())
        .unwrap_or_else(|e| e.exit());

    let heartbeats = Arc::new(RwLock::new(Vec::new()));

    let mut watcher = HeartbeatWatcher::new(&args.arg_sbd_dir, &args.flag_imei, heartbeats.clone());
    watcher.fill();
    thread::spawn(move || watcher.watch());

    let resource_path = PathBuf::from(args.flag_resource_dir);

    let mut hbse = HandlebarsEngine::new();
    let mut template_path = resource_path.clone();
    template_path.push("templates");
    hbse.add(Box::new(DirectorySource::new(template_path.to_str().unwrap(), ".hbs")));
    hbse.reload().unwrap();

    let mut router = Router::new();
    router.get("/", IndexHandler::new(heartbeats.clone()));
    router.get("/soc.csv",
               CsvHandler::new(heartbeats.clone(),
                               "Battery #1,Battery #2",
                               |mut csv, heartbeat| {
                                   write!(csv,
                                          "{:.2},{:.2}",
                                          100.0 * heartbeat.soc1 / 5.0,
                                          100.0 * heartbeat.soc2 / 5.0)
                               }));
    router.get("/temperature.csv",
               CsvHandler::new(heartbeats.clone(), "External,Mount", |mut csv, heartbeat| {
                   write!(csv,
                          "{:.2},{:.2}",
                          heartbeat.temperature_external,
                          heartbeat.temperature_mount)
               }));

    let mut mount = Mount::new();
    let mut static_path = resource_path.clone();
    static_path.push("static");
    mount.mount("/static/", Static::new(static_path));
    mount.mount("/", router);

    let logger = Logger::new(None);
    let mut chain = Chain::new(mount);
    chain.link_after(hbse);
    chain.link(logger);
    Iron::new(chain)
        .http(args.arg_addr.as_str())
        .unwrap();
}

struct HeartbeatWatcher {
    directory: String,
    imei: String,
    heartbeats: Arc<RwLock<Vec<Heartbeat>>>,
}

impl HeartbeatWatcher {
    fn new(directory: &str,
           imei: &str,
           heartbeats: Arc<RwLock<Vec<Heartbeat>>>)
           -> HeartbeatWatcher {
        HeartbeatWatcher {
            directory: directory.to_string(),
            imei: imei.to_string(),
            heartbeats: heartbeats,
        }
    }

    fn fill(&mut self) {
        let storage = FilesystemStorage::open(&self.directory).unwrap();
        let mut messages: Vec<_> = storage.iter().map(|r| r.unwrap()).collect();
        messages.retain(|m| m.imei() == self.imei);
        messages.sort();
        let mut heartbeats = self.heartbeats.write().unwrap();
        heartbeats.clear();
        heartbeats.extend(messages.into_heartbeats()
            .unwrap()
            .into_iter()
            .filter_map(|h| h.ok()))
    }

    fn watch(&mut self) {
        let (tx, rx) = channel();
        let mut watcher: RecommendedWatcher = Watcher::new(tx).unwrap();
        watcher.watch(&self.directory).unwrap();
        loop {
            match rx.recv() {
                Ok(notify::Event { path: Some(_), op: Ok(_) }) => {
                    println!("Refilling!");
                    self.fill();
                }
                Err(e) => println!("Error yo! {}", e),
                _ => (),
            }
            while let Ok(_) = rx.try_recv() {
                // pass, clear out the buffer
            }
        }
    }
}

struct IndexHandler {
    heartbeats: Arc<RwLock<Vec<Heartbeat>>>,
}

impl IndexHandler {
    fn new(heartbeats: Arc<RwLock<Vec<Heartbeat>>>) -> IndexHandler {
        IndexHandler { heartbeats: heartbeats }
    }
}

impl Handler for IndexHandler {
    fn handle(&self, _: &mut Request) -> IronResult<Response> {
        let heartbeats = self.heartbeats.read().unwrap();
        let heartbeat = heartbeats.last().unwrap();
        let mut data = BTreeMap::<String, Json>::new();
        data.insert("last_heartbeat".to_string(),
                    heartbeat.messages.first().unwrap().time_of_session().to_string().to_json());
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

        let mut response = Response::new();
        response.set_mut(Template::new("index", data)).set_mut(status::Ok);
        Ok(response)
    }
}

struct CsvHandler<F>
    where F: Fn(&mut String, &Heartbeat) -> std::fmt::Result
{
    heartbeats: Arc<RwLock<Vec<Heartbeat>>>,
    header: String,
    func: F,
}

impl<F> CsvHandler<F>
    where F: Fn(&mut String, &Heartbeat) -> std::fmt::Result
{
    fn new(heartbeats: Arc<RwLock<Vec<Heartbeat>>>, header: &str, func: F) -> CsvHandler<F> {
        CsvHandler {
            heartbeats: heartbeats,
            header: format!("Datetime,{}", header),
            func: func,
        }
    }
}

impl<F: 'static> Handler for CsvHandler<F>
    where F: Send + Sync + Fn(&mut String, &Heartbeat) -> std::fmt::Result
{
    fn handle(&self, _: &mut Request) -> IronResult<Response> {
        let mut response = Response::new();
        response.status = Some(status::Ok);
        response.headers
            .set(ContentType(Mime(TopLevel::Text, SubLevel::Ext("csv".to_string()), vec![])));
        let mut csv = String::new();
        writeln!(&mut csv, "{}", self.header).unwrap();

        let heartbeats = self.heartbeats.read().unwrap();
        for heartbeat in heartbeats.iter() {
            write!(&mut csv,
                   "{},",
                   heartbeat.messages.first().unwrap().time_of_session())
                .unwrap();
            let ref func = self.func;
            func(&mut csv, &heartbeat).unwrap();
            writeln!(&mut csv, "").unwrap();
        }
        response.body = Some(Box::new(csv));
        Ok(response)
    }
}
