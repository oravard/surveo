//! Simule N agents de poste élève : chacun trouve le relais, se connecte
//! en sortie, s'authentifie, envoie une miniature par seconde et acquitte
//! aussitôt les commandes reçues.
//!
//! Usage : agent-sim [--keys keys] [--identities keys/agents.json]
//!                   [--count 30] [--relay ADRESSE:PORT] [--thumb-kb 20]

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use surveo_proto_relais::*;
use tokio_tungstenite::tungstenite::Message;

#[tokio::main]
async fn main() -> Result<()> {
    let keys = PathBuf::from(arg("--keys").unwrap_or_else(|| "keys".into()));
    let ids_path = arg("--identities").map(PathBuf::from).unwrap_or_else(|| keys.join("agents.json"));
    let count: usize = arg("--count").map(|s| s.parse()).transpose()?.unwrap_or(30);
    let thumb_kb: usize = arg("--thumb-kb").map(|s| s.parse()).transpose()?.unwrap_or(20);

    let trust = Arc::new(Trust::from_file(&load_json(&keys.join("etab.json"))?)?);
    let ids: Vec<IdentityFile> = load_json(&ids_path)?;
    let ids: Vec<Identity> = ids.iter().take(count).map(Identity::from_file).collect::<Result<_>>()?;

    let relay = match arg("--relay") {
        Some(a) => a,
        None => {
            let a = discover(Duration::from_secs(6)).await?;
            println!("Relais trouvé par son annonce sur le réseau local : {a}");
            a
        }
    };

    // Contenu factice d'une miniature (octets aléatoires, donc incompressibles).
    let noise: Arc<Vec<u8>> = Arc::new((0..thumb_kb * 1024 * 2).map(|_| rand::random::<u8>()).collect());
    let tls = client_tls()?;
    let auth_ms = Arc::new(Mutex::new(Vec::<f64>::new()));
    let started = Instant::now();
    let n = ids.len();

    let mut tasks = Vec::new();
    for me in ids {
        let (relay, tls, trust, noise, auth_ms) = (relay.clone(), tls.clone(), trust.clone(), noise.clone(), auth_ms.clone());
        tasks.push(tokio::spawn(async move {
            let t0 = Instant::now();
            let ws = match connect(&relay, &tls, &me, &trust).await {
                Ok(ws) => ws,
                Err(e) => {
                    println!("{} : {e:#}", me.cert.member);
                    return;
                }
            };
            auth_ms.lock().unwrap().push(t0.elapsed().as_secs_f64() * 1000.0);
            run_agent(ws, &me.cert.member, &noise, thumb_kb).await;
        }));
    }

    // Bilan des connexions dès qu'elles sont toutes établies (ou au bout de 10 s).
    let deadline = Instant::now() + Duration::from_secs(10);
    while auth_ms.lock().unwrap().len() < n && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    {
        let mut v = auth_ms.lock().unwrap().clone();
        let ok = v.len();
        println!(
            "{ok}/{n} agents connectés et authentifiés en {:.0} ms · par agent : médiane {:.1} ms, max {:.1} ms",
            started.elapsed().as_secs_f64() * 1000.0,
            percentile(&mut v, 50.0),
            percentile(&mut v, 100.0),
        );
    }

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = futures_util::future::join_all(tasks) => {}
    }
    Ok(())
}

async fn run_agent(ws: ClientWs, member: &str, noise: &[u8], thumb_kb: usize) {
    let (mut sink, mut stream) = ws.split();
    // Départ décalé pour ne pas synchroniser toutes les miniatures.
    let offset = Duration::from_millis(rand::thread_rng().gen_range(0..1000));
    let mut tick = tokio::time::interval_at(tokio::time::Instant::now() + offset, Duration::from_secs(1));
    loop {
        tokio::select! {
            _ = tick.tick() => {
                let len = thumb_kb * 1024 * rand::thread_rng().gen_range(70..=130) / 100;
                let start = rand::thread_rng().gen_range(0..noise.len() - len);
                if sink.send(Message::Binary(noise[start..start + len].to_vec().into())).await.is_err() {
                    break;
                }
            }
            msg = stream.next() => {
                match msg {
                    Some(Ok(Message::Text(t))) => {
                        if let Ok(Ctl::Command { id, .. }) = serde_json::from_str::<Ctl>(t.as_str()) {
                            let ack = serde_json::to_string(&Ctl::Ack { id, agent: member.to_string() }).unwrap();
                            if sink.send(Message::Text(ack.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Some(Ok(_)) => {}
                    _ => break,
                }
            }
        }
    }
}
