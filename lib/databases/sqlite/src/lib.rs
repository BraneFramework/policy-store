//  LIB.rs
//    by Lut99
//
//  Created:
//    22 Oct 2024, 14:37:34
//  Last edited:
//    06 Nov 2024, 13:58:21
//  Auto updated?
//    Yes
//
//  Description:
//!   Implements the `DatabaseConnector` for an SQLite backend.
//

// Declare modules
mod databaseconn;
#[cfg(feature = "embedded-migrations")]
pub mod migrations;
mod models;
mod schema;

// Import some of it
pub use databaseconn::*;
