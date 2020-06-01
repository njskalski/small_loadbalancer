use rand::Rng;
use std::cell::Cell;

#[derive(Debug, Copy, Clone)]
pub enum Algorithm {
    RANDOM,
    ROUND_ROBIN
}

#[derive(Debug, Clone)]
pub struct Instance {
    pub hostname : String,
    pub port : u16
}

impl Instance {
    pub fn new(hostname : &str, port : u16) -> Self {
        Instance {
            hostname : hostname.to_string(), port
        }
    }
}

#[derive(Debug)]
pub struct LoadBalancerState {
    instances : Vec<Instance>,
    algorithm : Algorithm,
    round_robin_idx : usize,
}

impl LoadBalancerState {
    pub fn new(instances : Vec<Instance>, algorithm : Algorithm) -> Self {
        LoadBalancerState{ instances, algorithm, round_robin_idx : 0 }
    }

    pub fn get_next_instance(&mut self) -> Instance {
        match self.algorithm {
            Algorithm::RANDOM => {
                let mut rng = rand::thread_rng();
                let idx = rng.gen_range(0, self.instances.len());
                self.instances[idx].clone()
            },
            Algorithm::ROUND_ROBIN => {
                let idx = self.round_robin_idx;
                let next_idx = if idx + 1 < self.instances.len() { idx + 1 } else { 0 };
                self.round_robin_idx = next_idx;

                self.instances[idx].clone()
            }
        }
    }
}