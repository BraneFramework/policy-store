//  LIB.rs
//    by Lut99
//
//  Created:
//    18 Oct 2024, 17:38:02
//  Last edited:
//    18 Oct 2024, 17:50:59
//  Auto updated?
//    Yes
//
//  Description:
//!   Provides public interfaces for things to be compatible with the
//!   policy store library.
//

// Declare modules
pub mod databaseconn;
pub mod metadata;

// Import some things into the main scope
pub use databaseconn::DatabaseConnector;
