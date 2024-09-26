use std::{env, future::Future};

use anyhow::anyhow;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

pub trait MachinesAPI: Sized + Send + Sync {
    fn new() -> Self;
    fn stop_machine(
        &self,
        machine_id: String,
        signal: Option<String>,
    ) -> impl Future<Output = anyhow::Result<()>> + Send;
    fn suspend_machine(
        &self,
        machine_id: String,
        signal: Option<String>,
    ) -> impl Future<Output = anyhow::Result<()>> + Send;
    fn start_machine(&self, machine_id: String) -> impl Future<Output = anyhow::Result<()>> + Send;
}

pub struct FlyAPI {
    token: String,
}

#[derive(Serialize, Deserialize)]
struct StopMachineRequest {
    signal: String,
}

impl MachinesAPI for FlyAPI {
    fn new() -> Self {
        Self {
            token: env::var("FLY_API_TOKEN").unwrap(),
        }
    }

    async fn stop_machine(&self, machine_id: String, signal: Option<String>) -> anyhow::Result<()> {
        let stop_machine_request = StopMachineRequest {
            signal: signal.unwrap_or_else(|| String::new()),
        };

        let app_name = env::var("FLY_APP_NAME")?;
        let url = format!("https://api.machines.dev/v1/apps/{app_name}/machines/{machine_id}/stop");

        let client = reqwest::Client::default();
        let req = client
            .post(url)
            .json(&stop_machine_request)
            .bearer_auth(&self.token)
            .build()?;

        let resp = client.execute(req).await?;
        handle_resp(&machine_id, resp).await
    }

    async fn suspend_machine(
        &self,
        machine_id: String,
        signal: Option<String>,
    ) -> anyhow::Result<()> {
        let stop_machine_request = StopMachineRequest {
            signal: signal.unwrap_or_else(|| String::new()),
        };

        let app_name = env::var("FLY_APP_NAME")?;
        let url =
            format!("https://api.machines.dev/v1/apps/{app_name}/machines/{machine_id}/suspend");

        let client = reqwest::Client::default();
        let req = client
            .post(url)
            .json(&stop_machine_request)
            .bearer_auth(&self.token)
            .build()?;

        let resp = client.execute(req).await?;
        handle_resp(&machine_id, resp).await
    }

    async fn start_machine(&self, machine_id: String) -> anyhow::Result<()> {
        let app_name = env::var("FLY_APP_NAME")?;
        let url =
            format!("https://api.machines.dev/v1/apps/{app_name}/machines/{machine_id}/suspend");

        let client = reqwest::Client::default();
        let req = client.post(url).bearer_auth(&self.token).build()?;

        let resp = client.execute(req).await?;
        handle_resp(&machine_id, resp).await
    }
}

async fn handle_resp(machine_id: &str, resp: reqwest::Response) -> anyhow::Result<()> {
    match resp.status() {
        StatusCode::OK => {
            println!("Suspended machine {machine_id}");
            Ok(())
        }
        StatusCode::BAD_REQUEST => Err(anyhow!("bad request")),
        status => {
            let body = resp.text().await.unwrap();

            if body.contains("current state invalid") {
                return Ok(());
            }

            Err(anyhow!("invalid status code {status} with body: {}", body))
        }
    }
}
