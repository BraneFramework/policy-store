//  AUTHRESOLVER.rs
//    by Lut99
//
//  Created:
//    23 Oct 2024, 10:37:53
//  Last edited:
//    23 Oct 2024, 16:30:04
//  Auto updated?
//    Yes
//
//  Description:
//!   Provides the actual [`AuthResolver`] implementation.
//

use std::collections::HashMap;
use std::error::Error;
use std::future::Future;

use http::header::AUTHORIZATION;
use http::{HeaderMap, HeaderValue, StatusCode};
use jsonwebtoken::{Header, Validation};
use specifications::metadata::User;
use specifications::AuthResolver;
use thiserror::Error;
use tracing::{debug, info, span, Level};

use crate::keyresolver::KeyResolver;


/***** ERRORS *****/
/// Represents server-side errors which the client can't fix.
#[derive(Debug, Error)]
pub enum ServerError {
    /// The embedded [`KeyResolver`] failed to resolve a key due to some server-side error.
    #[error("Failed to resolve key")]
    KeyResolve { err: Box<dyn 'static + Error> },
}

/// Represents client-side errors which the server can't fix.
#[derive(Debug, Error)]
pub enum ClientError {
    /// The given 'Authorization'-header did not contain valid UTF-8.
    #[error("Value of header {header:?} in request is non-UTF-8")]
    AuthHeaderNonUtf8 {
        header: &'static str,
        #[source]
        err:    http::header::ToStrError,
    },
    /// No 'Authorization' header found in request.
    #[error("Missing header {header:?} in ")]
    AuthHeaderNotFound { header: &'static str },
    /// The JWT extracted from the 'Authorization'-header was not a valid JWT.
    #[error("Illegal JWT {raw:?} in header {header:?} in request")]
    IllegalJwt {
        header: &'static str,
        raw:    String,
        #[source]
        err:    jsonwebtoken::errors::Error,
    },
    /// The JWT initiator claim had an invalid type.
    #[error("JWT initiator claim {claim:?} in header {header:?} has an invalid type: only strings and integers allowed (value: {value:?})")]
    JwtIllegalType { header: &'static str, claim: String, value: String },
    /// The JWT did not have the initiator claim we're looking for.
    #[error("Initiator claim {claim:?} not found in JWT in header {header:?}")]
    JwtMissingInitiatorClaim { header: &'static str, claim: String },
    /// Failed to validate the JWT in the given header.
    #[error("Failed to validate JWT in header {header:?}")]
    JwtValidate {
        header: &'static str,
        #[source]
        err:    jsonwebtoken::errors::Error,
    },
    /// The embedded [`KeyResolver`] failed to resolve a key due to some client-side error.
    #[error("Failed to resolve key")]
    KeyResolve { err: Box<dyn 'static + specifications::authresolver::ClientError> },
    /// The given 'Authorization'-header was missing the 'Bearer '-part.
    #[error("Missing \"Bearer \" in header {header:?} in request (raw value: {raw:?})")]
    MissingBearer { header: &'static str, raw: String },
}
impl specifications::authresolver::ClientError for ClientError {
    #[inline]
    fn status_code(&self) -> StatusCode {
        use ClientError::*;
        match self {
            AuthHeaderNonUtf8 { .. }
            | AuthHeaderNotFound { .. }
            | IllegalJwt { .. }
            | JwtIllegalType { .. }
            | JwtMissingInitiatorClaim { .. }
            | MissingBearer { .. } => StatusCode::BAD_REQUEST,
            JwtValidate { .. } => StatusCode::UNAUTHORIZED,
            KeyResolve { err } => err.status_code(),
        }
    }
}





/***** HELPER FUNCTIONS *****/
/// Given a (potentially present) `Auth`-header, attempts to extract the JWT from it.
///
/// # Arguments
/// - `name`: The name of the Authorization header. Only used for debugging in this function.
/// - `value`: The [`HeaderValue`] representing what is in the header (or [`None`]) if it isn't
///   present!).
///
/// # Returns
/// A [`String`] representation of the token.
///
/// # Errors
/// This function may error if the header isn't present, or doesn't bear a valid token (e.g.,
/// missing "Bearer" in the token field).
fn extract_jwt<'h>(name: &'static str, value: Option<&'h HeaderValue>) -> Result<&'h str, ClientError> {
    // Get the header value as a string
    let header_val: &str = match value {
        Some(v) => match v.to_str() {
            Ok(v) => v,
            Err(err) => return Err(ClientError::AuthHeaderNonUtf8 { header: name, err }),
        },
        None => {
            return Err(ClientError::AuthHeaderNotFound { header: name });
        },
    };

    // Split on the bearer thingy
    if header_val.len() < 7 || &header_val[..7] != "Bearer " {
        return Err(ClientError::MissingBearer { header: name, raw: header_val.into() });
    }

    // OK, let's go
    Ok(&header_val[7..])
}





/***** LIBRARY *****/
/// Authorizes HTTP requests by finding JWKs in the headers.
#[derive(Debug)]
pub struct JwkResolver<K> {
    /// Determines which JWT claims we check to find the user in question.
    initiator_claim: String,
    /// The keystore that we use to verify JWTs
    resolver: K,
}
impl<K> JwkResolver<K> {
    /// Constructor for the JwkResolver.
    ///
    /// # Arguments
    /// - `initiator_claim`: The name of the claim that we use to read the user ID.
    /// - `resolver`: Something implementing [`KeyResolver`] that resolves JWT headers to
    ///   appropriate keys for validation.
    ///
    /// # Returns
    /// A new instance of Self, ready to rumble.
    #[inline]
    pub fn new(initiator_claim: impl Into<String>, resolver: K) -> Self { Self { initiator_claim: initiator_claim.into(), resolver } }
}
impl<K> AuthResolver for JwkResolver<K>
where
    K: KeyResolver,
    ClientError: From<K::ClientError>,
    ServerError: From<K::ServerError>,
{
    type Context = User;
    type ClientError = ClientError;
    type ServerError = ServerError;


    fn authorize(
        &self,
        headers: &HeaderMap<HeaderValue>,
    ) -> impl Future<Output = Result<Result<Self::Context, Self::ClientError>, Self::ServerError>> {
        async move {
            let _span = span!(Level::INFO, "JwkResolver::authorize");
            info!("Handling JWT authentication for incoming request");

            // Fetch the JWT from the header
            let raw_jwt = match extract_jwt(AUTHORIZATION.as_str(), headers.get(AUTHORIZATION.as_str())) {
                Ok(jwt) => jwt,
                Err(err) => return Ok(Err(err)),
            };
            debug!("Received JWT: {raw_jwt:?}");

            // Fetch the header from the JWT
            let header: Header = match jsonwebtoken::decode_header(&raw_jwt).map_err(|err| ClientError::IllegalJwt {
                header: AUTHORIZATION.as_str(),
                raw: raw_jwt.into(),
                err,
            }) {
                Ok(header) => header,
                Err(err) => return Ok(Err(err)),
            };
            debug!("JWT header: {header:?}");

            // Check if the key makes sense
            debug!("Resolving key in keystore...");
            let decoding_key = match self.resolver.resolve_key(&header).await? {
                Ok(key) => key,
                Err(err) => return Ok(Err(err.into())),
            };
            let validation = Validation::new(header.alg);
            debug!("Validating JWT with {:?}...", header.alg);
            let result = match jsonwebtoken::decode::<HashMap<String, serde_json::Value>>(&raw_jwt, &decoding_key, &validation) {
                Ok(res) => res,
                Err(err) => return Ok(Err(ClientError::JwtValidate { header: AUTHORIZATION.as_str(), err })),
            };
            debug!("Validating OK");

            match result.claims.get(&self.initiator_claim) {
                Some(initiator) => match initiator {
                    serde_json::Value::Number(v) => Ok(Ok(User { id: v.to_string(), name: "John Smith".into() })),
                    serde_json::Value::String(v) => Ok(Ok(User { id: v.clone(), name: "John Smith".into() })),
                    other => Ok(Err(ClientError::JwtIllegalType {
                        header: AUTHORIZATION.as_str(),
                        claim:  self.initiator_claim.clone(),
                        value:  format!("{other:?}"),
                    })),
                },
                None => Ok(Err(ClientError::JwtMissingInitiatorClaim { header: AUTHORIZATION.as_str(), claim: self.initiator_claim.clone() })),
            }
        }
    }
}
