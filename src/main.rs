extern crate atlas;
extern crate iron;
extern crate mount;
extern crate sbd;
extern crate router;
extern crate staticfile;

use std::fmt::Write;

use atlas::heartbeat::IntoHeartbeats;

use iron::prelude::*;
use iron::headers::ContentType;
use iron::mime::{Mime, SubLevel, TopLevel};
use iron::status;

use mount::Mount;

use router::Router;

use staticfile::Static;

fn main() {
    let mut router = Router::new();
    router.get("/", Static::new("html"));
    router.get("/temperature.csv", temperature);
    let mut mount = Mount::new();
    mount.mount("/static/", Static::new("static"));
    mount.mount("/", router);
    Iron::new(mount)
        .http("localhost:3000")
        .unwrap();
}

fn temperature(_: &mut Request) -> IronResult<Response> {
    let storage = sbd::storage::FilesystemStorage::open("/Users/gadomski/iridium").unwrap();
    let mut messages: Vec<_> = storage.iter().map(|r| r.unwrap()).collect();
    messages.retain(|m| m.imei() == "300234063909200");
    messages.sort();
    let heartbeats = messages.into_heartbeats()
        .unwrap()
        .into_iter()
        .filter_map(|h| h.ok())
        .collect::<Vec<_>>();
    let mut response = Response::new();
    response.status = Some(status::Ok);
    response.headers
        .set(ContentType(Mime(TopLevel::Text, SubLevel::Ext("csv".to_string()), vec![])));
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
    response.body = Some(Box::new(csv));
    Ok(response)
}
