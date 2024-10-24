//  KEYRESOLVER.rs
//    by Lut99
//
//  Created:
//    23 Oct 2024, 10:58:43
//  Last edited:
//    24 Oct 2024, 11:13:24
//  Auto updated?
//    Yes
//
//  Description:
//!   Provides resolvers for JWT keys.
//

// Modules
#[cfg(feature = "kid")]
pub mod kid;

// Imports
use std::error::Error;
use std::future::Future;

use jsonwebtoken::{DecodingKey, Header};
#[cfg(feature = "kid")]
pub use kid::KidResolver;


/***** LIBRARY *****/
/// The trait implemented by various backends.
///
/// Note that the KeyResolver is intended to be used in a distributed context. As such, any
/// reference to `self` is done immutably only.
pub trait KeyResolver {
    /// Client-side errors produced by the KeyResolver.
    type ClientError: Error;
    /// Server-side errors produced by the KeyResolver.
    type ServerError: Error;


    /// Provides the correct key to decode the JWT with based on its header.
    ///
    /// # Arguments
    /// - `header`: The JWT [`Header`] that tells us which key to find.
    ///
    /// # Returns
    /// A [`DecodingKey`] that can be used to verify the JWT.
    ///
    /// # Errors
    /// This function may error if we failed to obtain the key somehow.
    ///
    /// There are two levels at which it can do so:
    /// - The _outer_ [`Result`] is used to indicate _server_ errors (e.g., database
    ///   unreachable, etc); and
    /// - The _inner_ [`Result`] is used to indicate _user_ errors (e.g., no key, wrong key, etc).
    ///
    /// The first will always result in a (vague) 500 INTERNAL SERVER ERROR to the user, whereas
    /// the second may communicate custom status codes.
    fn resolve_key(&self, header: &Header) -> impl Send + Sync + Future<Output = Result<Result<DecodingKey, Self::ClientError>, Self::ServerError>>;
}
