//! Authentification mutuelle à la connexion (IMPL.md, 6.1), après la
//! négociation TLS :
//! 1. le relais envoie sa version, son certificat et un défi ;
//! 2. le client vérifie le certificat du relais, puis renvoie sa version,
//!    son certificat, son propre défi et sa signature du défi du relais ;
//! 3. le relais vérifie le client et signe le défi du client ;
//! 4. le client vérifie cette signature.
//!
//! Chaque signature porte aussi sur une valeur exportée de la session TLS :
//! un intermédiaire qui ouvrirait deux sessions TLS ne pourrait pas faire
//! accepter une signature d'une session sur l'autre.

use crate::tls::{self, EKM_LABEL};
use crate::{recv_json, send_json, ClientWs, Refused, ServerWs};
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use surveo_identity::{new_nonce, verify_hex, Credential, Identity, Role, Trust, Verified};
use surveo_protocol::{AgentInfo, PROTOCOL_VERSION};
use tokio::net::TcpStream;

/// Délai d'établissement d'une connexion TCP.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
/// Délai de l'authentification, TLS compris.
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "t", rename_all = "snake_case")]
enum Handshake {
    Hello { v: u16, credential: Credential, nonce: String },
    Auth {
        v: u16,
        credential: Credential,
        nonce: String,
        sig: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        info: Option<AgentInfo>,
    },
    AuthOk { sig: String },
    Denied { reason: String },
}

fn auth_message(side: &str, ekm: &[u8], first: &str, second: &str) -> Vec<u8> {
    let mut m = format!("surveo-auth-v1|{side}|").into_bytes();
    m.extend_from_slice(ekm);
    m.extend_from_slice(format!("|{first}|{second}").as_bytes());
    m
}

/// Connexion d'un client au relais `addr`, avec authentification mutuelle.
/// Un agent joint sa description ([`AgentInfo`]).
pub async fn connect(addr: &str, me: &Identity, trust: &Trust, info: Option<&AgentInfo>) -> Result<(ClientWs, Verified)> {
    tokio::time::timeout(HANDSHAKE_TIMEOUT, connect_inner(addr, me, trust, info))
        .await
        .map_err(|_| anyhow!("pas de réponse du relais {addr}"))?
}

async fn connect_inner(addr: &str, me: &Identity, trust: &Trust, info: Option<&AgentInfo>) -> Result<(ClientWs, Verified)> {
    let tcp = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(addr))
        .await
        .map_err(|_| anyhow!("relais {addr} injoignable"))?
        .with_context(|| format!("connexion à {addr}"))?;
    tcp.set_nodelay(true)?;
    let tls_stream = tls::client_connector()?.connect(tls::server_name(), tcp).await?;
    let ekm: [u8; 32] = tls_stream.get_ref().1.export_keying_material([0u8; 32], EKM_LABEL, None)?;
    let (mut ws, _) = tokio_tungstenite::client_async("ws://surveo-relay/", tls_stream).await?;

    let (relay_cred, relay_nonce) = match recv_json(&mut ws).await? {
        Handshake::Hello { v, credential, nonce } => {
            if v != PROTOCOL_VERSION {
                bail!(Refused(format!("relais d'une autre version du protocole ({v}, attendu {PROTOCOL_VERSION})")));
            }
            (credential, nonce)
        }
        Handshake::Denied { reason } => bail!(Refused(reason)),
        other => bail!("le relais n'a pas ouvert l'authentification ({other:?})"),
    };
    let relay = relay_cred.verify(trust).map_err(|e| Refused(format!("relais non reconnu : {e}")))?;
    if relay.role != Role::Relay {
        bail!(Refused("le pair n'est pas un relais".into()));
    }

    let my_nonce = new_nonce();
    let sig = me.sign(&auth_message("client", &ekm, &relay_nonce, &my_nonce));
    let auth = Handshake::Auth {
        v: PROTOCOL_VERSION,
        credential: me.credential.clone(),
        nonce: my_nonce.clone(),
        sig,
        info: info.cloned(),
    };
    send_json(&mut ws, &auth).await?;

    match recv_json(&mut ws).await? {
        Handshake::AuthOk { sig } => {
            verify_hex(&relay.key, &auth_message("relay", &ekm, &my_nonce, &relay_nonce), &sig)
                .map_err(|_| Refused("signature du relais invalide".into()))?;
        }
        Handshake::Denied { reason } => bail!(Refused(reason)),
        other => bail!("réponse inattendue du relais : {other:?}"),
    }
    Ok((ws, relay))
}

/// Un client authentifié par le relais.
pub struct Accepted {
    pub ws: ServerWs,
    pub peer: Verified,
    pub info: Option<AgentInfo>,
}

/// Côté relais : TLS, WebSocket et authentification d'un client. Un refus
/// est signalé au client avant de fermer, et rendu sous la forme d'une
/// erreur [`Refused`] portant le membre et son motif.
pub async fn accept(tcp: TcpStream, acceptor: &tokio_rustls::TlsAcceptor, me: &Identity, trust: &Trust) -> Result<Accepted> {
    tokio::time::timeout(HANDSHAKE_TIMEOUT, accept_inner(tcp, acceptor, me, trust))
        .await
        .map_err(|_| anyhow!("authentification trop longue"))?
}

async fn accept_inner(tcp: TcpStream, acceptor: &tokio_rustls::TlsAcceptor, me: &Identity, trust: &Trust) -> Result<Accepted> {
    tcp.set_nodelay(true)?;
    let tls_stream = acceptor.accept(tcp).await?;
    let ekm: [u8; 32] = tls_stream.get_ref().1.export_keying_material([0u8; 32], EKM_LABEL, None)?;
    let mut ws = tokio_tungstenite::accept_async(tls_stream).await?;

    let my_nonce = new_nonce();
    send_json(&mut ws, &Handshake::Hello { v: PROTOCOL_VERSION, credential: me.credential.clone(), nonce: my_nonce.clone() })
        .await?;
    let Handshake::Auth { v, credential, nonce, sig, info } = recv_json(&mut ws).await? else {
        bail!("le client n'a pas répondu au défi");
    };
    let who = format!("{} ({})", credential.cert.member, credential.cert.etab);
    let checked = (|| {
        if v != PROTOCOL_VERSION {
            bail!("version du protocole incompatible ({v}, attendu {PROTOCOL_VERSION})");
        }
        let peer = credential.verify(trust)?;
        verify_hex(&peer.key, &auth_message("client", &ekm, &my_nonce, &nonce), &sig)
            .map_err(|_| anyhow!("signature du client invalide"))?;
        match peer.role {
            Role::Console => {}
            Role::Agent if info.is_some() => {}
            Role::Agent => bail!("poste sans description"),
            _ => bail!("rôle non admis comme client"),
        }
        Ok(peer)
    })();
    match checked {
        Ok(peer) => {
            let reply = me.sign(&auth_message("relay", &ekm, &nonce, &my_nonce));
            send_json(&mut ws, &Handshake::AuthOk { sig: reply }).await?;
            Ok(Accepted { ws, peer, info })
        }
        Err(e) => {
            let _ = send_json(&mut ws, &Handshake::Denied { reason: e.to_string() }).await;
            let _ = ws.close(None).await;
            Err(Refused(format!("{who} : {e}")).into())
        }
    }
}
