//! Relais autonome du prototype : le cœur est dans `relay_core`, partagé
//! avec la console Tauri qui l'embarque (prototype 4). Affiche sa
//! consommation toutes les 5 s.
//!
//! Usage : relay [--keys keys] [--port 8443]

use anyhow::Result;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};
use surveo_proto_relais::arg;
use surveo_proto_relais::relay_core::{NoEvents, Relay, State};

#[tokio::main]
async fn main() -> Result<()> {
    let keys = PathBuf::from(arg("--keys").unwrap_or_else(|| "keys".into()));
    let port: u16 = arg("--port").map(|s| s.parse()).transpose()?.unwrap_or(8443);
    let relay = Relay::start(&keys, port, Arc::new(NoEvents)).await?;
    println!("Relais « {} » à l'écoute sur le port {}", relay.member(), relay.port);
    report(relay.state.clone()).await;
    Ok(())
}

/// Consommation du relais toutes les 5 s (Linux : /proc/self).
async fn report(state: Arc<State>) {
    let mut last = (Instant::now(), cpu_ticks(), 0u64, 0u64);
    loop {
        tokio::time::sleep(Duration::from_secs(5)).await;
        let now = (Instant::now(), cpu_ticks(), state.frames.load(Ordering::Relaxed), state.bytes.load(Ordering::Relaxed));
        let dt = now.0.duration_since(last.0).as_secs_f64();
        println!(
            "[relais] agents {:>3} · consoles {} · miniatures {:>6.1}/s ({:.2} Mo/s) · CPU {:>5.1} % d'un cœur · mémoire {:.1} Mo · refus {}",
            state.agent_count(),
            state.console_count(),
            (now.2 - last.2) as f64 / dt,
            (now.3 - last.3) as f64 / dt / 1e6,
            (now.1 - last.1) as f64 / 100.0 / dt * 100.0,
            rss_mb(),
            state.denied.load(Ordering::Relaxed),
        );
        last = now;
    }
}

fn cpu_ticks() -> u64 {
    let s = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let after = s.rsplit_once(')').map(|x| x.1).unwrap_or("");
    let f: Vec<&str> = after.split_whitespace().collect();
    let get = |i: usize| f.get(i).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
    get(11) + get(12)
}

fn rss_mb() -> f64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("VmRSS:")).and_then(|l| l.split_whitespace().nth(1)?.parse::<f64>().ok()))
        .map(|kb| kb / 1024.0)
        .unwrap_or(0.0)
}
