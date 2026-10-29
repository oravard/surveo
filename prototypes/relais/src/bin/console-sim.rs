//! Simule la console de l'enseignant : se connecte au relais, envoie une
//! commande par seconde à toute la salle et mesure le temps aller-retour
//! de chaque acquittement, ainsi que le débit des miniatures reçues.
//! À la fin, compare les mesures aux critères du prototype (IMPL.md).
//!
//! Usage : console-sim [--keys keys] [--relay 127.0.0.1:8443] [--duration 30]

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use surveo_proto_relais::*;
use tokio_tungstenite::tungstenite::Message;

/// Commande en vol : instant d'envoi, agents attendus, acquittements reçus.
struct InFlight {
    sent: Instant,
    expected: usize,
    acks: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    let keys = PathBuf::from(arg("--keys").unwrap_or_else(|| "keys".into()));
    let relay = arg("--relay").unwrap_or_else(|| "127.0.0.1:8443".into());
    let duration: u64 = arg("--duration").map(|s| s.parse()).transpose()?.unwrap_or(30);

    let trust = Trust::from_file(&load_json(&keys.join("etab.json"))?)?;
    let me = Identity::from_file(&load_json(&keys.join("console.json"))?)?;
    let ws = connect(&relay, &client_tls()?, &me, &trust).await?;
    println!("Console « {} » connectée et authentifiée auprès du relais {relay}", me.cert.member);
    let (mut sink, mut stream) = ws.split();

    let mut agents = 0usize;
    let mut next_id = 0u64;
    let mut flights: HashMap<u64, InFlight> = HashMap::new();
    let (mut win_lat, mut all_lat) = (Vec::<f64>::new(), Vec::<f64>::new());
    let (mut win_frames, mut win_bytes, mut all_frames) = (0u64, 0u64, 0u64);
    let (mut missing_total, mut commands_total) = (0usize, 0usize);
    let mut cmd_tick = tokio::time::interval(Duration::from_secs(1));
    let mut report_tick = tokio::time::interval_at(tokio::time::Instant::now() + Duration::from_secs(5), Duration::from_secs(5));
    let mut win_start = Instant::now();
    let end = tokio::time::sleep(Duration::from_secs(duration));
    tokio::pin!(end);

    loop {
        tokio::select! {
            _ = &mut end => break,
            _ = cmd_tick.tick() => {
                if agents > 0 {
                    next_id += 1;
                    flights.insert(next_id, InFlight { sent: Instant::now(), expected: agents, acks: 0 });
                    let cmd = serde_json::to_string(&Ctl::Command { id: next_id, action: "ping".into(), text: None, target: None })?;
                    sink.send(Message::Text(cmd.into())).await?;
                }
            }
            _ = report_tick.tick() => {
                // Commandes de plus de 2 s : les acquittements absents sont comptés manquants.
                let old: Vec<u64> = flights.iter().filter(|(_, f)| f.sent.elapsed() > Duration::from_secs(2)).map(|(id, _)| *id).collect();
                let mut missing = 0;
                for id in old {
                    let f = flights.remove(&id).unwrap();
                    missing += f.expected.saturating_sub(f.acks);
                    commands_total += 1;
                }
                missing_total += missing;
                let dt = win_start.elapsed().as_secs_f64();
                println!(
                    "[console] agents {:>3} · miniatures {:>6.1}/s ({:.2} Mo/s) · aller-retour d'une commande : médiane {:>5.1} ms, p95 {:>5.1} ms, max {:>5.1} ms · acquittements manquants {}",
                    agents,
                    win_frames as f64 / dt,
                    win_bytes as f64 / dt / 1e6,
                    percentile(&mut win_lat, 50.0),
                    percentile(&mut win_lat, 95.0),
                    percentile(&mut win_lat, 100.0),
                    missing,
                );
                win_lat.clear();
                win_frames = 0;
                win_bytes = 0;
                win_start = Instant::now();
            }
            msg = stream.next() => {
                match msg {
                    Some(Ok(Message::Text(t))) => match serde_json::from_str::<Ctl>(t.as_str()) {
                        Ok(Ctl::Presence { agents: n }) => agents = n,
                        Ok(Ctl::Ack { id, .. }) => {
                            if let Some(f) = flights.get_mut(&id) {
                                f.acks += 1;
                                let ms = f.sent.elapsed().as_secs_f64() * 1000.0;
                                win_lat.push(ms);
                                all_lat.push(ms);
                            }
                        }
                        _ => {}
                    },
                    Some(Ok(Message::Binary(b))) => {
                        win_frames += 1;
                        all_frames += 1;
                        win_bytes += b.len() as u64;
                    }
                    Some(Ok(_)) => {}
                    _ => { println!("Connexion au relais perdue"); break; }
                }
            }
        }
    }

    let p95 = percentile(&mut all_lat, 95.0);
    let max = percentile(&mut all_lat, 100.0);
    let rate = all_frames as f64 / duration as f64;
    println!("\nBilan sur {duration} s, {agents} agents, {commands_total} commandes évaluées :");
    println!(
        "  Commande en moins de 200 ms : {} (p95 {:.1} ms, max {:.1} ms)",
        if p95 < 200.0 { "OK" } else { "ÉCHEC" },
        p95,
        max
    );
    println!(
        "  Miniatures à 1 image/s par agent : {} ({:.2} image/s par agent en moyenne)",
        if agents > 0 && rate / agents as f64 > 0.9 { "OK" } else { "ÉCHEC" },
        if agents > 0 { rate / agents as f64 } else { 0.0 }
    );
    println!("  Acquittements manquants : {missing_total}");
    Ok(())
}
