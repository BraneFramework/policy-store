//  BUILD.rs
//    by Lut99
//
//  Created:
//    05 Nov 2024, 11:58:38
//  Last edited:
//    06 Nov 2024, 14:05:10
//  Auto updated?
//    Yes
//
//  Description:
//!   Embeds the migration in Rust code such that it can be imported
//!   through the `migrations` module.
//


/***** ENTRYPOINT *****/
fn main() {
    // Emit that rebuilding on migrations is necessary
    // See https://docs.rs/migrations_macros/2.2.0/migrations_macros/macro.embed_migrations.html#automatic-rebuilds
    println!("cargo:rerun-if-changed=./migrations");
}
