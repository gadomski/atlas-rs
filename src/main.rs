extern crate atlas;
extern crate docopt;
extern crate chrono;
extern crate env_logger;
extern crate rustc_serialize;

#[cfg(feature = "magick_rust")]
use std::io::Write;

use atlas::cam::Camera;
use atlas::server::Server;
use docopt::Docopt;
#[cfg(feature = "magick_rust")]
use atlas::magick::GifMaker;

const USAGE: &'static str =
    "
ATLAS command-line utility.

Usage:
    atlas serve <config-file>
    atlas gif <img-dir> [--gif-days=<n>] [--gif-delay=<n>] [--gif-width=<n>] [--gif-height=<n>]
    atlas (-h | --help)
    atlas --version

Options:
    -h --help               Show this screen.
    --version               Show version.
     --gif-days=<n>         The number of days to combine into a gif [default: 7].
     --gif-delay=<n>        The number of milliseconds between gif frames [default: 500].
     --gif-width=<n>        The width of the gif [default: 256].
     --gif-height=<n>       The height of the gif [default: 192].
";

#[derive(Debug, RustcDecodable)]
struct Args {
    cmd_serve: bool,
    cmd_gif: bool,
    arg_img_dir: String,
    arg_config_file: String,
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
    // FIXME adapt to other cameras
    let gif_maker = GifMaker::new(Camera::new("HEL_ATLAS", args.arg_img_dir).unwrap(),
                                  args.flag_gif_width,
                                  args.flag_gif_height);
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
    let mut server = Server::new(args.arg_config_file).unwrap();
    server.serve().unwrap().unwrap();
}
