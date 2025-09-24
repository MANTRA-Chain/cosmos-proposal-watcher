#![forbid(unsafe_code)]
#![deny(
    rust_2018_idioms,
    trivial_casts,
    unused_lifetimes,
    unused_qualifications
)]

#[cfg(test)]
#[allow(unused_imports)]
#[macro_use]
extern crate more_asserts;

pub mod config;
pub mod error;
pub mod incidentio;
pub mod slack;
pub mod worker;

pub const DEFAULT_CONFIG_PATH: &str = "chains.toml";

/// Helper function to create a gRPC client.
pub async fn create_grpc_client<T>(
    grpc_addr: tonic::transport::Uri,
    client_constructor: impl FnOnce(tonic::transport::Channel) -> T,
) -> Result<T, error::Error> {
    let tls_config = tonic::transport::ClientTlsConfig::new().with_native_roots();
    let channel = tonic::transport::Channel::builder(grpc_addr)
        .tls_config(tls_config)
        .map_err(error::Error::grpc_transport)?
        .connect()
        .await
        .map_err(error::Error::grpc_transport)?;
    Ok(client_constructor(channel))
}

/// Initialize the crypto provider for TLS
pub fn init_crypto_provider() {
    // Install the default crypto provider (aws_lc_rs) if not already installed
    // The error is ignored because it returns an error if already installed
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
}
