extern crate atlas;
extern crate handlebars_iron;
extern crate iron;
extern crate mount;
extern crate sbd;
extern crate router;
extern crate rustc_serialize;
extern crate staticfile;

use std::fmt::Write;
use std::collections::BTreeMap;

use atlas::heartbeat::{Heartbeat, IntoHeartbeats, expected_next_scan_time};

use handlebars_iron::{DirectorySource, HandlebarsEngine, Template};

use iron::prelude::*;
use iron::headers::ContentType;
use iron::mime::{Mime, SubLevel, TopLevel};
use iron::status;

use mount::Mount;

use router::Router;

use rustc_serialize::json::{Json, ToJson};

use staticfile::Static;

const ATLAS_IMEI: &'static str = "300234063909200";

fn main() {
    let mut hbse = HandlebarsEngine::new();
    hbse.add(Box::new(DirectorySource::new("templates", ".hbs")));
    hbse.reload().unwrap();

    let mut router = Router::new();
    router.get("/", index);
    router.get("/soc.csv", soc);
    router.get("/temperature.csv", temperature);

    let mut mount = Mount::new();
    mount.mount("/static/", Static::new("static"));
    mount.mount("/", router);

    let mut chain = Chain::new(mount);
    chain.link_after(hbse);
    Iron::new(chain)
        .http("localhost:3000")
        .unwrap();
}

fn index(_: &mut Request) -> IronResult<Response> {
    let heartbeat = heartbeats().pop().unwrap();
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
