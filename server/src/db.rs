use std::{
    collections::HashMap,
    error::Error,
    fmt::Display,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use shared::{req::Heartbeat, Autostop, MachineConfig, MachineID};
use tokio::{
    sync::{Mutex, RwLock},
    time::Instant,
};

use crate::api::MachinesAPI;

pub trait Database {
    fn new(pps_per_machine: u64) -> Self
    where
        Self: Sized;
    async fn heartbeat(&self, heartbeat: Heartbeat) -> anyhow::Result<()>;
    async fn register(&self, machine_id: &MachineID, config: MachineConfig);
    async fn scan<M: MachinesAPI>(&self, api: &'static M);
}

#[derive(PartialEq, Eq)]
enum MachineStatus {
    Started,
    //. suspended or stopped
    Stopped,
}

struct MachineInfo {
    config: MachineConfig,
    last_heartbeat_received: Instant,
    status: MachineStatus,
}

pub struct InMemoryDatabase {
    machines: Arc<RwLock<HashMap<MachineID, MachineInfo>>>,
    // the total number of packets we've received since the last scan
    total_packets: AtomicU64,
    // the time that we last did a scan
    last_scan_time: Mutex<Instant>,
    // the number of packets per second that each machine should be handling
    pps_per_machine: u64,
}

#[derive(Debug)]
pub struct MachineNotRegisterd;

impl Display for MachineNotRegisterd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("machine not registered")
    }
}

impl Error for MachineNotRegisterd {}

impl Database for InMemoryDatabase {
    fn new(pps_per_machine: u64) -> Self {
        Self {
            machines: Arc::new(RwLock::new(HashMap::new())),
            total_packets: AtomicU64::new(0),
            pps_per_machine,
            last_scan_time: Mutex::new(Instant::now()),
        }
    }

    async fn register(&self, machine_id: &MachineID, config: MachineConfig) {
        let machines = &mut self.machines.write().await;
        machines.insert(
            machine_id.clone(),
            MachineInfo {
                config,
                last_heartbeat_received: Instant::now(),
                status: MachineStatus::Started,
            },
        );

        println!("Registered machine {machine_id}");
    }

    async fn heartbeat(&self, heartbeat: Heartbeat) -> anyhow::Result<()> {
        self.total_packets.fetch_add(
            heartbeat.num_packets_since_last_heartbeat,
            Ordering::Relaxed,
        );

        let machines = &mut self.machines.write().await;
        let machine_info = machines
            .get_mut(&heartbeat.machine_id)
            .ok_or_else(|| MachineNotRegisterd {})?;
        machine_info.last_heartbeat_received = Instant::now();
        Ok(())
    }

    async fn scan<M: MachinesAPI>(&self, api: &'static M) {
        let time = Instant::now();

        let num_machines_to_start = {
            let machines = self.machines.read().await;
            let num_started_machines = machines
                .values()
                .filter(|machine_info| machine_info.status == MachineStatus::Started)
                .count();

            for (machine_id, machine_info) in machines
                .iter()
                .filter(|(_, machine_info)| machine_info.status == MachineStatus::Started)
            {
                let duration = time.duration_since(machine_info.last_heartbeat_received);
                // if the auto stop duration has passed, we kill the machine
                if duration.as_secs() >= machine_info.config.auto_stop_timeout_seconds.unwrap_or(60)
                {
                    // we need to keep at least one machine started
                    if num_started_machines == 1 {
                        continue;
                    }

                    match &machine_info.config.auto_stop {
                        Some(stop_config) => match stop_config {
                            Autostop::Stop => {
                                let machine_id = machine_id.clone();
                                let machines = Arc::clone(&self.machines);

                                tokio::task::spawn(async move {
                                    match api.stop_machine(machine_id.clone(), None).await {
                                        Ok(_) => {
                                            println!("Stopped machine {machine_id}");
                                            let machines = &mut machines.write().await;
                                            let machine_info =
                                                machines.get_mut(&machine_id).unwrap();
                                            machine_info.status = MachineStatus::Stopped;
                                        }
                                        Err(err) => {
                                            println!("Error stopping machine {machine_id}: {err}");
                                        }
                                    }
                                });
                            }
                            Autostop::Suspend => {
                                let machine_id = machine_id.clone();
                                let machines = Arc::clone(&self.machines);

                                tokio::task::spawn(async move {
                                    match api.suspend_machine(machine_id.clone(), None).await {
                                        Ok(_) => {
                                            println!("Suspended machine {machine_id}");
                                            let machines = &mut machines.write().await;
                                            let machine_info =
                                                machines.get_mut(&machine_id).unwrap();
                                            machine_info.status = MachineStatus::Stopped;
                                        }
                                        Err(err) => {
                                            println!(
                                                "Error suspending machine {machine_id}: {err}"
                                            );
                                        }
                                    }
                                });
                            }
                            Autostop::None => (),
                        },
                        None => (),
                    }
                }
            }

            let time_since_last_scan_secs = {
                let last_scan_instant = &mut self.last_scan_time.lock().await;
                let secs = Instant::now().duration_since(**last_scan_instant).as_secs();
                **last_scan_instant = Instant::now();
                secs
            };
            let total_packets_since_last_scan = self.total_packets.swap(0, Ordering::SeqCst);
            total_packets_since_last_scan
                .checked_rem(self.pps_per_machine * time_since_last_scan_secs)
                .unwrap_or(0)
        };

        let machines = &mut self.machines.write().await;
        let mut startable_machines: Vec<_> = machines
            .iter_mut()
            .filter(|(_machine_id, machine_info)| {
                machine_info.status == MachineStatus::Stopped
                    && machine_info.config.auto_start == Some(true)
            })
            .collect();

        for _ in 0..num_machines_to_start {
            if let Some((machine_id, machine_info)) = startable_machines.pop() {
                match api.start_machine(machine_id.clone()).await {
                    Ok(_) => {
                        println!("Started machine {machine_id}");
                        machine_info.status = MachineStatus::Started;
                    }
                    Err(err) => {
                        println!("Error starting machine {machine_id}: {err}");
                    }
                }
            }
        }
    }
}
