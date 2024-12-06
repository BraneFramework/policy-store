//  LIB.rs
//    by Lut99
//
//  Created:
//    18 Oct 2024, 17:31:50
//  Last edited:
//    06 Dec 2024, 18:01:14
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
    #[cfg(all(not(feature = "axum-server"), feature = "axum-server-spec"))]
    pub mod axum {
        pub use axum_server_spec as spec;
    }
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
