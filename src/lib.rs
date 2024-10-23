//  LIB.rs
//    by Lut99
//
//  Created:
//    18 Oct 2024, 17:31:50
//  Last edited:
//    23 Oct 2024, 11:41:32
//  Auto updated?
//    Yes
//
//  Description:
//!   Stores policy for use with a
//!   [policy reasoner](https://github.com/epi-project/policy-reasoner).
//

// Import the libraries
pub mod servers {
    #[cfg(feature = "axum-api")]
    pub use axum_api as axum;
}

pub mod auth {
    #[cfg(feature = "jwk-auth")]
    pub use jwk_auth as jwk;
}

pub mod databases {
    #[cfg(feature = "sqlite-database")]
    pub use sqlite_database as sqlite;
}

pub use specifications as spec;
