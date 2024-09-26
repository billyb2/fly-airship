pub mod req;

use serde::{Deserialize, Serialize};

pub type MachineID = String;

#[derive(Serialize, Deserialize, Debug)]
pub enum Autostop {
    Stop,
    Suspend,
    None,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MachineConfig {
    pub auto_stop: Option<Autostop>,
    pub auto_start: Option<bool>,
    pub auto_stop_timeout_seconds: Option<u64>,
    pub stop_signal: Option<String>,
}
