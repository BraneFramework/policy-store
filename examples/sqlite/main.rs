//  SQLITE.rs
//    by Lut99
//
//  Created:
//    24 Oct 2024, 13:55:22
//  Last edited:
//    06 Nov 2024, 14:10:15
//  Auto updated?
//    Yes
//
//  Description:
//!   Shows an example reasoner based on the SQLite database backend.
//

use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Parser;
use error_trace::trace;
use policy_store::auth::no_op::NoOpResolver;
use policy_store::databases::sqlite::SQLiteDatabase;
use policy_store::servers::axum::AxumServer;
use policy_store::spec::Server as _;
use tokio::signal::unix::{signal, SignalKind};
use tracing::{debug, error, info, warn, Level};


/***** ARGUMENTS *****/
/// Defines the arguments for this binary.
#[derive(Debug, Parser)]
struct Arguments {
    /// Whether to enable INFO- and DEBUG-level logging.
    #[clap(long)]
    debug: bool,
    /// Whether to enable TRACE-level logging. Implies '--debug'.
    #[clap(long)]
    trace: bool,

    /// The address/port on which to bind the server.
    #[clap(short, long, default_value = "127.0.0.1:8080")]
    address:  SocketAddr,
    /// The path to the database file to create/use.
    #[clap(short, long, default_value = "./policies.db")]
    database: PathBuf,
}





/***** ENTRYPOINT *****/
#[tokio::main]
async fn main() {
    // Parse the arguments
    let args = Arguments::parse();

    // Setup the logger
    tracing_subscriber::fmt()
        .with_max_level(if args.trace {
            Level::TRACE
        } else if args.debug {
            Level::DEBUG
        } else {
            Level::WARN
        })
        .init();
    info!("{} - v{}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));

    // Setup the auth
    let auth = NoOpResolver::new();

    // Setup the database
    let db: SQLiteDatabase<bool> = match SQLiteDatabase::with_migrations_from_dir_async(
        &args.database,
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib").join("databases").join("sqlite").join("migrations"),
    )
    .await
    {
        Ok(db) => db,
        Err(err) => {
            error!("{}", trace!(("Failed to create database connector"), err));
            std::process::exit(1);
        },
    };

    // OK, setup the server
    let server = AxumServer::new(args.address, auth, db);
    tokio::select! {
        res = server.serve() => match res {
            Ok(_) => info!("Done"),
            Err(err) => {
                error!("{}", trace!(("Failed to serve the server"), err));
                std::process::exit(1);
            },
        },

        _ = async move {
            match signal(SignalKind::interrupt()) {
                Ok(mut sign) => sign.recv().await,
                Err(err) => {
                    warn!("{}", trace!(("Failed to register SIGINT signal handler"), err));
                    warn!("Graceful shutdown by Ctrl+C disabled");
                    None
                },
            }
        } => {
            debug!("Received SIGINT");
        },
        _ = async move {
            match signal(SignalKind::terminate()) {
                Ok(mut sign) => sign.recv().await,
                Err(err) => {
                    warn!("{}", trace!(("Failed to register SIGTERM signal handler"), err));
                    warn!("Graceful shutdown by Docker disabled");
                    None
                },
            }
        } => {
            debug!("Received SIGTERM");
        },
    }
}
