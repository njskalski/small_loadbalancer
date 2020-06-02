#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate clap;
#[macro_use]
extern crate rocket;

use rocket::config::{Config, Environment};
use rocket::State;
use std::process::exit;

mod load_balancer_types;
use crate::load_balancer_types::Algorithm::{RANDOM, ROUND_ROBIN};
use crate::load_balancer_types::{Algorithm, RequestCounter};
use load_balancer_types::{Instance, LoadBalancerState};

use reqwest::blocking::Response;
use rocket::http::Status;
use std::sync::{Arc, RwLock};
use std::time::Duration;

const MAX_INSTANCES: usize = 10;

type LoadBalancerStateType = Arc<RwLock<LoadBalancerState>>;

fn forward_request(instance: &Instance) -> Result<String, Box<dyn std::error::Error>> {
    let address = format!("http://{}:{}/get", instance.hostname, instance.port);
    let resp: Response = reqwest::blocking::get(&address)?;
    let resp_as_text: String = resp.text()?;

    Ok(format!("got {}", resp_as_text))
}

fn check_instance_health(instance: &Instance) -> bool {
    let address = format!("http://{}:{}/get", instance.hostname, instance.port);
    let resp = reqwest::blocking::get(&address);

    match resp {
        Ok(_) => true,
        Err(_) => false,
    }
}

#[get("/get")]
fn get(
    state_lock: State<LoadBalancerStateType>,
    request_counter: State<Arc<RequestCounter>>,
) -> Result<String, Status> {
    let instance_op: Option<Instance> = {
        let mut state = state_lock.write().unwrap();

        if request_counter.get_num_requests() > state.current_capacity() {
            eprintln!("Balancer capacity reached.");
            return Err(Status::new(503, "Capacity limit reached"));
        } else {
            eprintln!(
                "{} / {} open requests.",
                request_counter.get_num_requests(),
                state.current_capacity()
            );
        }

        state.get_next_instance()
    };

    match instance_op {
        None => {
            eprintln!("No instances available.");
            Err(Status::new(500, "No instances available"))
        }
        Some(instance) => {
            let answer = forward_request(&instance);

            match answer {
                Err(e) => {
                    eprintln!("Failure receiving answer from instance, error \"{}\"", e);
                    Err(Status::new(500, "Backend instance failed."))
                }
                Ok(s) => Ok(s),
            }
        }
    }
}

#[get("/state")]
fn get_state(state_lock: State<LoadBalancerStateType>) -> String {
    let state_r = state_lock.read().unwrap();
    format!("{:#?}", *state_r)
}

#[get("/include/<idx>")]
fn include(state_lock: State<LoadBalancerStateType>, idx: usize) -> String {
    let instances: Vec<Instance> = {
        let state = state_lock.read().unwrap();
        state.instances().clone()
    };

    if idx > instances.len() {
        return format!(
            "Incorrect index {}, tracking only {} instances.",
            idx,
            instances.len()
        )
        .to_string();
    }

    let instance = &instances[idx];
    let health = check_instance_health(instance);

    if !health {
        return format!("Unable to include instance #{}, failed health check.", idx,).to_string();
    }

    {
        let mut state = state_lock.write().unwrap();
        state.set_enabled(idx, true);
    }

    "OK".to_string()
}

#[get("/exclude/<idx>")]
fn exclude(state_lock: State<LoadBalancerStateType>, idx: usize) -> String {
    let instance_num: usize = {
        let state = state_lock.read().unwrap();
        state.instances().len()
    };

    if idx > instance_num {
        return format!(
            "Incorrect index {}, tracking only {} instances.",
            idx, instance_num
        )
        .to_string();
    }

    {
        let mut state = state_lock.write().unwrap();
        state.set_enabled(idx, false);
    }

    "OK".to_string()
}

fn main() {
    let yml = clap::load_yaml!("load_balancer_clap.yaml");
    let app = clap::App::from_yaml(yml)
        .author("NJ Skalski <gitstuff@s5i.ch>")
        .long_version(crate_version!());

    let matches = app.get_matches();

    let instances_str = matches.value_of("instances").unwrap();
    let instances_list: Vec<&str> = instances_str.split(",").collect();

    let mut instances: Vec<Instance> = vec![];

    // Parsing instances.
    for si in instances_list.iter() {
        if si.trim().is_empty() {
            continue;
        } // handling tailing coma

        let colon_pos_op = si.find(":");
        match colon_pos_op {
            None => {
                println!(
                    "Cannot divide \"{}\" into a host:port pair, no colon found.",
                    si
                );
                exit(1);
            }
            Some(colon_pos) => {
                let hostname = si[0..colon_pos].trim();
                let port_op = si[colon_pos + 1..].parse::<u16>();
                match port_op {
                    Ok(port) => instances.push(Instance::new(hostname, port)),
                    Err(e) => {
                        println!(
                            "Cannot divide \"{}\" into a host:port, port parse error: \"{}\"",
                            si, e
                        );
                        exit(2);
                    }
                }
            }
        }
    }

    if instances.len() > MAX_INSTANCES {
        println!(
            "Up to {} instances allowed, found {}",
            MAX_INSTANCES,
            instances.len()
        );
        exit(3);
    }

    let port_str = matches.value_of("port").unwrap();

    let port: u16 = match port_str.parse::<u16>() {
        Ok(port_int) => port_int,
        _ => {
            println!(
                "Port must be unsigned 16 bit integer, instead \"{}\"",
                port_str
            );
            exit(4);
        }
    };

    let algorithm: Algorithm = match matches.value_of("algorithm").unwrap() {
        "random" => RANDOM,
        "round_robin" => ROUND_ROBIN,
        other => {
            println!("Unknown algorithm \"{}\"", other);
            exit(5);
        }
    };

    let healthcheck_delay_str = matches.value_of("healthcheck-delay").unwrap();
    let healthcheck_delay_s: u64 = match healthcheck_delay_str.parse::<u64>() {
        Ok(s) => s,
        Err(e) => {
            println!(
                "Failed to parse healthckeck-delay parameter, got \"{}\" and error is \"{}\"",
                healthcheck_delay_str, e
            );
            exit(6)
        }
    };

    let per_instance_limit_str = matches.value_of("provider-capacity").unwrap();
    let per_instance_limit: u32 = match per_instance_limit_str.parse::<u32>() {
        Ok(s) => s,
        Err(e) => {
            println!(
                "Failed to parse provider-capacity parameter, got \"{}\" and error is \"{}\"",
                per_instance_limit_str, e
            );
            exit(6)
        }
    };

    let config = match Config::build(Environment::Staging).port(port).finalize() {
        Ok(config) => config,
        Err(e) => {
            println!("Failed to build config, because: \"{}\"", e);
            exit(5);
        }
    };

    let state: LoadBalancerStateType = Arc::new(RwLock::new(LoadBalancerState::new(
        instances,
        algorithm,
        per_instance_limit,
    )));
    let state2 = state.clone(); // cloning Arc
    let _child = std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(healthcheck_delay_s));

        let instances: Vec<Instance> = {
            let state = state2.read().unwrap();
            state.instances().clone()
        };

        let results: Vec<bool> = instances
            .iter()
            .map(|instance| -> bool {
                let address = format!("http://{}:{}/get", instance.hostname, instance.port);
                let resp = reqwest::blocking::get(&address);

                match resp {
                    Ok(_) => true,
                    Err(_) => false,
                }
            })
            .collect();

        {
            let mut state = state2.write().unwrap();
            state.report_healthcheck_results(results);
        }
    });

    let request_counter_arc = Arc::new(RequestCounter::new());

    rocket::custom(config)
        .manage(state)
        .manage(request_counter_arc.clone())
        .attach(request_counter_arc)
        .mount("/", routes![get, get_state, include, exclude])
        .launch();
}
