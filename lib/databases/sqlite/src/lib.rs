//  LIB.rs
//    by Lut99
//
//  Created:
//    22 Oct 2024, 14:37:34
//  Last edited:
//    06 Nov 2024, 14:05:26
//  Auto updated?
//    Yes
//
//  Description:
//!   Implements the `DatabaseConnector` for an SQLite backend.
//

// Declare modules
mod databaseconn;
// #[cfg(feature = "embedded-migrations")]
// pub mod migrations;
mod models;
mod schema;

// Import some of it
pub use databaseconn::*;


// Optionally import the migrations
#[cfg(feature = "embedded-migrations")]
pub const MIGRATIONS: diesel_migrations::EmbeddedMigrations = diesel_migrations::embed_migrations!("./migrations");
