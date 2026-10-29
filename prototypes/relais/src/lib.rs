//! Prototype 1 de la phase 2 (IMPL.md) : relais, agents et console simulés.
//!
//! Code jetable : il sert à mesurer et à valider des choix, pas à être
//! repris tel quel. Ce module contient ce qui est commun aux binaires :
//! certificats d'établissement, authentification mutuelle liée à la
//! session TLS, messages de contrôle et petites mesures.

use anyhow::{anyhow, bail, Context, Result};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use futures_util::{SinkExt, StreamExt};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

/// Port UDP sur lequel le relais s'annonce (SPEC.md, DECOUV-1, niveau 3).
pub const DISCOVERY_PORT: u16 = 47823;
/// Étiquette de l'export de clé TLS qui lie l'authentification à la session.
pub const EKM_LABEL: &[u8] = b"EXPORTER-surveo-proto-v0";
/// Nom TLS attendu du relais. Le certificat TLS est auto-signé : la
/// confiance vient de l'authentification par la clé d'établissement.
pub const TLS_NAME: &str = "surveo-relay";

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Relay,
    Console,
    Agent,
}

/// Certificat de membre, signé par la clé d'établissement (SPEC.md, CONF-1 à CONF-3).
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Cert {
    pub etab: String,
    pub member: String,
    pub role: Role,
    pub pubkey: String,
    pub sig: String,
}

impl Cert {
    fn signed_bytes(etab: &str, member: &str, role: Role, pubkey: &str) -> Vec<u8> {
        format!("surveo-cert-v0|{etab}|{member}|{role:?}|{pubkey}").into_bytes()
    }

    /// Vérifie que le certificat a été émis par l'établissement de
    /// confiance, et renvoie la clé publique du membre.
    pub fn verify(&self, trust: &Trust) -> Result<VerifyingKey> {
        if self.etab != trust.etab_id {
            bail!("membre d'un autre établissement ({})", self.etab);
        }
        let sig = sig_from_hex(&self.sig)?;
        trust
            .etab_pk
            .verify(&Self::signed_bytes(&self.etab, &self.member, self.role, &self.pubkey), &sig)
            .map_err(|_| anyhow!("certificat non signé par l'établissement"))?;
        vk_from_hex(&self.pubkey)
    }
}

/// Partie publique de l'établissement, livrée avec le paquet d'installation (INST-3).
#[derive(Serialize, Deserialize)]
pub struct EtabFile {
    pub id: String,
    pub pubkey: String,
}

/// Identité d'un membre : son certificat et sa clé privée.
#[derive(Serialize, Deserialize, Clone)]
pub struct IdentityFile {
    pub cert: Cert,
    pub secret: String,
}

pub struct Identity {
    pub cert: Cert,
    pub key: SigningKey,
}

impl Identity {
    pub fn from_file(f: &IdentityFile) -> Result<Self> {
        let bytes: [u8; 32] = hex::decode(&f.secret)?
            .try_into()
            .map_err(|_| anyhow!("clé privée de taille invalide"))?;
        Ok(Self { cert: f.cert.clone(), key: SigningKey::from_bytes(&bytes) })
    }
}

/// Ce qu'un membre sait de son établissement : son identifiant et sa clé publique.
pub struct Trust {
    pub etab_id: String,
    pub etab_pk: VerifyingKey,
}

impl Trust {
    pub fn from_file(f: &EtabFile) -> Result<Self> {
        Ok(Self { etab_id: f.id.clone(), etab_pk: vk_from_hex(&f.pubkey)? })
    }
}

/// Établissement : détient la clé qui signe les certificats des membres.
pub struct Establishment {
    pub id: String,
    pub key: SigningKey,
}

impl Establishment {
    pub fn generate(id: &str) -> Self {
        Self { id: id.to_string(), key: SigningKey::generate(&mut rand::rngs::OsRng) }
    }

    pub fn public(&self) -> EtabFile {
        EtabFile { id: self.id.clone(), pubkey: hex::encode(self.key.verifying_key().to_bytes()) }
    }

    pub fn issue(&self, member: &str, role: Role) -> IdentityFile {
        let key = SigningKey::generate(&mut rand::rngs::OsRng);
        let pubkey = hex::encode(key.verifying_key().to_bytes());
        let sig = self.key.sign(&Cert::signed_bytes(&self.id, member, role, &pubkey));
        IdentityFile {
            cert: Cert {
                etab: self.id.clone(),
                member: member.to_string(),
                role,
                pubkey,
                sig: hex::encode(sig.to_bytes()),
            },
            secret: hex::encode(key.to_bytes()),
        }
    }
}

pub fn vk_from_hex(s: &str) -> Result<VerifyingKey> {
    let bytes: [u8; 32] = hex::decode(s)?.try_into().map_err(|_| anyhow!("clé publique invalide"))?;
    Ok(VerifyingKey::from_bytes(&bytes)?)
}

pub fn sig_from_hex(s: &str) -> Result<Signature> {
    let bytes: [u8; 64] = hex::decode(s)?.try_into().map_err(|_| anyhow!("signature invalide"))?;
    Ok(Signature::from_bytes(&bytes))
}

pub fn load_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let text = std::fs::read_to_string(path).with_context(|| format!("lecture de {}", path.display()))?;
    Ok(serde_json::from_str(&text)?)
}

pub fn save_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    std::fs::write(path, serde_json::to_string_pretty(value)?)?;
    Ok(())
}

/// Messages de contrôle, échangés en trames texte JSON. Les miniatures
/// circulent à part, en trames binaires.
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Ctl {
    /// Relais → client, à l'ouverture : son certificat et un défi.
    Hello { cert: Cert, nonce: String },
    /// Client → relais : son certificat, son défi, et sa signature du défi du relais.
    Auth { cert: Cert, nonce: String, sig: String },
    /// Relais → client : sa signature du défi du client.
    AuthOk { sig: String },
    Denied { reason: String },
    /// Console → relais → agents.
    Command { id: u64, action: String },
    /// Agent → relais → consoles.
    Ack { id: u64, agent: String },
    /// Relais → consoles : nombre d'agents connectés.
    Presence { agents: usize },
}

/// Message signé pendant l'authentification. Il inclut l'export de clé
/// TLS (`ekm`) : un relais intermédiaire, qui aurait deux sessions TLS
/// distinctes, ne pourrait pas faire accepter une signature d'une session
/// sur l'autre.
pub fn auth_message(side: &str, ekm: &[u8], first: &str, second: &str) -> Vec<u8> {
    let mut m = format!("surveo-auth-v0|{side}|").into_bytes();
    m.extend_from_slice(ekm);
    m.extend_from_slice(format!("|{first}|{second}").as_bytes());
    m
}

pub fn new_nonce() -> String {
    hex::encode(rand::random::<[u8; 16]>())
}

fn provider() -> Arc<rustls::crypto::CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

/// Configuration TLS du relais, avec un certificat auto-signé généré au démarrage.
pub fn server_tls() -> Result<tokio_rustls::TlsAcceptor> {
    let ck = rcgen::generate_simple_self_signed(vec![TLS_NAME.to_string()])?;
    let cert = ck.cert.der().clone();
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(ck.key_pair.serialize_der()));
    let cfg = rustls::ServerConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)?;
    Ok(tokio_rustls::TlsAcceptor::from(Arc::new(cfg)))
}

/// Configuration TLS des clients. Le certificat du relais n'est pas
/// vérifié par une autorité : c'est l'authentification par la clé
/// d'établissement, liée à la session TLS, qui établit la confiance.
/// Les signatures de la négociation TLS restent vérifiées.
pub fn client_tls() -> Result<tokio_rustls::TlsConnector> {
    let cfg = rustls::ClientConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(EstablishmentBound))
        .with_no_client_auth();
    Ok(tokio_rustls::TlsConnector::from(Arc::new(cfg)))
}

#[derive(Debug)]
struct EstablishmentBound;

impl ServerCertVerifier for EstablishmentBound {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &provider().signature_verification_algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &provider().signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        provider().signature_verification_algorithms.supported_schemes()
    }
}

pub async fn send_ctl<S>(ws: &mut WebSocketStream<S>, c: &Ctl) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    ws.send(Message::Text(serde_json::to_string(c)?.into())).await?;
    Ok(())
}

pub async fn recv_ctl<S>(ws: &mut WebSocketStream<S>) -> Result<Ctl>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    loop {
        match ws.next().await {
            Some(Ok(Message::Text(t))) => return Ok(serde_json::from_str(t.as_str())?),
            Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => continue,
            Some(Ok(m)) => bail!("trame inattendue pendant l'authentification : {m:?}"),
            Some(Err(e)) => return Err(e.into()),
            None => bail!("connexion fermée par le pair"),
        }
    }
}

pub type ClientWs = WebSocketStream<tokio_rustls::client::TlsStream<TcpStream>>;

/// Connexion d'un client (agent ou console) au relais, avec
/// authentification mutuelle par la clé d'établissement.
pub async fn connect(
    addr: &str,
    tls: &tokio_rustls::TlsConnector,
    me: &Identity,
    trust: &Trust,
) -> Result<ClientWs> {
    let tcp = TcpStream::connect(addr).await.with_context(|| format!("connexion à {addr}"))?;
    tcp.set_nodelay(true)?;
    let tls_stream = tls.connect(ServerName::try_from(TLS_NAME)?.to_owned(), tcp).await?;
    let ekm: [u8; 32] = tls_stream.get_ref().1.export_keying_material([0u8; 32], EKM_LABEL, None)?;
    let (mut ws, _) = tokio_tungstenite::client_async("ws://surveo-relay/", tls_stream).await?;

    let Ctl::Hello { cert: relay_cert, nonce: relay_nonce } = recv_ctl(&mut ws).await? else {
        bail!("le relais n'a pas ouvert l'authentification");
    };
    if relay_cert.role != Role::Relay {
        bail!("le pair n'est pas un relais");
    }
    let relay_pk = relay_cert.verify(trust).context("certificat du relais refusé")?;

    let my_nonce = new_nonce();
    let sig = me.key.sign(&auth_message("client", &ekm, &relay_nonce, &my_nonce));
    send_ctl(
        &mut ws,
        &Ctl::Auth { cert: me.cert.clone(), nonce: my_nonce.clone(), sig: hex::encode(sig.to_bytes()) },
    )
    .await?;

    match recv_ctl(&mut ws).await? {
        Ctl::AuthOk { sig } => {
            relay_pk
                .verify(&auth_message("relay", &ekm, &my_nonce, &relay_nonce), &sig_from_hex(&sig)?)
                .map_err(|_| anyhow!("signature du relais invalide"))?;
        }
        Ctl::Denied { reason } => bail!("refusé par le relais : {reason}"),
        other => bail!("réponse inattendue du relais : {other:?}"),
    }
    Ok(ws)
}

/// Trouve le relais par son annonce sur le réseau local (DECOUV-1, niveau 3).
pub async fn discover(timeout: std::time::Duration) -> Result<String> {
    let sock = tokio::net::UdpSocket::bind(("0.0.0.0", DISCOVERY_PORT))
        .await
        .context("écoute des annonces du relais")?;
    let mut buf = [0u8; 1024];
    let (n, from) = tokio::time::timeout(timeout, sock.recv_from(&mut buf))
        .await
        .map_err(|_| anyhow!("aucune annonce de relais reçue"))??;
    let v: serde_json::Value = serde_json::from_slice(&buf[..n])?;
    let port = v["port"].as_u64().ok_or_else(|| anyhow!("annonce sans port"))?;
    Ok(format!("{}:{}", from.ip(), port))
}

/// Centile d'une série de durées en millisecondes (la série est triée sur place).
pub fn percentile(values: &mut [f64], p: f64) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let idx = ((p / 100.0) * (values.len() - 1) as f64).round() as usize;
    values[idx]
}

/// Lecture minimale des arguments `--nom valeur`.
pub fn arg(name: &str) -> Option<String> {
    let mut it = std::env::args();
    while let Some(a) = it.next() {
        if a == name {
            return it.next();
        }
    }
    None
}
