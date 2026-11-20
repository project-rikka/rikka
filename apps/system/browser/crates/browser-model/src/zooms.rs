use std::collections::HashMap;

use browser_pages::host_key;

use crate::ids::ProfileId;

pub const STEPS: [f64; 16] = [
    0.25, 0.33, 0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 4.0,
];
const SAME: f64 = 0.001;

#[derive(Debug, Default)]
pub struct Zooms {
    by_profile: HashMap<ProfileId, HashMap<String, f64>>,
}

impl Zooms {
    pub fn new(by_profile: HashMap<ProfileId, HashMap<String, f64>>) -> Zooms {
        Zooms { by_profile }
    }

    pub fn get(&self, profile: ProfileId, host: &str) -> Option<f64> {
        self.by_profile.get(&profile)?.get(&host_key(host)).copied()
    }

    pub fn set(&mut self, profile: ProfileId, host: &str, zoom: Option<f64>) {
        let hosts = self.by_profile.entry(profile).or_default();
        match zoom {
            Some(zoom) => hosts.insert(host_key(host), zoom),
            None => hosts.remove(&host_key(host)),
        };
    }

    pub fn forget(&mut self, profile: ProfileId) {
        self.by_profile.remove(&profile);
    }

    pub fn replace(&mut self, profile: ProfileId, hosts: HashMap<String, f64>) {
        self.by_profile.insert(profile, hosts);
    }
}

pub fn same(left: f64, right: f64) -> bool {
    (left - right).abs() < SAME
}

pub fn step(current: f64, by: i32, default: f64) -> f64 {
    match by {
        0 => default,
        by if by > 0 => STEPS
            .iter()
            .copied()
            .find(|zoom| *zoom > current + SAME)
            .unwrap_or(current),
        _ => STEPS
            .iter()
            .rev()
            .copied()
            .find(|zoom| *zoom < current - SAME)
            .unwrap_or(current),
    }
}
