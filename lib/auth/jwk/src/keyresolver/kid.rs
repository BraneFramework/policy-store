//  KID.rs
//    by Lut99
//
//  Created:
//    23 Oct 2024, 11:16:54
//  Last edited:
//    11 Nov 2024, 11:56:00
//  Auto updated?
//    Yes
//
//  Description:
//!   Implements a KID resolver.
//

use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};

use base64ct::Encoding as _;
use http::StatusCode;
use jsonwebtoken::jwk::{AlgorithmParameters, Jwk, JwkSet};
use jsonwebtoken::{DecodingKey, Header};
use specifications::authresolver::HttpError;
use thiserror::Error;
use tracing::{Level, debug, span};

use super::KeyResolver;
use crate::KeyResolveErrorWrapper;


/***** ERRORS *****/
/// Defines the errors originating from the [`KidResolver`] which are the server's fault (poor bby).
#[derive(Debug, Error)]
pub enum ServerError {
    /// Failed to deserialize the keystore file.
    #[error("Failed to deserialize keystore file {:?}", path.display())]
    FileDeserialize { path: PathBuf, err: serde_json::Error },
    /// Failed to read the keystore to memory.
    #[error("Failed to read keystore file {:?}", path.display())]
    FileRead { path: PathBuf, err: std::io::Error },
    /// The given key was not valid Base64
    #[error("Given key {kid:?} was not valid Base64")]
    KeyDecodeBase64 { kid: String, err: base64ct::Error },
    /// The given key was in an unsupported format
    #[error("Given key {kid:?} has an unsupported format (only octet keys are supported)")]
    KeyTypeUnsupprted { kid: String },
}
impl From<ServerError> for crate::authresolver::ServerError {
    #[inline]
    fn from(value: ServerError) -> Self { Self::KeyResolve { err: Box::new(value) } }
}

/// Defines the errors originating from the [`KidResolver`] which are the client's fault (stupid
/// client)
#[derive(Debug, Error)]
pub enum ClientError {
    /// Missing Key ID field in the JWT header.
    #[error("Missing key ID field in given JWT header")]
    HeaderKidNotFound,
    /// The suggested key ID wasn't found in the given JWT.
    #[error("Unknown key with ID {kid:?}")]
    UnknownKeyId { kid: String },
}
impl HttpError for ClientError {
    #[inline]
    fn status_code(&self) -> StatusCode {
        use ClientError::*;
        match self {
            HeaderKidNotFound => StatusCode::BAD_REQUEST,
            UnknownKeyId { .. } => StatusCode::NOT_FOUND,
        }
    }
}
impl From<ClientError> for crate::authresolver::ClientError {
    #[inline]
    fn from(value: ClientError) -> Self { Self::KeyResolve { err: KeyResolveErrorWrapper(Box::new(value)) } }
}





/***** LIBRARY *****/
/// Resolves keys for the JWT by ID.
#[derive(Debug)]
pub struct KidResolver {
    jwk_store: JwkSet,
}
impl KidResolver {
    /// Constructor for the KidResolver.
    ///
    /// # Arguments
    /// - `path`: The path where the key set is stored on disk.
    ///
    /// # Returns
    /// A new KidResolver that can resolve keys by ID.
    ///
    /// # Errors
    /// This function can fail if it failed to read the file (e.g., it does not exist) or if it
    /// wasn't parsable as a JSON key set.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, ServerError> {
        let path: &Path = path.as_ref();
        let r = fs::read_to_string(path).map_err(|err| ServerError::FileRead { path: path.into(), err })?;
        let keyfile: JwkSet = serde_json::from_str(&r).map_err(|err| ServerError::FileDeserialize { path: path.into(), err })?;

        Ok(Self { jwk_store: keyfile })
    }
}
impl KeyResolver for KidResolver {
    type ClientError = ClientError;
    type ServerError = ServerError;


    fn resolve_key(&self, header: &Header) -> impl Send + Sync + Future<Output = Result<Result<DecodingKey, Self::ClientError>, Self::ServerError>> {
        async move {
            let _span = span!(Level::INFO, "KidResolver::resolve_key");

            // Unpack the key ID in the header
            let kid: &str = match header.kid.as_ref() {
                Some(kid) => kid,
                None => return Ok(Err(ClientError::HeaderKidNotFound)),
            };

            // Get the key
            debug!("Finding key with ID {kid:?}...");
            let key: &Jwk = match self.jwk_store.find(kid) {
                Some(key) => key,
                None => return Ok(Err(ClientError::UnknownKeyId { kid: kid.into() })),
            };
            debug!("Key ID {kid:?}: {:?}", key.algorithm);

            // Extract the secret from it
            let mut secret: Vec<u8> = Vec::new();
            if let AlgorithmParameters::OctetKey(oct) = &key.algorithm {
                match base64ct::Base64Url::decode(&oct.value, &mut secret) {
                    Ok(val) => val,
                    Err(err) => return Err(ServerError::KeyDecodeBase64 { kid: kid.into(), err }),
                }
            } else {
                return Err(ServerError::KeyTypeUnsupprted { kid: kid.into() });
            };

            // Now return that as decoding key
            Ok(Ok(DecodingKey::from_secret(&secret)))
        }
    }
}
