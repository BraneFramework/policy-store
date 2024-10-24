//  SPEC.rs
//    by Lut99
//
//  Created:
//    24 Oct 2024, 12:05:52
//  Last edited:
//    24 Oct 2024, 12:54:37
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines the input/output structs for the server.
//

use serde::{Deserialize, Serialize};
use specifications::metadata::AttachedMetadata;


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
