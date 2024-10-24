//  LIB.rs
//    by Lut99
//
//  Created:
//    18 Oct 2024, 17:31:50
//  Last edited:
//    24 Oct 2024, 14:05:57
//  Auto updated?
//    Yes
//
//  Description:
//!   Stores policy for use with a
//!   [policy reasoner](https://github.com/epi-project/policy-reasoner).
//

// Import the libraries
pub mod servers {
    #[cfg(feature = "axum-server")]
    pub use axum_server as axum;
}

pub mod auth {
    #[cfg(feature = "jwk-auth")]
    pub use jwk_auth as jwk;
    #[cfg(feature = "no-op-auth")]
    pub use no_op_auth as no_op;
}

pub mod databases {
    #[cfg(feature = "sqlite-database")]
    pub use sqlite_database as sqlite;
}

pub use specifications as spec;
