use std::env;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use pnet::datalink::{self, interfaces, Channel};
use pnet::packet::ethernet::EthernetPacket;
use pnet::packet::udp::UdpPacket;
use pnet::packet::Packet;
use reqwest::StatusCode;
use shared::req::{Heartbeat, RegisterRequest};
use shared::{Autostop, MachineConfig};

#[tokio::main]
async fn main() {
    let interface_name = env::var("INTERFACE_NAME").unwrap();
    let interface = interfaces()
        .into_iter()
        .find(|interface| interface.name.as_str() == interface_name)
        .unwrap();
    let (_, mut rx) = match datalink::channel(&interface, Default::default()) {
        Ok(Channel::Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => panic!("Unhandled channel type: {}", &interface),
        Err(e) => panic!(
            "An error occurred when creating the datalink channel: {}",
            e
        ),
    };

    let num_packets = Arc::new(AtomicU64::new(0));
    let num_packets_clone = Arc::clone(&num_packets);

    let server_url = env::var("AIRSHIP_SERVER_URL").unwrap();
    let client = reqwest::Client::default();

    let machine_id = env::var("FLY_MACHINE_ID").unwrap();
    let register_req = RegisterRequest {
        machine_id: machine_id.clone(),
        config: MachineConfig {
            auto_stop: Some(Autostop::Stop),
            auto_start: Some(true),
            auto_stop_timeout_seconds: Some(60),
            stop_signal: None,
        },
    };

    let req = client
        .post(format!("{server_url}/register"))
        .json(&register_req)
        .build()
        .unwrap();

    client.execute(req).await.unwrap();

    tokio::task::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;

            let num_packets = num_packets_clone.swap(0, Ordering::SeqCst);
            if num_packets == 0 {
                continue;
            }

            let req = client
                .post(format!("{server_url}/heartbeat"))
                .json(&Heartbeat {
                    machine_id: machine_id.clone(),
                    num_packets_since_last_heartbeat: num_packets,
                })
                .build()
                .unwrap();

            let resp = client.execute(req).await.unwrap();
            if resp.status() == StatusCode::BAD_REQUEST
                && resp
                    .text()
                    .await
                    .map(|resp| resp.contains("machine not registerd"))
                    .unwrap_or(false)
            {
                let req = client
                    .post(format!("{server_url}/register"))
                    .json(&register_req)
                    .build()
                    .unwrap();

                client.execute(req).await.unwrap();
            }
        }
    });

    loop {
        match rx.next() {
            Ok(packet) => {
                if let Some(ethernet_packet) = EthernetPacket::new(packet) {
                    if let Some(udp_packet) = UdpPacket::new(ethernet_packet.payload()) {
                        println!("{num_packets:?}");
                        num_packets.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
            Err(e) => {
                panic!("An error occurred while reading: {}", e);
            }
        }
        tokio::task::yield_now().await;
    }
}
