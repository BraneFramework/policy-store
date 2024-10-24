//  SPEC.rs
//    by Lut99
//
//  Created:
//    24 Oct 2024, 12:05:52
//  Last edited:
//    24 Oct 2024, 16:41:13
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines the input/output structs for the server.
//

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use specifications::metadata::{AttachedMetadata, Metadata, User};


/***** LIBRARY *****/
/// What to send in the body of a request when [adding](crate::server::AxumServer::add_version())
/// a new version.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AddVersionRequest<C> {
    /// The metadata for this policy.
    pub metadata: AttachedMetadata,
    /// The contents of the policy itself.
    pub contents: C,
}

/// Replied when [adding](crate::server::AxumServer::add_version()) a new version.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AddVersionResponse {
    /// The newly assigned ID of the version.
    pub version: u64,
}



/// Replied when [listing](crate::server::AxumServer::get_versions()) all versions.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GetVersionsResponse {
    /// The versions in the reasoner.
    pub versions: HashMap<u64, Metadata>,
}



/// Replied when [retrieving the active policy](crate::server::AxumServer::get_active_version()).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GetActiveVersionResponse {
    /// The version of the active policy, if any.
    pub version: Option<u64>,
}



/// Replied when [retrieving the activator](crate::server::AxumServer::get_activator()).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GetActivatorResponse {
    /// The person who activated the active policy, if any.
    pub user: Option<User>,
}



/// Replied when [retrieving metadata](crate::server::AxumServer::get_version_metadata()).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GetVersionMetadataResponse {
    /// The metadata of the requested policy.
    pub metadata: Metadata,
}



/// Replied when [retrieving content](crate::server::AxumServer::get_version_content()).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GetVersionContentResponse<C> {
    /// The content of the requested policy.
    pub content: C,
}
