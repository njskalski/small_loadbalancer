#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate clap;
#[macro_use]
extern crate rocket;
use std::process;
use std::process::exit;

use rocket::config::{Config, Environment};

#[get("/get")]
fn get() -> String {
    format!("Provider#{}", process::id())
}

#[get("/check")]
fn check() -> &'static str {
    "OK"
}

fn main() {
    let yml = clap::load_yaml!("provider_clap.yaml");
    let app = clap::App::from_yaml(yml)
        .author("NJ Skalski <gitstuff@s5i.ch>")
        .long_version(crate_version!());

    let matches = app.get_matches();
    let port_str = matches.value_of("port").unwrap();

    let port: u16 = match port_str.parse::<u16>() {
        Ok(port_int) => port_int,
        _ => {
            println!(
                "Port must be unsigned 16 bit integer, instead \"{}\"",
                port_str
            );
            exit(1);
        }
    };

    let config = match Config::build(Environment::Staging).port(port).finalize() {
        Ok(config) => config,
        Err(e) => {
            println!("Failed to build config, because: \"{}\"", e);
            exit(2);
        }
    };

    rocket::custom(config)
        .mount("/", routes![get, check])
        .launch();
}
