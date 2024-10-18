//  METADATA.rs
//    by Lut99
//
//  Created:
//    18 Oct 2024, 17:50:16
//  Last edited:
//    18 Oct 2024, 17:53:05
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines metadata that is associated with every policy.
//


/***** LIBRARY *****/
/// Defines what is known across all possible policy to store in the store.
#[derive(Clone, Debug)]
pub struct Metadata {
    /// Defines who has written a policy.
    pub creator: String,
    /// Some name for the policy to recognise it later. Doesn't have to be unique.
    pub name: String,
    /// Some description of the policy for recognition.
    pub description: String,
}
