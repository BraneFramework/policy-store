//  KID.rs
//    by Lut99
//
//  Created:
//    23 Oct 2024, 11:16:54
//  Last edited:
//    11 Nov 2024, 12:30:00
//  Auto updated?
//    Yes
//
//  Description:
//!   Implements a KID resolver.
//

use std::collections::HashMap;
use std::convert::Infallible;
use std::fs;
use std::path::{Path, PathBuf};

use base64ct::Encoding as _;
use http::StatusCode;
use jsonwebtoken::jwk::{AlgorithmParameters, JwkSet};
use jsonwebtoken::{DecodingKey, Header};
use specifications::authresolver::HttpError;
use thiserror::Error;
use tracing::{Level, debug, span, warn};

use super::KeyResolver;
use crate::KeyResolveErrorWrapper;


/***** ERRORS *****/
/// Defines the errors originating from the [`KidResolver`] which are the server's fault (poor bby).
#[derive(Debug, Error)]
pub enum ServerError {
    /// Failed to deserialize the keystore file.
    #[error("Failed to deserialize keystore file {:?}", path.display())]
    FileDeserialize {
        path: PathBuf,
        #[source]
        err:  serde_json::Error,
    },
    /// Failed to read the keystore to memory.
    #[error("Failed to read keystore file {:?}", path.display())]
    FileRead {
        path: PathBuf,
        #[source]
        err:  std::io::Error,
    },
    /// The given key was not valid Base64
    #[error("Given key {kid:?} in store file {:?} was not valid Base64", path.display())]
    KeyDecodeBase64 {
        path: PathBuf,
        kid:  String,
        #[source]
        err:  base64ct::Error,
    },
    /// The given key was in an unsupported format
    #[error("Given key {kid:?} in store file {:?} has an unsupported format (only octet keys are supported)", path.display())]
    KeyTypeUnsupprted { path: PathBuf, kid: String },
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
pub struct KidResolver {
    /// Maps key IDs to keys
    store: HashMap<String, DecodingKey>,
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
        let _span = span!(Level::INFO, "KidResolver::new");

        // Read the contents of the file
        let path: &Path = path.as_ref();
        let r = fs::read_to_string(path).map_err(|err| ServerError::FileRead { path: path.into(), err })?;
        let keyfile: JwkSet = serde_json::from_str(&r).map_err(|err| ServerError::FileDeserialize { path: path.into(), err })?;

        // Parse the keys as we go
        let mut store = HashMap::with_capacity(keyfile.keys.len());
        for (i, key) in keyfile.keys.into_iter().enumerate() {
            if let Some(id) = key.common.key_id {
                debug!("Key {:?}: {:?}", id, key.algorithm);

                // Get the encoded binary value
                let mut secret: [u8; 32] = [0; 32];
                if let AlgorithmParameters::OctetKey(oct) = &key.algorithm {
                    match base64ct::Base64Url::decode(&oct.value, &mut secret) {
                        Ok(val) => val,
                        Err(err) => return Err(ServerError::KeyDecodeBase64 { path: path.into(), kid: id, err }),
                    }
                } else {
                    return Err(ServerError::KeyTypeUnsupprted { path: path.into(), kid: id });
                };

                // Store it now
                if store.insert(id.clone(), DecodingKey::from_secret(&secret)).is_some() {
                    warn!("Found duplicate key with ID {id:?}");
                }
            } else {
                warn!("Skipping key {} in keyfile '{}' because it has no ID", i, path.display());
            }
        }
        debug!("Loaded {} key(s)", store.len());

        // Done
        Ok(Self { store })
    }
}
impl KeyResolver for KidResolver {
    type ClientError = ClientError;
    type ServerError = Infallible;


    async fn resolve_key(&self, header: &Header) -> Result<Result<DecodingKey, Self::ClientError>, Self::ServerError> {
        let _span = span!(Level::INFO, "KidResolver::resolve_key");

        // Unpack the key ID in the header
        let kid: &str = match header.kid.as_ref() {
            Some(kid) => kid,
            None => return Ok(Err(ClientError::HeaderKidNotFound)),
        };

        // Get the key
        match self.store.get(kid) {
            Some(key) => {
                debug!("Resolved key with ID {kid:?}");
                Ok(Ok(key.clone()))
            },
            None => Ok(Err(ClientError::UnknownKeyId { kid: kid.into() })),
        }
    }
}
