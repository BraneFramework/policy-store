//  METADATA.rs
//    by Lut99
//
//  Created:
//    18 Oct 2024, 17:50:16
//  Last edited:
//    23 Oct 2024, 14:57:25
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines metadata that is associated with every policy.
//

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};


/***** LIBRARY *****/
/// Represents the relevant information about a creator/editor/w/e.
///
/// Note that it can be generally assumed that other parts of the reasoner fuss about how to
/// make sure this represents an actual, authenticated user.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct User {
    /// Some machine-relevant identifier of the creator.
    pub id:   String,
    /// Some human-relevant identifier of the creator.
    pub name: String,
}

/// Metadata that is given by the user as an attachment to a policy.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AttachedMetadata {
    /// Some name for the policy to recognise it later. Doesn't have to be unique.
    pub name: String,
    /// Some description of the policy for recognition.
    pub description: String,
}

/// Metadata associated with a policy snippet.
///
/// Includes whatever is [attached](AttachedMetadata), but also things inferred when pushing
/// versions (e.g., created time).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Metadata {
    /// Whatever the user attached at runtime.
    pub attached: AttachedMetadata,

    /// The time the policy was created.
    pub created: DateTime<Utc>,
    /// Defines who has written a policy.
    pub creator: User,
    /// The version number of this snippet.
    pub version: u64,
}
