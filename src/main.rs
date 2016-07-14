extern crate atlas;
extern crate iron;
extern crate rustc_serialize;
extern crate sbd;

use std::fmt::Write;

use atlas::heartbeat::IntoHeartbeats;

use iron::prelude::*;
use iron::headers::ContentType;
use iron::mime::{Mime, SubLevel, TopLevel};
use iron::status;

fn main() {
    Iron::new(heartbeats)
        .http("localhost:3000")
        .unwrap();
}

fn heartbeats(_: &mut Request) -> IronResult<Response> {
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
    writeln!(&mut csv,
             "datetime,temperature_external,pressure,humidity,scan_start_datetime,\
              temperature_mount,soc1,soc2")
        .unwrap();
    for heartbeat in heartbeats {
        writeln!(&mut csv,
                 "{},{:.2},{:.2},{:.2},{},{:.2},{:.2},{:.2}",
                 heartbeat.messages.first().unwrap().time_of_session(),
                 heartbeat.temperature_external,
                 heartbeat.pressure,
                 heartbeat.humidity,
                 heartbeat.scan_start_datetime,
                 heartbeat.temperature_mount,
                 heartbeat.soc1,
                 heartbeat.soc2)
            .unwrap();
    }
    response.body = Some(Box::new(csv));
    Ok(response)
}
