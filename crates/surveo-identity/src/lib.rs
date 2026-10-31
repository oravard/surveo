//! Identités et confiance (SPEC.md, chap. 6 ; IMPL.md, 6.1).
//!
//! - L'**établissement** a une paire de clés Ed25519 créée localement
//!   (CONF-1). Sa partie publique ([`EtabPublic`]) fonde la confiance de
//!   tous ses membres.
//! - Chaque **membre** (relais, console, agent) a sa paire de clés et un
//!   [`Cert`] {établissement, membre, rôle, salle, clé publique}.
//! - Un relais ou une console reçoit un certificat signé directement par
//!   l'établissement. Un **agent** signe lui-même le sien, au premier
//!   démarrage, avec la clé d'installation de son paquet de rattachement
//!   ([`Package`], INST-3), dont le certificat est signé par
//!   l'établissement et limité à une salle. Le poste rejoint ainsi son
//!   établissement et sa salle sans saisie et sans réseau.

use anyhow::{anyhow, bail, Context, Result};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::path::Path;
use surveo_protocol::{MemberId, PosteKind, RoomId};

/// Version du format des certificats et des paquets.
const FORMAT: u8 = 1;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Relay,
    Console,
    /// Clé d'un paquet d'installation : elle ne sert qu'à signer le
    /// certificat des agents de sa salle, jamais à se connecter.
    Installer,
    Agent,
}

/// Partie signée d'un certificat.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
struct CertBody<'a> {
    v: u8,
    etab: &'a str,
    member: &'a str,
    role: Role,
    room: Option<&'a str>,
    pubkey: &'a str,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Cert {
    pub v: u8,
    pub etab: String,
    pub member: MemberId,
    pub role: Role,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room: Option<RoomId>,
    pub pubkey: String,
    pub sig: String,
}

impl Cert {
    fn signed_bytes(&self) -> Vec<u8> {
        let body = CertBody {
            v: self.v,
            etab: &self.etab,
            member: &self.member,
            role: self.role,
            room: self.room.as_deref(),
            pubkey: &self.pubkey,
        };
        let mut m = b"surveo-cert-v1\0".to_vec();
        m.extend_from_slice(&serde_json::to_vec(&body).expect("sérialisation d'un certificat"));
        m
    }

    fn signed_by(&self, issuer: &VerifyingKey) -> Result<()> {
        issuer.verify(&self.signed_bytes(), &sig_from_hex(&self.sig)?).map_err(|_| anyhow!("signature du certificat invalide"))
    }

    fn issue(signer: &SigningKey, etab: &str, member: &str, role: Role, room: Option<&str>, key: &SigningKey) -> Self {
        let mut cert = Cert {
            v: FORMAT,
            etab: etab.into(),
            member: member.into(),
            role,
            room: room.map(Into::into),
            pubkey: hex::encode(key.verifying_key().to_bytes()),
            sig: String::new(),
        };
        cert.sig = hex::encode(signer.sign(&cert.signed_bytes()).to_bytes());
        cert
    }
}

/// Ce qu'un membre présente pour s'authentifier : son certificat et,
/// pour un agent, le certificat d'installation qui l'a signé.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Credential {
    pub cert: Cert,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via: Option<Cert>,
}

/// Un membre dont l'appartenance à l'établissement a été vérifiée.
#[derive(Clone, Debug)]
pub struct Verified {
    pub member: MemberId,
    pub role: Role,
    pub room: Option<RoomId>,
    pub key: VerifyingKey,
}

impl Credential {
    /// Vérifie que le membre appartient à l'établissement de confiance
    /// (CONF-3). Les motifs de refus sont en français : le relais les
    /// journalise et les transmet aux consoles.
    pub fn verify(&self, trust: &Trust) -> Result<Verified> {
        let cert = &self.cert;
        if cert.v != FORMAT {
            bail!("certificat d'un format inconnu ({})", cert.v);
        }
        if cert.etab != trust.public.id {
            bail!("membre d'un autre établissement ({})", cert.etab);
        }
        match &self.via {
            None => {
                cert.signed_by(&trust.key).map_err(|_| anyhow!("certificat non signé par l'établissement"))?;
                if cert.role == Role::Installer {
                    bail!("une clé d'installation ne peut pas se connecter");
                }
            }
            Some(installer) => {
                if installer.etab != trust.public.id || installer.role != Role::Installer {
                    bail!("certificat d'installation invalide");
                }
                installer
                    .signed_by(&trust.key)
                    .map_err(|_| anyhow!("paquet d'installation non signé par l'établissement"))?;
                if cert.role != Role::Agent {
                    bail!("un paquet d'installation ne peut certifier qu'un poste");
                }
                if cert.room != installer.room {
                    bail!("salle du poste différente de celle de son paquet");
                }
                cert.signed_by(&vk_from_hex(&installer.pubkey)?)
                    .map_err(|_| anyhow!("certificat du poste non signé par son paquet"))?;
            }
        }
        Ok(Verified { member: cert.member.clone(), role: cert.role, room: cert.room.clone(), key: vk_from_hex(&cert.pubkey)? })
    }
}

/// Partie publique de l'établissement, livrée avec chaque paquet (INST-3).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct EtabPublic {
    pub id: String,
    pub name: String,
    pub pubkey: String,
}

/// Ce qu'un membre sait de son établissement.
#[derive(Clone, Debug)]
pub struct Trust {
    pub public: EtabPublic,
    key: VerifyingKey,
}

impl Trust {
    pub fn new(public: EtabPublic) -> Result<Self> {
        let key = vk_from_hex(&public.pubkey)?;
        Ok(Self { public, key })
    }

    pub fn etab_id(&self) -> &str {
        &self.public.id
    }
}

/// Un membre : son certificat et sa clé privée.
#[derive(Clone)]
pub struct Identity {
    pub credential: Credential,
    key: SigningKey,
}

/// Forme enregistrée d'une [`Identity`].
#[derive(Serialize, Deserialize, Clone)]
pub struct IdentityFile {
    pub credential: Credential,
    pub secret: String,
}

impl Identity {
    pub fn member(&self) -> &str {
        &self.credential.cert.member
    }

    pub fn role(&self) -> Role {
        self.credential.cert.role
    }

    pub fn sign(&self, msg: &[u8]) -> String {
        hex::encode(self.key.sign(msg).to_bytes())
    }

    pub fn to_file(&self) -> IdentityFile {
        IdentityFile { credential: self.credential.clone(), secret: hex::encode(self.key.to_bytes()) }
    }

    pub fn from_file(f: &IdentityFile) -> Result<Self> {
        let key = sk_from_hex(&f.secret)?;
        if hex::encode(key.verifying_key().to_bytes()) != f.credential.cert.pubkey {
            bail!("clé privée sans rapport avec le certificat");
        }
        Ok(Self { credential: f.credential.clone(), key })
    }

    /// Identité d'un poste, signée par la clé d'installation de son paquet.
    pub fn enroll_agent(package: &Package, member: &str) -> Result<Self> {
        let installer = Identity::from_file(&package.installer)?;
        if installer.role() != Role::Installer {
            bail!("le paquet ne contient pas de clé d'installation");
        }
        let key = SigningKey::generate(&mut rand::rngs::OsRng);
        let room = installer.credential.cert.room.as_deref();
        let cert = Cert::issue(&installer.key, &package.etab.id, member, Role::Agent, room, &key);
        Ok(Self { credential: Credential { cert, via: Some(installer.credential.cert.clone()) }, key })
    }
}

/// Établissement : détient la clé qui signe les certificats (CONF-1).
pub struct Establishment {
    pub public: EtabPublic,
    key: SigningKey,
}

/// Forme enregistrée d'un [`Establishment`]. Contient la clé privée : à ne
/// jamais livrer dans un paquet.
#[derive(Serialize, Deserialize, Clone)]
pub struct EstablishmentFile {
    pub public: EtabPublic,
    pub secret: String,
}

impl Establishment {
    /// Crée un établissement, localement et sans réseau (CONF-1).
    pub fn create(name: &str) -> Self {
        let key = SigningKey::generate(&mut rand::rngs::OsRng);
        let id = format!("{}-{}", slug(name), hex::encode(rand::random::<[u8; 3]>()));
        let public = EtabPublic { id, name: name.into(), pubkey: hex::encode(key.verifying_key().to_bytes()) };
        Self { public, key }
    }

    pub fn trust(&self) -> Trust {
        Trust { public: self.public.clone(), key: self.key.verifying_key() }
    }

    pub fn to_file(&self) -> EstablishmentFile {
        EstablishmentFile { public: self.public.clone(), secret: hex::encode(self.key.to_bytes()) }
    }

    pub fn from_file(f: &EstablishmentFile) -> Result<Self> {
        let key = sk_from_hex(&f.secret)?;
        if hex::encode(key.verifying_key().to_bytes()) != f.public.pubkey {
            bail!("clé d'établissement incohérente");
        }
        Ok(Self { public: f.public.clone(), key })
    }

    /// Certificat d'un relais ou d'une console.
    pub fn issue(&self, member: &str, role: Role) -> Identity {
        let key = SigningKey::generate(&mut rand::rngs::OsRng);
        let cert = Cert::issue(&self.key, &self.public.id, member, role, None, &key);
        Identity { credential: Credential { cert, via: None }, key }
    }

    /// Paquet de rattachement d'une salle (INST-3).
    pub fn package(&self, room: &RoomId, room_name: &str, kind: PosteKind, relays: Vec<String>) -> Package {
        let key = SigningKey::generate(&mut rand::rngs::OsRng);
        let member = new_member_id("paquet");
        let cert = Cert::issue(&self.key, &self.public.id, &member, Role::Installer, Some(room), &key);
        let installer = Identity { credential: Credential { cert, via: None }, key };
        Package {
            v: FORMAT,
            etab: self.public.clone(),
            room: room.clone(),
            room_name: room_name.into(),
            kind,
            relays,
            installer: installer.to_file(),
        }
    }
}

/// Paquet de rattachement : ce qu'il faut à un agent pour rejoindre son
/// établissement et sa salle sans saisie (INST-2, INST-3).
#[derive(Serialize, Deserialize, Clone)]
pub struct Package {
    pub v: u8,
    pub etab: EtabPublic,
    pub room: RoomId,
    pub room_name: String,
    pub kind: PosteKind,
    /// Adresses du relais connues à la préparation du paquet, essayées
    /// avant la recherche sur le réseau local (DECOUV-1).
    #[serde(default)]
    pub relays: Vec<String>,
    pub installer: IdentityFile,
}

/// Identifiant aléatoire d'un membre, par exemple `poste-3f9a0c12d4e5`.
pub fn new_member_id(prefix: &str) -> MemberId {
    format!("{prefix}-{}", hex::encode(rand::random::<[u8; 6]>()))
}

/// Identifiant aléatoire court (salle, séance, commande…).
pub fn random_id() -> String {
    hex::encode(rand::random::<[u8; 8]>())
}

/// Nonce d'authentification.
pub fn new_nonce() -> String {
    hex::encode(rand::random::<[u8; 16]>())
}

pub fn verify_hex(key: &VerifyingKey, msg: &[u8], sig: &str) -> Result<()> {
    key.verify(msg, &sig_from_hex(sig)?).map_err(|_| anyhow!("signature invalide"))
}

fn slug(name: &str) -> String {
    let mut s = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        let c = match c {
            'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else if !s.ends_with('-') && !s.is_empty() {
            s.push('-');
        }
        if s.len() >= 24 {
            break;
        }
    }
    let s = s.trim_end_matches('-').to_string();
    if s.is_empty() { "etab".into() } else { s }
}

fn vk_from_hex(s: &str) -> Result<VerifyingKey> {
    let bytes: [u8; 32] = hex::decode(s)?.try_into().map_err(|_| anyhow!("clé publique invalide"))?;
    Ok(VerifyingKey::from_bytes(&bytes)?)
}

fn sk_from_hex(s: &str) -> Result<SigningKey> {
    let bytes: [u8; 32] = hex::decode(s)?.try_into().map_err(|_| anyhow!("clé privée invalide"))?;
    Ok(SigningKey::from_bytes(&bytes))
}

fn sig_from_hex(s: &str) -> Result<Signature> {
    let bytes: [u8; 64] = hex::decode(s)?.try_into().map_err(|_| anyhow!("signature invalide"))?;
    Ok(Signature::from_bytes(&bytes))
}

/// Lecture d'un fichier JSON.
pub fn load_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let text = std::fs::read_to_string(path).with_context(|| format!("lecture de {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("contenu de {}", path.display()))
}

/// Écriture d'un fichier JSON, par remplacement atomique. Un fichier
/// secret n'est lisible que par son propriétaire (Unix).
pub fn save_json<T: Serialize>(path: &Path, value: &T, secret: bool) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
    #[cfg(unix)]
    if secret {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    let _ = secret;
    std::fs::rename(&tmp, path).with_context(|| format!("écriture de {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn members_of_the_establishment_are_accepted() {
        let etab = Establishment::create("Lycée Jean Moulin");
        assert!(etab.public.id.starts_with("lycee-jean-moulin-"));
        let trust = etab.trust();
        let relay = etab.issue("relais-1", Role::Relay);
        let v = relay.credential.verify(&trust).unwrap();
        assert_eq!((v.role, v.room), (Role::Relay, None));

        let package = etab.package(&"b204".into(), "B204", PosteKind::Salle, vec![]);
        let agent = Identity::enroll_agent(&package, "poste-1").unwrap();
        let v = agent.credential.verify(&trust).unwrap();
        assert_eq!((v.member.as_str(), v.role, v.room.as_deref()), ("poste-1", Role::Agent, Some("b204")));

        let reloaded = Identity::from_file(&agent.to_file()).unwrap();
        assert_eq!(reloaded.member(), "poste-1");
    }

    #[test]
    fn foreign_or_forged_members_are_refused() {
        let etab = Establishment::create("A");
        let other = Establishment::create("B");
        let trust = etab.trust();

        let foreign = other.issue("console-x", Role::Console);
        let e = foreign.credential.verify(&trust).unwrap_err().to_string();
        assert!(e.contains("autre établissement"), "{e}");

        // Certificat recopié sous le nom de l'établissement, mais signé par un autre.
        let mut forged = foreign.credential.clone();
        forged.cert.etab = etab.public.id.clone();
        assert!(forged.verify(&trust).is_err());

        // Un poste qui change de salle invalide sa signature.
        let package = etab.package(&"b204".into(), "B204", PosteKind::Salle, vec![]);
        let mut agent = Identity::enroll_agent(&package, "poste-1").unwrap().credential;
        agent.cert.room = Some("c102".into());
        assert!(agent.verify(&trust).is_err());

        // La clé d'installation ne se connecte pas, et ne certifie pas une console.
        let installer = Identity::from_file(&package.installer).unwrap();
        assert!(installer.credential.verify(&trust).is_err());
        let console_key = SigningKey::generate(&mut rand::rngs::OsRng);
        let cert = Cert::issue(&installer.key, &etab.public.id, "console-y", Role::Console, Some("b204"), &console_key);
        let fake = Credential { cert, via: Some(installer.credential.cert.clone()) };
        assert!(fake.verify(&trust).unwrap_err().to_string().contains("ne peut certifier qu'un poste"));
    }
}
