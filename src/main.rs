extern crate atlas;
extern crate iron;
extern crate mount;
extern crate sbd;
extern crate router;
extern crate staticfile;

use std::fmt::Write;

use atlas::heartbeat::{Heartbeat, IntoHeartbeats};

use iron::prelude::*;
use iron::headers::ContentType;
use iron::mime::{Mime, SubLevel, TopLevel};
use iron::status;

use mount::Mount;

use router::Router;

use staticfile::Static;

const ATLAS_IMEI: &'static str = "300234063909200";

fn main() {
    let mut router = Router::new();
    router.get("/", Static::new("html"));
    router.get("/soc.csv", soc);
    router.get("/temperature.csv", temperature);
    let mut mount = Mount::new();
    mount.mount("/static/", Static::new("static"));
    mount.mount("/", router);
    Iron::new(mount)
        .http("localhost:3000")
        .unwrap();
}

fn heartbeats() -> Vec<Heartbeat> {
    let storage = sbd::storage::FilesystemStorage::open("/Users/gadomski/iridium").unwrap();
    let mut messages: Vec<_> = storage.iter().map(|r| r.unwrap()).collect();
    messages.retain(|m| m.imei() == ATLAS_IMEI);
    messages.sort();
    messages.into_heartbeats()
        .unwrap()
        .into_iter()
        .filter_map(|h| h.ok())
        .collect::<Vec<_>>()
}

fn csv_response(csv: String) -> IronResult<Response> {
    let mut response = Response::new();
    response.status = Some(status::Ok);
    response.headers
        .set(ContentType(Mime(TopLevel::Text, SubLevel::Ext("csv".to_string()), vec![])));
    response.body = Some(Box::new(csv));
    Ok(response)
}

fn temperature(_: &mut Request) -> IronResult<Response> {
    let heartbeats = heartbeats();
    let mut csv = String::new();
    writeln!(&mut csv, "Datetime,External,Mount").unwrap();
    for heartbeat in heartbeats {
        writeln!(&mut csv,
                 "{},{:.2},{:.2}",
                 heartbeat.messages.first().unwrap().time_of_session(),
                 heartbeat.temperature_external,
                 heartbeat.temperature_mount,)
            .unwrap();
    }
    csv_response(csv)
}

fn soc(_: &mut Request) -> IronResult<Response> {
    let heartbeats = heartbeats();
    let mut csv = String::new();
    writeln!(&mut csv, "Datetime,Battery #1,Battery #2").unwrap();
    for heartbeat in heartbeats {
        writeln!(&mut csv,
                 "{},{:.2},{:.2}",
                 heartbeat.messages.first().unwrap().time_of_session(),
                 100.0 * heartbeat.soc1 / 5.0,
                 100.0 * heartbeat.soc2 / 5.0)
            .unwrap();
    }
    csv_response(csv)
}
