use rand::Rng;
use rocket::fairing::{Fairing, Info, Kind};
use rocket::{Request, Data, Response};
use std::sync::atomic::Ordering;
use std::sync::atomic::AtomicU32;

#[derive(Debug, Copy, Clone)]
pub enum Algorithm {
    RANDOM,
    ROUND_ROBIN,
}

#[derive(Debug, Clone)]
pub struct Instance {
    pub hostname: String,
    pub port: u16,
}

impl Instance {
    pub fn new(hostname: &str, port: u16) -> Self {
        Instance {
            hostname: hostname.to_string(),
            port,
        }
    }
}

#[derive(Debug)]
pub struct LoadBalancerState {
    instances: Vec<Instance>,
    is_enabled : Vec<bool>,
    consecutive_health_ok : Vec<u8>, // I store history of up to 255 health checks.
    algorithm: Algorithm,
    round_robin_idx: usize,
    per_instance_capacity : u32
}

impl LoadBalancerState {
    pub fn new(instances: Vec<Instance>, algorithm: Algorithm, per_instance_capacity : u32) -> Self {
        let is_enabled : Vec<bool> = instances.iter().map(|_| true).collect();
        let consecutive_health_ok : Vec<u8> = instances.iter().map( |_| 0).collect();

        LoadBalancerState {
            instances,
            is_enabled,
            consecutive_health_ok,
            algorithm,
            round_robin_idx: 0,
            per_instance_capacity
        }
    }

    pub fn current_capacity(&self) -> u32 {
        let mut result : u32 = 0;
        for it in self.is_enabled.iter() {
            if *it {
                result += 1;
            }
        }

        result *= self.per_instance_capacity;
        result
    }

    pub fn instances(&self) -> &Vec<Instance> {
        &self.instances
    }

    pub fn get_next_instance(&mut self) -> Option<Instance> {
        let mut is_any_enabled : bool = false;

        for value in self.is_enabled.iter() {
            if *value {
                is_any_enabled = true;
                break;
            }
        }

        if !is_any_enabled {
            return None;
        }

        match self.algorithm {
            Algorithm::RANDOM => {
                let mut enabled_indices : Vec<usize> = Vec::new();

                for (idx, is_enabled) in self.is_enabled.iter().enumerate() {
                    if *is_enabled {
                        enabled_indices.push(idx);
                    }
                }

                // I know indices are non-empty since I checked it above.

                let mut rng = rand::thread_rng();
                let idx_idx = rng.gen_range(0, enabled_indices.len());
                let idx = enabled_indices[idx_idx];

                Some(self.instances[idx].clone())
            }
            Algorithm::ROUND_ROBIN => {
                // This loop will terminate since at least one field in is_enabled is true.
                loop {
                    let idx = self.round_robin_idx;
                    let next_idx = if idx + 1 < self.instances.len() {
                        idx + 1
                    } else {
                        0
                    };
                    self.round_robin_idx = next_idx;

                    if self.is_enabled[idx] {
                        return Some(self.instances[idx].clone())
                    }
                }
            }
        }
    }

    pub fn report_healthcheck_results(&mut self, results: Vec<bool>) {
        assert_eq!(results.len(), self.instances.len());
        println!("health check results : {:?}", results);

        for (idx, health_ok) in results.iter().enumerate() {
            if *health_ok {
                // do not increment above maximum value.
                if self.consecutive_health_ok[idx] < std::u8::MAX {
                    self.consecutive_health_ok[idx] += 1;
                }

                if self.consecutive_health_ok[idx] >= 2 {
                    // Re-include provider after two successful heartbeat checks.
                    self.set_enabled(idx, true);
                }
            } else {
                self.consecutive_health_ok[idx] = 0;

                self.set_enabled(idx, false);
            }
        }
    }

    pub fn set_enabled(&mut self, idx : usize, enabled : bool) {
        assert!(idx < self.instances.len());
        self.is_enabled[idx] = enabled;
    }
}

#[derive(Debug)]
pub struct RequestCounter {
    num_request : AtomicU32
}

impl RequestCounter {
    pub fn new() -> Self {
        RequestCounter{
            num_request : AtomicU32::new(0)
        }
    }

    pub fn get_num_requests(&self) -> u32 {
        self.num_request.load(Ordering::SeqCst)
    }
}

impl Fairing for RequestCounter {
    fn info(&self) -> Info {
        Info {
            name: "Request Counter",
            kind: Kind::Request | Kind::Response
        }
    }

    fn on_request(&self, _request: &mut Request, _: &Data) {
        self.num_request.fetch_add(1, Ordering::SeqCst);
    }

    fn on_response(&self, _request: &Request, _response: &mut Response) {
        self.num_request.fetch_sub(1, Ordering::SeqCst);
    }
}