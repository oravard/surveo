//! Proxy local de filtrage web (SPEC.md, WEB-1, WEB-2 ; IMPL.md, 2.1),
//! à l'écoute sur `127.0.0.1`, sans droits d'administration.
//!
//! En HTTPS, il ne voit que le nom du site (requête `CONNECT`) et refuse
//! la connexion : le navigateur affiche sa propre erreur, que la
//! notification de l'agent explique. En HTTP, il voit la requête entière
//! et répond par une page qui dit pourquoi le site est bloqué.
//!
//! Il sert aussi le script de configuration automatique (PAC) que l'agent
//! pose dans le réglage de l'utilisateur : « proxy local, sinon connexion
//! directe ». Si l'agent s'arrête, le script devient injoignable et le
//! navigateur se connecte directement (MODE-5).

use anyhow::{bail, Result};
use std::sync::{Arc, RwLock};
use surveo_protocol::WebPolicy;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Appelé pour chaque site bloqué, avec son nom.
pub type OnBlocked = Arc<dyn Fn(&str) + Send + Sync>;

struct Shared {
    policy: RwLock<WebPolicy>,
    teacher: String,
    on_blocked: OnBlocked,
    port: u16,
}

/// Proxy en fonctionnement ; il s'arrête quand on le lâche.
pub struct WebFilter {
    shared: Arc<Shared>,
    task: tokio::task::JoinHandle<()>,
}

impl WebFilter {
    /// Démarre le proxy sur un port libre de `127.0.0.1`.
    pub fn start(policy: WebPolicy, teacher: &str, on_blocked: OnBlocked) -> Result<Self> {
        let std = std::net::TcpListener::bind(("127.0.0.1", 0))?;
        std.set_nonblocking(true)?;
        let port = std.local_addr()?.port();
        let listener = TcpListener::from_std(std)?;
        let shared = Arc::new(Shared { policy: RwLock::new(policy), teacher: teacher.into(), on_blocked, port });
        let task = tokio::spawn(serve(listener, shared.clone()));
        Ok(Self { shared, task })
    }

    pub fn set_policy(&self, p: WebPolicy) {
        *self.shared.policy.write().unwrap() = p;
    }

    pub fn port(&self) -> u16 {
        self.shared.port
    }

    /// Adresse du script de configuration automatique.
    pub fn pac_url(&self) -> String {
        format!("http://127.0.0.1:{}/proxy.pac", self.shared.port)
    }
}

impl Drop for WebFilter {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn serve(listener: TcpListener, shared: Arc<Shared>) {
    loop {
        let Ok((client, _)) = listener.accept().await else { continue };
        let shared = shared.clone();
        tokio::spawn(async move {
            if let Err(e) = handle(client, &shared).await {
                log::debug!("proxy : {e:#}");
            }
        });
    }
}

/// Page affichée pour un site bloqué en HTTP (WEB-2).
fn blocked_page(host: &str, teacher: &str) -> String {
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let body = format!(
        "<!doctype html><html lang=\"fr\"><meta charset=\"utf-8\"><title>Site non accessible</title>\
         <body style=\"font-family:system-ui,sans-serif;max-width:36em;margin:18vh auto;padding:0 1em;color:#1f2328\">\
         <h1 style=\"font-size:1.5em\">Site non accessible pendant la séance</h1>\
         <p><b>{}</b> n'est pas accessible pendant la séance de {}.</p>\
         <p style=\"color:#59636e\">Surveo</p></body></html>",
        esc(host),
        esc(teacher)
    );
    format!(
        "HTTP/1.1 403 Forbidden\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn pac_script(port: u16) -> String {
    format!(
        "function FindProxyForURL(url, host) {{\n  if (isPlainHostName(host) || host == \"localhost\" || host == \"127.0.0.1\") return \"DIRECT\";\n  return \"PROXY 127.0.0.1:{port}; DIRECT\";\n}}\n"
    )
}

/// « hôte:port » ou « [ipv6]:port ».
fn split_authority(a: &str, default_port: u16) -> (&str, u16) {
    match a.rsplit_once(':') {
        Some((h, p)) if !p.contains(']') => (h.trim_matches(|c| c == '[' || c == ']'), p.parse().unwrap_or(default_port)),
        _ => (a.trim_matches(|c| c == '[' || c == ']'), default_port),
    }
}

async fn handle(mut client: TcpStream, shared: &Shared) -> Result<()> {
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

    if target == "/proxy.pac" {
        let pac = pac_script(shared.port);
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/x-ns-proxy-autoconfig\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{pac}",
            pac.len()
        );
        client.write_all(resp.as_bytes()).await?;
        return Ok(());
    }

    let allowed = |host: &str| shared.policy.read().unwrap().allows(host);
    // Refus d'abord, notification ensuite : l'élève n'attend jamais
    // l'affichage de la notification.
    let refuse = |mut client: TcpStream, host: String| async move {
        let r = client.write_all(blocked_page(&host, &shared.teacher).as_bytes()).await;
        drop(client);
        (shared.on_blocked)(&host);
        Ok(r?)
    };

    if method.eq_ignore_ascii_case("CONNECT") {
        let (host, port) = split_authority(target, 443);
        if !allowed(host) {
            return refuse(client, host.to_string()).await;
        }
        let mut upstream = TcpStream::connect((host, port)).await?;
        client.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n").await?;
        upstream.write_all(&buf[head_end..]).await?;
        tokio::io::copy_bidirectional(&mut client, &mut upstream).await?;
        return Ok(());
    }

    // HTTP en clair : « GET http://site/chemin HTTP/1.1 ».
    let rest = target.strip_prefix("http://").unwrap_or(target);
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = split_authority(authority, 80);
    if !allowed(host) {
        return refuse(client, host.to_string()).await;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    async fn ask(port: u16, req: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        s.write_all(req.as_bytes()).await.unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).await.unwrap();
        out
    }

    #[tokio::test]
    async fn blocks_by_policy_and_serves_the_pac_script() {
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let s = seen.clone();
        let f = WebFilter::start(WebPolicy::Block { sites: vec!["jeux.example".into()] }, "M. Durand", Arc::new(move |h| s.lock().unwrap().push(h.into())))
            .unwrap();
        let pac = ask(f.port(), "GET /proxy.pac HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").await;
        assert!(pac.contains(&format!("PROXY 127.0.0.1:{}; DIRECT", f.port())), "{pac}");

        let https = ask(f.port(), "CONNECT www.jeux.example:443 HTTP/1.1\r\n\r\n").await;
        assert!(https.starts_with("HTTP/1.1 403"));
        let http = ask(f.port(), "GET http://jeux.example/ HTTP/1.1\r\nHost: jeux.example\r\n\r\n").await;
        assert!(http.contains("<b>jeux.example</b> n'est pas accessible pendant la séance de M. Durand."), "{http}");

        // Changement de politique sans arrêter le proxy.
        f.set_policy(WebPolicy::All);
        let blocked = ask(f.port(), "CONNECT education.gouv.fr:443 HTTP/1.1\r\n\r\n").await;
        assert!(blocked.starts_with("HTTP/1.1 403"));
        assert_eq!(*seen.lock().unwrap(), ["www.jeux.example", "jeux.example", "education.gouv.fr"]);
    }

    #[test]
    fn authorities_are_split() {
        assert_eq!(split_authority("site.fr:8443", 443), ("site.fr", 8443));
        assert_eq!(split_authority("site.fr", 80), ("site.fr", 80));
        assert_eq!(split_authority("[::1]:443", 80), ("::1", 443));
        assert_eq!(split_authority("[::1]", 80), ("::1", 80));
    }
}
