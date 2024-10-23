//  LIB.rs
//    by Lut99
//
//  Created:
//    23 Oct 2024, 10:37:34
//  Last edited:
//    23 Oct 2024, 10:59:20
//  Auto updated?
//    Yes
//
//  Description:
//!   Implements a JSON Web Token (JWT) / JSON Web Key (JWK)-based scheme
//!   for the `AuthResolver`.
//

// Modules
mod authresolver;
pub mod keyresolver;

// Use some of it into the main namespace
pub use authresolver::*;
