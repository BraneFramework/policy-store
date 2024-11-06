//  BUILD.rs
//    by Lut99
//
//  Created:
//    05 Nov 2024, 11:58:38
//  Last edited:
//    06 Nov 2024, 13:58:09
//  Auto updated?
//    Yes
//
//  Description:
//!   Embeds the migration in Rust code such that it can be imported
//!   through the `migrations` module.
//

use std::borrow::Cow;
use std::ffi::OsString;
use std::fs::{DirEntry, File, ReadDir};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::{env, fs};


/***** CONSTANTS *****/
/// The name of the migrations directory.
pub const MIGRATIONS_DIR_NAME: &'static str = "migrations";

/// The name of the migrations source file.
pub const MIGRATIONS_SRC_FILE: &'static str = "migrations.rs";





/***** HELPER FUNCTIONS *****/
/// Foreaches into a directory with proper error handling.
fn for_each_entry(dir: &Path, mut handler: impl FnMut(DirEntry)) {
    // Write all the migrations nested in the dir
    let entries: ReadDir = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => panic!("Failed to read entries in directory '{}': {}", dir.display(), err),
    };
    for entry in entries {
        // Unwrap the entry
        let entry: DirEntry = match entry {
            Ok(entry) => entry,
            Err(err) => panic!("Failed to ready entry in directory '{}': {}", dir.display(), err),
        };

        // Call the handler
        handler(entry);
    }
}

/// Parses a migrations filename into a (date, time, name) tuple
fn parse_migration_name(dir: &str) -> Option<((u64, u64, u64), (u64, u64, u64), &str)> {
    // Parse the date first
    let mut year: u64 = 0;
    let mut month: u64 = 0;
    let mut day: u64 = 0;
    let mut hour: u64 = 0;
    let mut minute: u64 = 0;
    let mut second: u64 = 0;
    for (i, c) in dir.chars().enumerate() {
        println!("{i} => {c:?}");
        match i {
            // First, expect four characters as year
            0 | 1 | 2 | 3 => {
                if c >= '0' && c <= '9' {
                    year *= 10;
                    year += c as u64 - b'0' as u64;
                } else {
                    return None;
                }
            },
            // Then a dash
            4 => {
                if c != '-' {
                    return None;
                }
            },
            // Then two as month
            5 | 6 => {
                if c >= '0' && c <= '9' {
                    month *= 10;
                    month += c as u64 - b'0' as u64;
                } else {
                    return None;
                }
            },
            // Dash again
            7 => {
                if c != '-' {
                    return None;
                }
            },
            // The day
            8 | 9 => {
                if c >= '0' && c <= '9' {
                    day *= 10;
                    day += c as u64 - b'0' as u64;
                } else {
                    return None;
                }
            },
            // Final dash
            10 => {
                if c != '-' {
                    return None;
                }
            },
            // Then hours...
            11 | 12 => {
                if c >= '0' && c <= '9' {
                    hour *= 10;
                    hour += c as u64 - b'0' as u64;
                } else {
                    return None;
                }
            },
            // ...minutes...
            13 | 14 => {
                if c >= '0' && c <= '9' {
                    minute *= 10;
                    minute += c as u64 - b'0' as u64;
                } else {
                    return None;
                }
            },
            // ...and finally, seconds
            15 | 16 => {
                if c >= '0' && c <= '9' {
                    second *= 10;
                    second += c as u64 - b'0' as u64;
                } else {
                    return None;
                }
            },
            // Close off with an underscore...
            17 => {
                if c != '_' {
                    return None;
                }
            },
            // ...and the rest is the name!
            i => return Some(((year, month, day), (hour, minute, second), &dir[i..])),
        }
    }
    todo!()
}





/***** ENTRYPOINT *****/
fn main() {
    // Proclaim that we depend on the migrations-dir
    let crate_dir: PathBuf = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let migrations_dir: PathBuf = crate_dir.join(MIGRATIONS_DIR_NAME);
    println!("cargo:rerun-if-changed={}", migrations_dir.display());

    // Attempt to open the file
    let migrations_file: PathBuf = crate_dir.join("src").join(MIGRATIONS_SRC_FILE);
    let mut handle = match File::create(&migrations_file) {
        Ok(handle) => handle,
        Err(err) => panic!("Failed to create migrations file '{}': {}", migrations_file.display(), err),
    };

    // Write the file header
    if let Err(err) = handle.write_all(
        br#"// MIGRATIONS.rs
//   by Lut99
// 
// Created:
//    06 Nov 2024, 13:24:42
//  Last edited:
//    06 Nov 2024, 13:24:42
//  Auto updated?
//    Yes
//
//  Description:
//!   Embes [`diesel`] migrations in the binary.
//! 
//!   This file is generated by `build.rs`. Any problems, please fix it threre
//!   instead of here.
//

"#,
    ) {
        panic!("Failed to write to migations file '{}': {}", migrations_file.display(), err);
    }

    // Write all the migrations nested in the dir
    for_each_entry(&migrations_dir, |entry| {
        // Only do something if it's a dir
        let entry_path: PathBuf = entry.path();
        if entry_path.is_dir() {
            // Translate the entry name to something Rust-safe
            let entry_name: OsString = entry.file_name();
            let entry_name: Cow<str> = entry_name.to_string_lossy();
            let (date, time, name) = match parse_migration_name(&entry_name) {
                Some(res) => res,
                None => return,
            };

            // Write the module header
            if let Err(err) =
                handle.write_all(format!("pub mod {}_{}_{}_{}_{}{}{} {{\n", name, date.0, date.1, date.2, time.0, time.1, time.2).as_bytes())
            {
                panic!("Failed to write to migations file '{}': {}", migrations_file.display(), err);
            }

            // Traverse into that folder
            for_each_entry(&entry_path, |entry| {
                // Only do something if it's an SQL file
                let entry_path: PathBuf = entry.path();
                if entry_path.is_file() && entry_path.file_name().map(|name| name.to_string_lossy().ends_with(".sql")).unwrap_or(false) {
                    // Translate the entry name to something Rust-safe
                    let entry_name: String = entry
                        .file_name()
                        .to_string_lossy()
                        .chars()
                        .map(|c| {
                            if c >= 'a' && c <= 'z' {
                                c.to_ascii_uppercase()
                            } else if (c >= '0' && c <= '9') || (c >= 'A' && c <= 'Z') {
                                c
                            } else {
                                '_'
                            }
                        })
                        .collect();

                    // Write it
                    if let Err(err) = handle.write_all(
                        format!("    pub const {}: &'static str =\n        include_str!(\"{}\");\n", entry_name, entry_path.display()).as_bytes(),
                    ) {
                        panic!("Failed to write to migations file '{}': {}", migrations_file.display(), err);
                    }
                }
            });

            // Then close the header
            if let Err(err) = handle.write_all(b"}\n") {
                panic!("Failed to write to migations file '{}': {}", migrations_file.display(), err);
            }
        }
    });
}
