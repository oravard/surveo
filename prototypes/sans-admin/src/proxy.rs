//! Proxy local de filtrage, à l'écoute sur 127.0.0.1 (aucun droit
//! d'administration requis). Il applique une politique de la séance
//! (SPEC.md, WEB-1) et tient le compte de ses décisions.

use anyhow::{bail, Result};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Debug, Clone)]
pub enum Policy {
    /// Seuls ces sites sont accessibles (liste d'autorisation).
    Allow(Vec<String>),
    /// Ces sites sont bloqués, le reste est accessible (liste de blocage).
    Block(Vec<String>),
    /// Aucun site accessible.
    All,
}

impl Policy {
    pub fn allows(&self, host: &str) -> bool {
        let host = host.trim_end_matches('.').to_ascii_lowercase();
        let matches = |list: &[String]| list.iter().any(|d| host == *d || host.ends_with(&format!(".{d}")));
        match self {
            Policy::Allow(l) => matches(l),
            Policy::Block(l) => !matches(l),
            Policy::All => false,
        }
    }
}

/// Compteurs par site : (autorisées, bloquées) pour HTTPS et HTTP.
#[derive(Default)]
pub struct Tally {
    pub sites: BTreeMap<String, [u32; 4]>,
}

impl Tally {
    fn add(&mut self, host: &str, https: bool, allowed: bool) {
        let e = self.sites.entry(host.to_string()).or_default();
        e[(if https { 0 } else { 2 }) + if allowed { 0 } else { 1 }] += 1;
    }

    pub fn print(&self) {
        if self.sites.is_empty() {
            println!("  Aucune connexion n'est passée par le proxy.");
            return;
        }
        println!("  {:<40} {:>14} {:>14}", "Site", "HTTPS aut./bl.", "HTTP aut./bl.");
        for (site, c) in &self.sites {
            println!("  {:<40} {:>8}/{:<5} {:>8}/{:<5}", site, c[0], c[1], c[2], c[3]);
        }
    }
}

/// Page affichée pour un site bloqué (SPEC.md, WEB-2). Elle n'est visible
/// qu'en HTTP : en HTTPS, le navigateur montre sa propre erreur.
fn blocked_page(host: &str) -> String {
    let body = format!(
        "<!doctype html><html lang=\"fr\"><meta charset=\"utf-8\"><title>Accès suspendu</title>\
         <body style=\"font-family:sans-serif;max-width:40em;margin:15vh auto;padding:0 1em\">\
         <h1>Accès suspendu pendant la séance</h1>\
         <p>Le site <b>{host}</b> n'est pas accessible pendant la séance de M. Durand.</p>\
         <p style=\"color:#666\">Surveo · prototype</p></body></html>"
    );
    format!(
        "HTTP/1.1 403 Forbidden\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
}

pub async fn serve(listener: TcpListener, policy: Arc<Policy>, tally: Arc<Mutex<Tally>>) {
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
    loop {
        let Ok((client, _)) = listener.accept().await else { continue };
        let (policy, tally) = (policy.clone(), tally.clone());
        tokio::spawn(async move {
            let _ = handle(client, &policy, &tally, port).await;
        });
    }
}

async fn handle(mut client: TcpStream, policy: &Policy, tally: &Mutex<Tally>, port: u16) -> Result<()> {
    // Lecture de l'en-tête de la requête.
    let mut buf = Vec::with_capacity(4096);
    let head_end = loop {
        let mut tmp = [0u8; 4096];
        let n = client.read(&mut tmp).await?;
        if n == 0 {
            bail!("connexion fermée");
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break p + 4;
        }
        if buf.len() > 32 * 1024 {
            bail!("en-tête trop long");
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let mut first = head.lines().next().unwrap_or("").split_whitespace();
    let (method, target, version) = (first.next().unwrap_or(""), first.next().unwrap_or(""), first.next().unwrap_or("HTTP/1.1"));

    // Script de configuration automatique (PAC) : « proxy local, sinon
    // connexion directe ». Si l'agent s'arrête, le script devient
    // inaccessible et le navigateur se connecte directement : la
    // restriction ne survit pas à l'agent (SPEC.md, RESTR-1).
    if target == "/proxy.pac" {
        let pac = format!(
            "function FindProxyForURL(url, host) {{\n  if (isPlainHostName(host) || host == \"localhost\" || host == \"127.0.0.1\") return \"DIRECT\";\n  return \"PROXY 127.0.0.1:{port}; DIRECT\";\n}}\n"
        );
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/x-ns-proxy-autoconfig\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{pac}",
            pac.len()
        );
        client.write_all(resp.as_bytes()).await?;
        return Ok(());
    }

    if method.eq_ignore_ascii_case("CONNECT") {
        // HTTPS : le proxy ne voit que le nom du site, pas la page.
        let (host, port) = target.rsplit_once(':').unwrap_or((target, "443"));
        let host = host.trim_matches(|c| c == '[' || c == ']');
        let allowed = policy.allows(host);
        tally.lock().unwrap().add(host, true, allowed);
        if !allowed {
            client.write_all(blocked_page(host).as_bytes()).await?;
            return Ok(());
        }
        let mut upstream = TcpStream::connect((host, port.parse::<u16>().unwrap_or(443))).await?;
        client.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n").await?;
        upstream.write_all(&buf[head_end..]).await?;
        tokio::io::copy_bidirectional(&mut client, &mut upstream).await?;
        return Ok(());
    }

    // HTTP en clair : requête de la forme « GET http://site/chemin ».
    let rest = target.strip_prefix("http://").unwrap_or(target);
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = authority.rsplit_once(':').map(|(h, p)| (h, p.parse().unwrap_or(80))).unwrap_or((authority, 80u16));
    let allowed = policy.allows(host);
    tally.lock().unwrap().add(host, false, allowed);
    if !allowed {
        client.write_all(blocked_page(host).as_bytes()).await?;
        return Ok(());
    }
    let mut upstream = TcpStream::connect((host, port)).await?;
    let mut out = format!("{method} {path} {version}\r\n");
    for line in head.lines().skip(1) {
        let l = line.to_ascii_lowercase();
        if line.is_empty() || l.starts_with("proxy-connection:") || l.starts_with("connection:") {
            continue;
        }
        out.push_str(line);
        out.push_str("\r\n");
    }
    out.push_str("Connection: close\r\n\r\n");
    upstream.write_all(out.as_bytes()).await?;
    upstream.write_all(&buf[head_end..]).await?;
    tokio::io::copy_bidirectional(&mut client, &mut upstream).await?;
    Ok(())
}
