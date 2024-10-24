//  LIB.rs
//    by Lut99
//
//  Created:
//    23 Oct 2024, 10:25:43
//  Last edited:
//    24 Oct 2024, 12:07:02
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
pub mod spec;

// Use some of it
pub use server::*;
