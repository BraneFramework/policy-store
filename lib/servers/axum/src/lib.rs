//  LIB.rs
//    by Lut99
//
//  Created:
//    23 Oct 2024, 10:25:43
//  Last edited:
//    23 Oct 2024, 13:59:19
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

// Use some of it
pub use server::*;
