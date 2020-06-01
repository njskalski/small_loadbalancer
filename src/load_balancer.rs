#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate clap;
#[macro_use]
extern crate rocket;

use std::process::exit;
use rocket::config::{Config, Environment};
use rocket::{State};

mod load_balancer_types;
use load_balancer_types::{Instance, LoadBalancerState};
use crate::load_balancer_types::Algorithm;
use crate::load_balancer_types::Algorithm::{RANDOM, ROUND_ROBIN};

use reqwest::blocking::Response as Response;
use std::sync::{RwLock, Arc};
use std::borrow::Borrow;
use std::ops::Deref;

const MAX_INSTANCES : usize = 10;

type LoadBalancerStateType = Arc<RwLock<LoadBalancerState>>;

fn forward_request(instance : &Instance) -> Result<String, Box<dyn std::error::Error>> {
    let address = format!("http://{}:{}/get", instance.hostname, instance.port);
    let resp : Response = reqwest::blocking::get(&address)?;
    let resp_as_text : String = resp.text()?;

    Ok(format!("got {}", resp_as_text))
}


#[get("/get")]
fn get(state_lock: State<LoadBalancerStateType>) -> String {
    let instance : Instance = {
        let mut state = state_lock.write().unwrap();
        state.get_next_instance()
    };

    let answer = forward_request(&instance);

    format!("a : {:#?}", answer)
}

#[get("/state")]
fn get_state(state_lock : State<LoadBalancerStateType>) -> String {
    let state_r = state_lock.read().unwrap();
    format!("{:#?}", *state_r)
}

fn main() {
    let yml = clap::load_yaml!("load_balancer_clap.yaml");
    let app = clap::App::from_yaml(yml)
        .author("NJ Skalski <gitstuff@s5i.ch>")
        .long_version(crate_version!());

    let matches = app.get_matches();

    let instances_str = matches.value_of("instances").unwrap();
    let instances_list : Vec<&str> = instances_str.split(",").collect();

    let mut instances : Vec<Instance> = vec![];

    // Parsing instances.
    for si in instances_list.iter() {
        if si.trim().is_empty() { continue; } // handling tailing coma

        let colon_pos_op = si.find(":");
        match colon_pos_op {
            None => {
                println!("Cannot divide \"{}\" into a host:port pair, no colon found.", si);
                exit(1);
            }
            Some(colon_pos) => {

                let hostname = si[0..colon_pos].trim();
                let port_op = si[colon_pos+1..].parse::<u16>();
                match port_op {
                    Ok(port) => instances.push(Instance::new(hostname, port)),
                    Err(e) => {
                        println!("Cannot divide \"{}\" into a host:port, port parse error: \"{}\"", si, e);
                        exit(2);
                    }
                }
            }
        }
    }

    if instances.len() > MAX_INSTANCES {
        println!("Up to {} instances allowed, found {}", MAX_INSTANCES, instances.len());
        exit(3);
    }

    let port_str = matches.value_of("port").unwrap();

    let port : u16 = match port_str.parse::<u16>() {
        Ok(port_int) => port_int,
        _ => {
            println!("Port must be unsigned 16 bit integer, instead \"{}\"", port_str);
            exit(4);
        }
    };

    let algorithm : Algorithm = match matches.value_of("algorithm").unwrap() {
        "random" => RANDOM,
        "round_robin" => ROUND_ROBIN,
        other => {
            println!("Unknown algorithm \"{}\"", other);
            exit(5);
        }
    };

    let config = match Config::build(Environment::Staging)
        .port(port)
        .finalize() {
        Ok(config) => config,
        Err(e) => {
            println!("Failed to build config, because: \"{}\"", e);
            exit(5);
        }
    };

    let state : LoadBalancerStateType = Arc::new(RwLock::new(LoadBalancerState::new(instances, algorithm)));

    // let child = std::thread::spawn( || {
    //     state_ref.write();
    // });

    rocket::custom(config).manage(state).mount("/", routes![get, get_state]).launch();
}