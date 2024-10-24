//  AUTHRESOLVER.rs
//    by Lut99
//
//  Created:
//    23 Oct 2024, 10:31:06
//  Last edited:
//    24 Oct 2024, 12:00:58
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines the [`AuthResolver`] trait, which can take an HTTP request
//!   and use it to authorize it.
//

use std::error::Error;
use std::future::Future;

use http::{HeaderMap, StatusCode};


/***** AUXILLARY *****/
/// Extends an [`Error`] with the ability to associate status codes with it.
pub trait ClientError: Error {
    /// Returns the status code associated with this error.
    ///
    /// # Returns
    /// A [`StatusCode`].
    fn status_code(&self) -> StatusCode;
}





/***** LIBRARY *****/
/// A resolver that takes an HTTP request and (hopefully) authorizes it.
///
/// Note that the AuthResolver is intended to be used in a distributed context. As such, any
/// reference to `self` is done immutably only.
pub trait AuthResolver {
    /// Something produced by the resolver that can later be used to identify the user (e.g., some
    /// identifier).
    type Context;
    /// Client-side errors produced by the AuthResolver.
    type ClientError: ClientError;
    /// Server-side errors produced by the AuthResolver.
    type ServerError: Error;


    /// Resolves the given HTTP request to some authorization context.
    ///
    /// # Arguments
    /// - `headers`: The headers of the HTTP request to resolve.
    ///
    /// # Returns
    /// An [`AuthResolver::Context`] that can be used to identify the user later.
    ///
    /// # Errors
    /// This function can error when it fails to authorize the user. There are two levels at which
    /// it can do so:
    /// - The _outer_ [`Result`] is used to indicate _server_ errors (e.g., database
    ///   unreachable, etc); and
    /// - The _inner_ [`Result`] is used to indicate _user_ errors (e.g., no key, wrong key, etc).
    ///
    /// The first will always result in a (vague) 500 INTERNAL SERVER ERROR to the user, whereas
    /// the second may communicate custom status codes.
    fn authorize(&self, headers: &HeaderMap) -> impl Send + Future<Output = Result<Result<Self::Context, Self::ClientError>, Self::ServerError>>;
}
