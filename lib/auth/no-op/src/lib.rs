//  LIB.rs
//    by Lut99
//
//  Created:
//    24 Oct 2024, 13:50:43
//  Last edited:
//    24 Oct 2024, 14:01:06
//  Auto updated?
//    Yes
//
//  Description:
//!   Implements an [`AuthResolver`] that doesn't actually resolve anything.
//

use std::convert::Infallible;

use http::HeaderMap;
use specifications::authresolver::AuthResolver;
use specifications::metadata::User;


/***** LIBRARY *****/
/// Defines an [`AuthResolver`] that doesn't authorize people whatsoever.
#[derive(Clone, Copy, Debug)]
pub struct NoOpResolver;
impl Default for NoOpResolver {
    #[inline]
    fn default() -> Self { Self::new() }
}
impl NoOpResolver {
    /// Constructor for the NoOpResolver.
    ///
    /// # Returns
    /// A new NoOpResolver ready to do absolutely nothing.
    #[inline]
    pub const fn new() -> Self { Self }
}
impl AuthResolver for NoOpResolver {
    type Context = User;
    type ClientError = Infallible;
    type ServerError = Infallible;

    #[inline]
    async fn authorize(&self, _headers: &HeaderMap) -> Result<Result<Self::Context, Self::ClientError>, Self::ServerError> {
        Ok(Ok(User { id: "johnsmith".into(), name: "John Smith".into() }))
    }
}
