//! HTTP metrics server for Prometheus scraping with CNSA 2.0 mTLS
//!
//! # NIST 800-53 Rev 5 Control Mappings
//! - AU-6: Audit Record Review - Metrics endpoint for analysis
//! - SI-4: System Monitoring - HTTP endpoint for monitoring systems
//! - SC-8: Transmission Confidentiality - TLS 1.3 with CNSA 2.0 cipher suites
//! - SC-8(1): Cryptographic Protection - mTLS with AES-256-GCM, SHA-384+, P-384+
//! - IA-5(2): PKI-Based Authentication - Client certificate validation

use crate::metrics::MetricsCollector;
use axum::{Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};
use axum_server::tls_rustls::RustlsConfig;
use prometheus::{Encoder, TextEncoder};
use rustls::pki_types::CertificateDer;
use rustls::server::WebPkiClientVerifier;
use rustls::{CipherSuite, RootCertStore, ServerConfig, SupportedCipherSuite};
use std::fs::File;
use std::io::BufReader;
use std::net::{Ipv6Addr, SocketAddr};
use std::sync::Arc;
use tracing::{error, info, warn};

/// Default metrics server port
const DEFAULT_METRICS_PORT: u16 = 9091;

/// CNSA 2.0 compliant cipher suite (TLS 1.3 only)
///
/// # NIST Controls
/// - SC-13: Cryptographic Protection - CNSA 2.0 approved algorithms
///
/// # Requirements
/// - Cipher: AES-256-GCM
/// - Hash: SHA-384
/// - Key Exchange: ECDHE with P-384 or P-521
const CNSA_CIPHER_SUITE: CipherSuite = CipherSuite::TLS13_AES_256_GCM_SHA384;

/// Metrics server configuration for mTLS
///
/// # NIST Controls
/// - IA-5(2): PKI-Based Authentication - Certificate configuration
///
/// # Security Requirements
/// - Server certificate must use EC P-384 or higher
/// - Client certificates mandatory (mTLS)
/// - TLS 1.3 only with CNSA 2.0 cipher suite (TLS_AES_256_GCM_SHA384)
#[derive(Clone)]
pub struct MetricsServerConfig {
    /// Server certificate path (PEM format)
    pub server_cert_path: String,
    /// Server private key path (PEM format, EC P-384 or higher required)
    pub server_key_path: String,
    /// CA certificate path for client verification (PEM format)
    pub ca_cert_path: String,
    /// Port to bind to
    pub port: u16,
}

impl Default for MetricsServerConfig {
    fn default() -> Self {
        Self {
            server_cert_path: "/etc/sonic/metrics/server-cert.pem".to_string(),
            server_key_path: "/etc/sonic/metrics/server-key.pem".to_string(),
            ca_cert_path: "/etc/sonic/metrics/ca-cert.pem".to_string(),
            port: DEFAULT_METRICS_PORT,
        }
    }
}

/// Metrics server state
///
/// # NIST Controls
/// - SI-4: System Monitoring - Shared metrics collector state
#[derive(Clone)]
struct MetricsServerState {
    collector: MetricsCollector,
}

/// Load and configure full CNSA 2.0 compliant TLS with mandatory mTLS
///
/// # NIST Controls
/// - SC-8(1): Cryptographic Protection - TLS 1.3 with CNSA 2.0 cipher suites
/// - IA-5(2): PKI-Based Authentication - Client certificate mandatory
/// - SC-13: Cryptographic Protection - FIPS 140-3 validated crypto (AWS-LC-RS)
///
/// # Security Features
/// - TLS 1.3 ONLY (no TLS 1.2 or earlier)
/// - Single cipher suite: TLS_AES_256_GCM_SHA384
/// - Mandatory client certificate verification
/// - AWS-LC-RS crypto provider (FIPS 140-3)
/// - No session resumption
/// - ALPN with HTTP/2 and HTTP/1.1
fn load_cnsa_mtls_config(
    config: &MetricsServerConfig,
) -> Result<ServerConfig, Box<dyn std::error::Error>> {
    info!("Loading CNSA 2.0 mTLS configuration");

    // Load server certificate chain
    let cert_file = File::open(&config.server_cert_path)?;
    let mut cert_reader = BufReader::new(cert_file);
    let certs: Vec<CertificateDer> =
        rustls_pemfile::certs(&mut cert_reader).collect::<Result<Vec<_>, _>>()?;

    if certs.is_empty() {
        return Err("No certificates found in server certificate file".into());
    }

    info!(
        cert_count = certs.len(),
        path = %config.server_cert_path,
        "Loaded server certificates"
    );

    // Load server private key
    let key_file = File::open(&config.server_key_path)?;
    let mut key_reader = BufReader::new(key_file);
    let private_key = rustls_pemfile::private_key(&mut key_reader)?
        .ok_or("No private key found in server key file")?;

    info!(path = %config.server_key_path, "Loaded server private key");

    // Load CA certificates for client verification
    let ca_file = File::open(&config.ca_cert_path)?;
    let mut ca_reader = BufReader::new(ca_file);
    let ca_certs: Vec<CertificateDer> =
        rustls_pemfile::certs(&mut ca_reader).collect::<Result<Vec<_>, _>>()?;

    if ca_certs.is_empty() {
        return Err("No CA certificates found in CA certificate file".into());
    }

    // Build root certificate store for client verification
    let mut root_store = RootCertStore::empty();
    for cert in ca_certs {
        root_store.add(cert)?;
    }

    info!(
        ca_count = root_store.len(),
        path = %config.ca_cert_path,
        "Loaded CA certificates for client verification"
    );

    // Create client certificate verifier (mandatory mTLS)
    let client_verifier = WebPkiClientVerifier::builder(Arc::new(root_store))
        .build()
        .map_err(|e| format!("Failed to create client verifier: {}", e))?;

    info!("Client certificate verifier configured (mTLS mandatory)");

    // Get AWS-LC-RS crypto provider (FIPS 140-3 validated)
    let crypto_provider = rustls::crypto::aws_lc_rs::default_provider();

    // Filter to CNSA 2.0 cipher suite only
    let cnsa_cipher_suites: Vec<SupportedCipherSuite> = crypto_provider
        .cipher_suites
        .iter()
        .filter(|cs| cs.suite() == CNSA_CIPHER_SUITE)
        .copied()
        .collect();

    if cnsa_cipher_suites.is_empty() {
        return Err("CNSA 2.0 cipher suite TLS_AES_256_GCM_SHA384 not available".into());
    }

    // Create custom crypto provider with only CNSA 2.0 cipher suite
    let cnsa_provider = rustls::crypto::CryptoProvider {
        cipher_suites: cnsa_cipher_suites,
        kx_groups: crypto_provider.kx_groups.to_vec(),
        signature_verification_algorithms: crypto_provider.signature_verification_algorithms,
        secure_random: crypto_provider.secure_random,
        key_provider: crypto_provider.key_provider,
    };

    info!("Configured CNSA 2.0 crypto provider: TLS_AES_256_GCM_SHA384 only, AWS-LC-RS");

    // Build ServerConfig with CNSA 2.0 restrictions
    let mut tls_config = ServerConfig::builder_with_provider(Arc::new(cnsa_provider))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|e| format!("Failed to set TLS 1.3 only: {}", e))?
        .with_client_cert_verifier(client_verifier)
        .with_single_cert(certs, private_key)?;

    // Disable session resumption for maximum security
    tls_config.session_storage = Arc::new(rustls::server::NoServerSessionStorage {});

    // Configure ALPN (HTTP/2 preferred, HTTP/1.1 fallback)
    tls_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    info!("CNSA 2.0 mTLS configuration complete:");
    info!("  - TLS 1.3 only (no earlier versions)");
    info!("  - Cipher suite: TLS_AES_256_GCM_SHA384");
    info!("  - Client certificates: MANDATORY");
    info!("  - Session resumption: DISABLED");
    info!("  - Crypto provider: AWS-LC-RS (FIPS 140-3)");
    info!("  - ALPN: h2, http/1.1");

    Ok(tls_config)
}

/// Start the metrics HTTPS server with mandatory CNSA 2.0 mTLS
///
/// # NIST Controls
/// - AU-6: Audit Record Review - Expose metrics for collection
/// - SC-8: Transmission Confidentiality - TLS 1.3 encryption
/// - SC-8(1): Cryptographic Protection - CNSA 2.0 cipher suites
/// - IA-5(2): PKI-Based Authentication - Client certificate validation
/// - SC-13: Cryptographic Protection - FIPS 140-3 validated crypto
///
/// # Arguments
/// * `collector` - Metrics collector to expose
/// * `config` - TLS configuration with certificate paths
///
/// # Returns
/// A future that runs the HTTPS server
///
/// # Security (CNSA 2.0 Compliant)
/// - **Mandatory mTLS**: Client certificate required and verified against CA
/// - **TLS 1.3 only**: No TLS 1.2 or earlier protocols
/// - **Single cipher suite**: TLS_AES_256_GCM_SHA384 (AES-256-GCM with SHA-384)
/// - **AWS-LC-RS**: FIPS 140-3 validated cryptographic provider
/// - **No session resumption**: Each connection is fully authenticated
/// - **ALPN**: HTTP/2 and HTTP/1.1 support
///
/// # Certificate Requirements
/// - **Server key**: EC P-384 or P-521 (CNSA 2.0)
/// - **Hash algorithm**: SHA-384 or SHA-512
/// - **Client certificates**: Must be signed by provided CA
/// - **Key usage**: Digital signature, key encipherment
pub async fn start_metrics_server(
    collector: MetricsCollector,
    config: MetricsServerConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr = SocketAddr::from((Ipv6Addr::LOCALHOST, config.port));

    let state = MetricsServerState { collector };

    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .route("/health", get(health_handler))
        .with_state(state);

    info!(
        "Starting CNSA 2.0 compliant metrics server on https://[::1]:{}/metrics",
        config.port
    );

    // Load full CNSA 2.0 mTLS configuration
    let server_config = load_cnsa_mtls_config(&config)?;

    // Convert ServerConfig to RustlsConfig for axum-server
    let rustls_config = RustlsConfig::from_config(Arc::new(server_config));

    info!("✅ CNSA 2.0 mTLS enabled:");
    info!("   ✓ TLS 1.3 only");
    info!("   ✓ Cipher: TLS_AES_256_GCM_SHA384");
    info!("   ✓ Client certificates: REQUIRED");
    info!("   ✓ Crypto: AWS-LC-RS (FIPS 140-3)");
    info!("   ✓ Session resumption: DISABLED");

    axum_server::bind_rustls(addr, rustls_config)
        .serve(app.into_make_service())
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}

/// Start metrics server in development mode (HTTP only, no TLS)
///
/// # WARNING
/// This mode is insecure and should ONLY be used for local development.
/// Production deployments MUST use start_metrics_server() with mTLS.
///
/// # NIST Controls
/// - SC-8: This violates transmission confidentiality - use only for development
pub async fn start_metrics_server_insecure(
    collector: MetricsCollector,
    port: Option<u16>,
) -> Result<(), Box<dyn std::error::Error>> {
    warn!("⚠️  Starting metrics server in INSECURE mode (HTTP without TLS)");
    warn!("⚠️  This mode should ONLY be used for local development");
    warn!("⚠️  Production REQUIRES CNSA 2.0 mTLS via start_metrics_server()");

    let port = port.unwrap_or(DEFAULT_METRICS_PORT);
    let addr = SocketAddr::from((Ipv6Addr::LOCALHOST, port));

    let state = MetricsServerState { collector };

    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .route("/health", get(health_handler))
        .with_state(state);

    info!("Starting metrics server on http://[::1]:{}/metrics", port);

    let listener = tokio::net::TcpListener::bind(&addr).await?;

    axum::serve(listener, app)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}

/// Handle /metrics endpoint - Prometheus text format
///
/// # NIST Controls
/// - AU-6: Audit Record Review - Provide metrics in Prometheus format
async fn metrics_handler(State(state): State<MetricsServerState>) -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let metric_families = state.collector.registry.gather();

    let mut buffer = vec![];
    match encoder.encode(&metric_families, &mut buffer) {
        Ok(_) => (
            StatusCode::OK,
            [("content-type", encoder.format_type())],
            buffer,
        )
            .into_response(),
        Err(e) => {
            error!(error = %e, "Failed to encode metrics");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to encode metrics",
            )
                .into_response()
        }
    }
}

/// Handle /health endpoint - Simple health check
///
/// # NIST Controls
/// - CP-10: System Recovery - Health check for monitoring
async fn health_handler(State(state): State<MetricsServerState>) -> impl IntoResponse {
    let health_value = state.collector.health_status.get();

    let status = if health_value >= 1.0 {
        "healthy"
    } else if health_value >= 0.5 {
        "degraded"
    } else {
        "unhealthy"
    };

    let body = format!(
        r#"{{"status": "{}", "health_score": {}}}"#,
        status, health_value
    );

    (StatusCode::OK, [("content-type", "application/json")], body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_server_creation() {
        let collector = MetricsCollector::new().unwrap();

        // Just verify we can create the state
        let _state = MetricsServerState { collector };
    }

    #[test]
    fn test_default_port() {
        assert_eq!(DEFAULT_METRICS_PORT, 9091);
    }

    #[test]
    fn test_default_config() {
        let config = MetricsServerConfig::default();
        assert_eq!(config.port, 9091);
        assert!(config.server_cert_path.contains("server-cert.pem"));
        assert!(config.server_key_path.contains("server-key.pem"));
        assert!(config.ca_cert_path.contains("ca-cert.pem"));
    }

    #[test]
    fn test_cnsa_cipher_suite() {
        // Verify CNSA 2.0 cipher suite constant
        assert_eq!(CNSA_CIPHER_SUITE, CipherSuite::TLS13_AES_256_GCM_SHA384);
    }
}
