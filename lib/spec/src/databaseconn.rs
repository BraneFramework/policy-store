//  DATABASECONN.rs
//    by Lut99
//
//  Created:
//    18 Oct 2024, 17:38:33
//  Last edited:
//    18 Oct 2024, 17:51:09
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines an interface to some backend database that stores policies.
//

use std::error::Error;
use std::future::Future;

use crate::metadata::Metadata;


/***** LIBRARY *****/
/// Defines how the policy store connects to the backend database that stores them.
pub trait DatabaseConnector {
    /// The content of the policy stored in the database.
    type Content;
    /// The type of errors returned by the connector.
    type Error: Error;


    // Version management
    /// Adds a new policy to the database.
    ///
    /// # Arguments
    /// - `metadata`: The [`Metadata`] that describes the context of the request.
    /// - `content`: The [`DatabaseConnector::Content`] that is the body of the policy to store.
    ///
    /// # Returns
    /// A version number that can be used to refer to this policy.
    ///
    /// # Errors
    /// This function may error if it failed to add the version to the backend database.
    fn add_version(metadata: Metadata, content: Self::Content) -> impl Future<Output = Result<u64, Self::Error>>;
}
