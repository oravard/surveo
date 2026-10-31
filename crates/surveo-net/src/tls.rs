//! TLS : le relais présente un certificat auto-signé, engendré à son
//! démarrage ; le client ne le fait pas vérifier par une autorité. La
//! confiance vient de l'authentification par la clé d'établissement, liée
//! à la session TLS par un export de clé (IMPL.md, 6.1).

use anyhow::Result;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use std::sync::Arc;

/// Nom TLS du relais, attendu par les clients.
pub const TLS_NAME: &str = "surveo-relay";

/// Étiquette de l'export de clé TLS sur lequel portent les signatures de
/// l'authentification.
pub const EKM_LABEL: &[u8] = b"EXPORTER-surveo-v1";

fn provider() -> Arc<rustls::crypto::CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

pub fn server_acceptor() -> Result<tokio_rustls::TlsAcceptor> {
    let ck = rcgen::generate_simple_self_signed(vec![TLS_NAME.to_string()])?;
    let cert = ck.cert.der().clone();
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(ck.key_pair.serialize_der()));
    let cfg = rustls::ServerConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)?;
    Ok(tokio_rustls::TlsAcceptor::from(Arc::new(cfg)))
}

/// Les signatures de la négociation TLS restent vérifiées ; seul le
/// rattachement du certificat à une autorité ne l'est pas.
pub fn client_connector() -> Result<tokio_rustls::TlsConnector> {
    let cfg = rustls::ClientConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(EstablishmentBound))
        .with_no_client_auth();
    Ok(tokio_rustls::TlsConnector::from(Arc::new(cfg)))
}

pub fn server_name() -> ServerName<'static> {
    ServerName::try_from(TLS_NAME).expect("nom TLS valide").to_owned()
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
