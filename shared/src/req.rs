use serde::{Deserialize, Serialize};

use crate::MachineConfig;

#[derive(Serialize, Deserialize, Debug)]
pub struct Heartbeat {
    pub machine_id: String,
    pub num_packets_since_last_heartbeat: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct HeartbeatResponse {
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RegisterRequest {
    pub machine_id: String,
    pub config: MachineConfig,
}

#[derive(Serialize, Deserialize)]
pub struct RegisterResponse {
    pub error: Option<String>,
}
