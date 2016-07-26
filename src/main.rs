extern crate atlas;
extern crate docopt;
extern crate env_logger;
extern crate handlebars_iron;
extern crate iron;
extern crate logger;
extern crate mount;
extern crate router;
extern crate rustc_serialize;
extern crate staticfile;

use std::path::PathBuf;
use std::thread;

use atlas::server::{CsvHandler, IndexHandler};
use atlas::watch::HeartbeatWatcher;
use docopt::Docopt;
use handlebars_iron::{DirectorySource, HandlebarsEngine};
use iron::prelude::*;
use logger::Logger;
use mount::Mount;
use router::Router;
use staticfile::Static;

const USAGE: &'static str =
    "
ATLAS command-line utility.

Usage:
    atlas serve <addr> <sbd-dir> <img-dir> \
     [--imei=<string>] [--resource-dir=<dir>] [--img-url=<url>]
    atlas (-h | --help)
    atlas \
     --version

Options:
    -h --help               Show this screen.
    --version               \
     Show version.
    --imei=<string>         The IMEI number of the transmitting SBD unit \
     [default: 300234063909200].
    --resource-dir=<dir>   The root directory for static web \
     resources, e.g. templates and javascript files [default: .].
     --img-url=<url>       The \
     url (server + path) that can serve up ATLAS images [default: \
     http://iridiumcam.lidar.io/ATLAS_CAM].
";

#[derive(Debug, RustcDecodable)]
struct Args {
    cmd_serve: bool,
    arg_addr: String,
    arg_sbd_dir: String,
    arg_img_dir: String,
    flag_imei: String,
    flag_resource_dir: String,
    flag_img_url: String,
}

fn main() {
    env_logger::init().unwrap();

    let args: Args = Docopt::new(USAGE)
        .map(|d| d.version(option_env!("CARGO_PKG_VERSION").map(|s| s.to_string())))
        .and_then(|d| d.decode())
        .unwrap_or_else(|e| e.exit());

    let mut watcher = HeartbeatWatcher::new(&args.arg_sbd_dir, &args.flag_imei).unwrap();

    let resource_path = PathBuf::from(args.flag_resource_dir);

    let mut hbse = HandlebarsEngine::new();
    let mut template_path = resource_path.clone();
    template_path.push("templates");
    hbse.add(Box::new(DirectorySource::new(template_path.to_str().unwrap(), ".hbs")));
    hbse.reload().unwrap();

    let mut router = Router::new();
    router.get("/",
               IndexHandler::new(watcher.heartbeats(), &args.arg_img_dir, &args.flag_img_url)
                   .unwrap());
    router.get("/soc.csv",
               CsvHandler::new(watcher.heartbeats(),
                               &vec!["Battery #1", "Battery #2"],
                               |heartbeat| {
                                   vec![format!("{:.2}", 100.0 * heartbeat.soc1 / 5.0),
                                   format!("{:.2}", 100.0 * heartbeat.soc2 / 5.0),]
                               }));
    router.get("/temperature.csv",
               CsvHandler::new(watcher.heartbeats(),
                               &vec!["External", "Mount"],
                               |heartbeat| {
                                   vec![format!("{:.2}", heartbeat.temperature_external),
                                        format!("{:.2}", heartbeat.temperature_mount)]
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

    thread::spawn(move || watcher.watch().unwrap());
    Iron::new(chain)
        .http(args.arg_addr.as_str())
        .unwrap();
}
