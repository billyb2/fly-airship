mod api;
mod db;

use std::{
    convert::Infallible,
    net::{IpAddr, SocketAddr},
    sync::{Arc, LazyLock},
    time::Duration,
};

use api::{FlyAPI, MachinesAPI};
use db::{Database, InMemoryDatabase, MachineNotRegisterd};
use shared::req::{Heartbeat, HeartbeatResponse, RegisterRequest, RegisterResponse};
use warp::{reject::Reject, Filter};

#[derive(Debug)]
struct InvalidIP {}
impl Reject for InvalidIP {}

static API: LazyLock<FlyAPI> = LazyLock::new(|| FlyAPI::new());

#[tokio::main]
pub async fn main() {
    let in_memory_db = Arc::new(InMemoryDatabase::new(100));

    let in_memory_db_clone = Arc::clone(&in_memory_db);
    tokio::task::spawn(async move {
        let in_memory_db = in_memory_db_clone;
        loop {
            let api: &FlyAPI = &API;
            in_memory_db.scan(api).await;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });

    let ip_filter = warp::filters::addr::remote().and_then(
        move |addr: Option<std::net::SocketAddr>| async move {
            return Ok(());
            if let Some(addr) = addr {
                if let IpAddr::V6(ipv6) = addr.ip() {
                    if ipv6.octets()[0..2] == [0xfd; 0xaa] {
                        Ok(())
                    } else {
                        Err(warp::reject::custom(InvalidIP {}))
                    }
                } else {
                    Err(warp::reject::custom(InvalidIP {}))
                }
            } else {
                Err(warp::reject::custom(InvalidIP {}))
            }
        },
    );

    let in_memory_db_clone = Arc::clone(&in_memory_db);
    let hearbeat = warp::path("heartbeat")
        .and(ip_filter)
        .and(warp::body::json())
        .and(warp::any().map(move || Arc::clone(&in_memory_db_clone)))
        .and_then(heartbeat);

    let register = warp::path("register")
        .and(ip_filter)
        .and(warp::body::json())
        .and(warp::any().map(move || Arc::clone(&in_memory_db)))
        .and_then(register);

    let routes = warp::post().and(register.or(hearbeat));
    let addr: SocketAddr = "[::]:8080".parse().unwrap();
    warp::serve(routes).run(addr).await;
}

async fn register(
    _: (),
    req: RegisterRequest,
    db: Arc<InMemoryDatabase>,
) -> Result<warp::reply::Json, Infallible> {
    db.register(&req.machine_id, req.config).await;
    Ok(warp::reply::json(&RegisterResponse { error: None }))
}

async fn heartbeat(
    _: (),
    req: Heartbeat,
    db: Arc<InMemoryDatabase>,
) -> Result<warp::reply::WithStatus<warp::reply::Json>, Infallible> {
    match db.heartbeat(req).await {
        Ok(_) => Ok(warp::reply::with_status(
            warp::reply::json(&HeartbeatResponse { error: None }),
            warp::http::StatusCode::OK,
        )),
        Err(err) => match err.downcast_ref::<MachineNotRegisterd>() {
            Some(_) => Ok(warp::reply::with_status(
                warp::reply::json(&HeartbeatResponse {
                    error: Some("machine not registered".to_string()),
                }),
                warp::http::StatusCode::BAD_REQUEST,
            )),
            None => Ok(warp::reply::with_status(
                warp::reply::json(&HeartbeatResponse {
                    error: Some(err.to_string()),
                }),
                warp::http::StatusCode::BAD_GATEWAY,
            )),
        },
    }
}
