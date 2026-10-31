//! Recherche du relais sur le réseau local (SPEC.md, DECOUV-1, niveau 3).
//!
//! Le poste émet une recherche par diffusion sur chacune de ses
//! interfaces ; le relais, qui seul écoute, lui répond directement. Un
//! poste n'a ainsi jamais rien à écouter (RELAIS-4) : la réponse à un
//! envoi est admise par les pare-feux. La réponse n'est qu'une
//! indication ; la confiance vient ensuite de l'authentification
//! (DECOUV-2).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio::net::UdpSocket;

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "surveo", rename_all = "snake_case")]
enum Datagram {
    /// Poste → réseau local.
    Recherche { v: u16, etab: String },
    /// Relais → poste.
    Relais { v: u16, etab: String, port: u16 },
}

/// Cherche les relais de l'établissement `etab` pendant `wait`, et rend
/// leurs adresses (adresse du répondant, port annoncé). `port` vaut
/// [`DISCOVERY_PORT`](surveo_protocol::DISCOVERY_PORT) hors des tests.
pub async fn search(etab: &str, wait: Duration, port: u16) -> Result<Vec<SocketAddr>> {
    let sock = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await.context("recherche du relais")?;
    sock.set_broadcast(true)?;
    let msg = serde_json::to_vec(&Datagram::Recherche { v: 1, etab: etab.into() })?;
    for dest in destinations() {
        let _ = sock.send_to(&msg, (dest, port)).await;
    }
    let mut found = BTreeSet::new();
    let mut buf = [0u8; 512];
    let deadline = tokio::time::Instant::now() + wait;
    while let Ok(Ok((n, from))) = tokio::time::timeout_at(deadline, sock.recv_from(&mut buf)).await {
        if let Ok(Datagram::Relais { etab: e, port, .. }) = serde_json::from_slice(&buf[..n]) {
            if e == etab {
                found.insert(SocketAddr::new(from.ip(), port));
            }
        }
    }
    Ok(found.into_iter().collect())
}

/// Adresses de diffusion de chaque interface, la diffusion générale et
/// l'interface locale (relais sur le même poste, pour les essais).
fn destinations() -> BTreeSet<Ipv4Addr> {
    let mut d = BTreeSet::from([Ipv4Addr::BROADCAST, Ipv4Addr::LOCALHOST]);
    if let Ok(ifs) = if_addrs::get_if_addrs() {
        for i in ifs {
            if let if_addrs::IfAddr::V4(v4) = i.addr {
                if let Some(b) = v4.broadcast {
                    d.insert(b);
                }
            }
        }
    }
    d
}

/// Côté relais : répond aux recherches des postes de l'établissement, sur
/// le port UDP `port`. Échoue si ce port est déjà pris (un autre relais
/// sur ce poste).
pub async fn respond(etab: String, relay_port: u16, port: u16) -> Result<tokio::task::JoinHandle<()>> {
    let sock = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, port))
        .await
        .with_context(|| format!("écoute des recherches sur le port UDP {port}"))?;
    let reply = serde_json::to_vec(&Datagram::Relais { v: 1, etab: etab.clone(), port: relay_port })?;
    Ok(tokio::spawn(async move {
        let mut buf = [0u8; 512];
        loop {
            let Ok((n, from)) = sock.recv_from(&mut buf).await else { continue };
            if let Ok(Datagram::Recherche { etab: e, .. }) = serde_json::from_slice(&buf[..n]) {
                if e == etab {
                    let _ = sock.send_to(&reply, from).await;
                }
            }
        }
    }))
}

/// Adresses IPv4 de ce poste hors interface locale, pour les indiquer à
/// l'enseignant et dans les paquets de rattachement.
pub fn local_addresses() -> Vec<Ipv4Addr> {
    let mut v: Vec<Ipv4Addr> = if_addrs::get_if_addrs()
        .map(|ifs| {
            ifs.into_iter()
                .filter_map(|i| match i.addr {
                    if_addrs::IfAddr::V4(v4) if !v4.ip.is_loopback() && !v4.ip.is_link_local() => Some(v4.ip),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v.dedup();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_relay_answers_only_its_establishment() {
        // Port de test, pour ne pas gêner un relais en fonctionnement.
        let port = 47900;
        let task = respond("lycee-a".into(), 5555, port).await.unwrap();
        let found = search("lycee-a", Duration::from_millis(400), port).await.unwrap();
        assert!(found.iter().any(|a| a.port() == 5555), "{found:?}");
        let none = search("lycee-b", Duration::from_millis(300), port).await.unwrap();
        assert!(none.is_empty());
        task.abort();
    }
}
