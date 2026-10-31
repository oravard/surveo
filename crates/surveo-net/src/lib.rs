//! Transport entre les membres d'un établissement (IMPL.md, « Protocole »).
//!
//! - [`tls`] : WebSocket sur TLS. Le certificat TLS du relais est
//!   auto-signé ; la confiance vient de la clé d'établissement.
//! - [`handshake`] : authentification mutuelle par certificats de membre,
//!   liée à la session TLS (IMPL.md, 6.1).
//! - [`discovery`] : recherche du relais sur le réseau local, émise par le
//!   poste ; seul le relais écoute (SPEC.md, DECOUV-1, RELAIS-4).

pub mod discovery;
pub mod handshake;
pub mod tls;

use anyhow::{bail, Result};
use futures_util::{SinkExt, StreamExt};
use serde::{de::DeserializeOwned, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

pub use tokio_tungstenite::tungstenite::Message as WsMessage;

/// Connexion d'un client (agent ou console) au relais.
pub type ClientWs = WebSocketStream<tokio_rustls::client::TlsStream<tokio::net::TcpStream>>;
/// Connexion vue du relais.
pub type ServerWs = WebSocketStream<tokio_rustls::server::TlsStream<tokio::net::TcpStream>>;

/// Refus explicite par le pair ou par nous-mêmes (CONF-3). Les autres
/// erreurs sont des pannes de réseau.
#[derive(Debug)]
pub struct Refused(pub String);

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "refusé : {}", self.0)
    }
}

impl std::error::Error for Refused {}

pub async fn send_json<S, T>(ws: &mut WebSocketStream<S>, value: &T) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
    T: Serialize,
{
    ws.send(Message::Text(serde_json::to_string(value)?.into())).await?;
    Ok(())
}

/// Prochain message de contrôle ; les ping et pong sont sautés.
pub async fn recv_json<S, T>(ws: &mut WebSocketStream<S>) -> Result<T>
where
    S: AsyncRead + AsyncWrite + Unpin,
    T: DeserializeOwned,
{
    loop {
        match ws.next().await {
            Some(Ok(Message::Text(t))) => return Ok(serde_json::from_str(t.as_str())?),
            Some(Ok(Message::Ping(_) | Message::Pong(_))) => continue,
            Some(Ok(m)) => bail!("trame inattendue : {m:?}"),
            Some(Err(e)) => return Err(e.into()),
            None => bail!("connexion fermée par le pair"),
        }
    }
}
