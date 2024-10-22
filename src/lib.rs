//  LIB.rs
//    by Lut99
//
//  Created:
//    18 Oct 2024, 17:31:50
//  Last edited:
//    22 Oct 2024, 14:35:53
//  Auto updated?
//    Yes
//
//  Description:
//!   Stores policy for use with a
//!   [policy reasoner](https://github.com/epi-project/policy-reasoner).
//

// Import the libraries
pub mod databases {
    #[cfg(feature = "sqlite-database")]
    pub use sqlite_database as sqlite;
}

pub use specifications as spec;
