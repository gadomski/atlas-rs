extern crate atlas;
extern crate chrono;
extern crate docopt;
extern crate env_logger;
extern crate handlebars_iron;
extern crate iron;
extern crate logger;
extern crate mount;
extern crate router;
extern crate rustc_serialize;
extern crate staticfile;

#[cfg(feature = "magick_rust")]
use std::io::Write;
use std::path::PathBuf;
use std::thread;

use atlas::server::{CsvHandler, IndexHandler, SocCsvProvider, TemperatureCsvProvider};
use atlas::watch::{DirectoryWatcher, HeartbeatWatcher};
use docopt::Docopt;
use handlebars_iron::{DirectorySource, HandlebarsEngine};
use iron::prelude::*;
use mount::Mount;
use router::Router;
use staticfile::Static;
#[cfg(feature = "magick_rust")]
use atlas::magick::{GifHandler, GifMaker, GifWatcher};

const USAGE: &'static str =
    "
ATLAS command-line utility.

Usage:
    atlas serve <addr> <sbd-dir> <img-dir> \
     [--imei=<string>] [--resource-dir=<dir>] [--img-url=<url>] [--color-logs] [--gif-days=<n>] \
     [--gif-delay=<n>] [--gif-width=<n>] [--gif-height=<n>]
    atlas gif <img-dir> [--gif-days=<n>] [--gif-delay=<n>] [--gif-width=<n>] [--gif-height=<n>]
    atlas (-h | --help)
    atlas --version

Options:
    -h --help               \
     Show this screen.
    --version               Show version.
    --imei=<string>         The \
     IMEI number of the transmitting SBD unit [default: 300234063909200].
    \
     --resource-dir=<dir>   The root directory for static web resources, e.g. templates and \
     javascript files [default: .].
     --img-url=<url>       The url (server + path) that can \
     serve up ATLAS images [default: http://iridiumcam.lidar.io/ATLAS_CAM].
     --color-logs          \
     HTTP logs are printed in color.
     --gif-days=<n>        The number of days to combine into a gif [default: 7].
     --gif-delay=<n>       The number of milliseconds between gif frames [default: 500].
     --gif-width=<n>       The width of the gif [default: 256].
     --gif-height=<n>      The height of the gif [default: 192].
";

#[derive(Debug, RustcDecodable)]
struct Args {
    cmd_serve: bool,
    cmd_gif: bool,
    arg_addr: String,
    arg_sbd_dir: String,
    arg_img_dir: String,
    flag_imei: String,
    flag_resource_dir: String,
    flag_img_url: String,
    flag_color_logs: bool,
    flag_gif_days: i64,
    flag_gif_delay: i64,
    flag_gif_width: u64,
    flag_gif_height: u64,
}

fn main() {
    env_logger::init().unwrap();

    let args: Args = Docopt::new(USAGE)
        .map(|d| d.version(option_env!("CARGO_PKG_VERSION").map(|s| s.to_string())))
        .and_then(|d| d.decode())
        .unwrap_or_else(|e| e.exit());

    if args.cmd_serve {
        serve(args);
    } else if args.cmd_gif {
        gif(args);
    }
}

#[cfg(feature = "magick_rust")]
fn gif(args: Args) {
    let gif_maker = GifMaker::new(args.arg_img_dir, args.flag_gif_width, args.flag_gif_height);
    let gif = gif_maker.since(&(chrono::UTC::now() - chrono::Duration::days(args.flag_gif_days)),
               chrono::Duration::milliseconds(args.flag_gif_delay))
        .unwrap();
    std::io::stdout().write(&gif).unwrap();
}

#[cfg(not(feature = "magick_rust"))]
fn gif(_: Args) {
    println!("ERROR: atlas not built with ImageMagick, cannot create gif");
    std::process::exit(1);
}

fn serve(args: Args) {
    let mut heartbeat_watcher = HeartbeatWatcher::new(&args.arg_sbd_dir, &args.flag_imei);

    let resource_path = PathBuf::from(&args.flag_resource_dir);

    let mut hbse = HandlebarsEngine::new();
    let mut template_path = resource_path.clone();
    template_path.push("templates");
    hbse.add(Box::new(DirectorySource::new(template_path.to_str().unwrap(), ".hbs")));
    hbse.reload().unwrap();

    let mut router = Router::new();
    router.get("/",
               IndexHandler::new(heartbeat_watcher.heartbeats(),
                                 &args.arg_img_dir,
                                 &args.flag_img_url)
                   .unwrap());
    router.get("/soc.csv",
               CsvHandler::new(heartbeat_watcher.heartbeats(), SocCsvProvider));
    router.get("/temperature.csv",
               CsvHandler::new(heartbeat_watcher.heartbeats(), TemperatureCsvProvider));


    add_gif_handler(&args, &mut router);
    #[cfg(feature = "magick_rust")]
    fn add_gif_handler(args: &Args, router: &mut Router) {
        let mut gif_watcher = GifWatcher::new(&args.arg_img_dir,
                                              chrono::Duration::days(args.flag_gif_days),
                                              chrono::Duration::milliseconds(args.flag_gif_delay),
                                              args.flag_gif_width,
                                              args.flag_gif_height);
        router.get("/atlas-cam.gif", GifHandler::new(gif_watcher.gif()));
        thread::spawn(move || {
            gif_watcher.refresh().unwrap();
            gif_watcher.watch().unwrap();
        });
    };
    #[cfg(not(feature = "magick_rust"))]
    fn add_gif_handler(_: &Args, _: &mut Router) {};

    let mut mount = Mount::new();
    let mut static_path = resource_path.clone();
    static_path.push("static");
    mount.mount("/static/", Static::new(static_path));
    mount.mount("/", router);

    let format = if args.flag_color_logs {
        None
    } else {
        logger::format::Format::new("{method} {uri} -> {status} ({response-time})",
                                    vec![],
                                    vec![])
    };
    let logger = logger::Logger::new(format);
    let mut chain = Chain::new(mount);
    chain.link_after(hbse);
    chain.link(logger);

    thread::spawn(move || {
        heartbeat_watcher.refresh().unwrap();
        heartbeat_watcher.watch().unwrap();
    });

    Iron::new(chain)
        .http(args.arg_addr.as_str())
        .unwrap();
}
