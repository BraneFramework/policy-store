//  LIB.rs
//    by Lut99
//
//  Created:
//    23 Oct 2024, 10:25:43
//  Last edited:
//    06 Dec 2024, 18:02:38
//  Auto updated?
//    Yes
//
//  Description:
//!   Implements an out-of-the-box, standardized HTTP API for the policy
//!   store using `axum`.
//

// Modules
mod auth;
mod paths;
mod server;

// Re-exports
pub use axum_server_spec as spec;
// Use local parts
pub use server::*;
