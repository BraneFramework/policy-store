//  DATABASECONN.rs
//    by Lut99
//
//  Created:
//    22 Oct 2024, 14:37:56
//  Last edited:
//    07 Feb 2025, 16:53:45
//  Auto updated?
//    Yes
//
//  Description:
//!   Implements the actual [`DatabaseConnector`].
//

use std::collections::HashMap;
use std::future::Future;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use chrono::{NaiveDateTime, Utc};
use diesel::connection::LoadConnection;
use diesel::migration::MigrationSource;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::sqlite::Sqlite;
use diesel::{Connection as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, SelectableHelper as _, SqliteConnection};
use diesel_migrations::{FileBasedMigrations, MigrationHarness as _};
use serde::Serialize;
use serde::de::DeserializeOwned;
use specifications::DatabaseConnector;
use specifications::databaseconn::DatabaseConnection;
use specifications::metadata::{AttachedMetadata, Metadata, User};
use thiserror::Error;
use tokio::fs;
use tracing::{Level, debug, info, span};

use crate::models::{SqliteActiveVersion, SqlitePolicy};


/***** ERRORS *****/
/// Defines errors originating from the [`SQLiteDatabase`].
#[derive(Debug, Error)]
pub enum DatabaseError {
    /// Failed to create a new connection to the backend database.
    #[error("Failed to connect to backend database {:?}", path.display())]
    Connect {
        path: PathBuf,
        #[source]
        err:  diesel::r2d2::PoolError,
    },
    /// Failed to connect to the database when creating it.
    #[error("Failed to first-time connect to backend database {:?}", path.display())]
    ConnectDatabase {
        path: PathBuf,
        #[source]
        err:  diesel::ConnectionError,
    },
    /// Failed to create the database.
    #[error("Failed to create database file {:?}", path.display())]
    DatabaseCreate {
        path: PathBuf,
        #[source]
        err:  std::io::Error,
    },
    /// Failed to create the parent directory of the database.
    #[error("Failed to create database parent directory {:?}", path.display())]
    DatabaseDirCreate {
        path: PathBuf,
        #[source]
        err:  std::io::Error,
    },
    /// Failed to apply the migrations in a particular folder to a particular database.
    #[error("Failed to apply migrations to new database {:?}", path.display())]
    MigrationsApply {
        path: PathBuf,
        #[source]
        err:  Box<dyn 'static + std::error::Error>,
    },
    /// Failed to find the migrations for a database in the given folder.
    #[error("Failed to find migrations in migrations folder {:?}", migrations_dir.display())]
    MigrationsFind {
        migrations_dir: PathBuf,
        #[source]
        err: diesel_migrations::MigrationError,
    },
    /// Failed to create a new connection pool.
    #[error("Failed to create a connection pool to backend database {:?}", path.display())]
    PoolCreate {
        path: PathBuf,
        #[source]
        err:  diesel::r2d2::PoolError,
    },
}

/// Defines errors originating from the [`SQLiteConnection`].
#[derive(Debug, Error)]
pub enum ConnectionError {
    /// Failed to add a new version to the backend database.
    #[error("Failed to add a new version to backend database {:?}", path.display())]
    AddVersion {
        path: PathBuf,
        #[source]
        err:  diesel::result::Error,
    },
    /// Failed to deserialize the given content from JSON.
    #[error("Failed to deserialize the given content of policy {name:?} ({version}) from JSON")]
    ContentDeserialize {
        name:    String,
        version: u64,
        #[source]
        err:     serde_json::Error,
    },
    /// Failed to serialize the given content as JSON.
    #[error("Failed to serialize the content of policy {name:?} as JSON")]
    ContentSerialize {
        name: String,
        #[source]
        err:  serde_json::Error,
    },
    /// Failed to deactivate an active version.
    #[error("Failed to deactivate active policy version {version} in backend database {:?}", path.display())]
    DeactivateVersion {
        path:    PathBuf,
        version: u64,
        #[source]
        err:     diesel::result::Error,
    },
    /// Failed to fetch the active version.
    #[error("Failed to get active version from backend database {:?}", path.display())]
    GetActiveVersion {
        path: PathBuf,
        #[source]
        err:  diesel::result::Error,
    },
    /// Failed to fetch the latest version.
    #[error("Failed to get latest version from backend database {:?}", path.display())]
    GetLatestVersion {
        path: PathBuf,
        #[source]
        err:  diesel::result::Error,
    },
    /// Failed to get a specific version.
    #[error("Failed to get version {version} from backend database {:?}", path.display())]
    GetVersion {
        path:    PathBuf,
        version: u64,
        #[source]
        err:     diesel::result::Error,
    },
    /// Failed to get the list of versions.
    #[error("Failed to get the list of versions from backend database {:?}", path.display())]
    GetVersions {
        path: PathBuf,
        #[source]
        err:  diesel::result::Error,
    },
    /// Failed to set the currently active policy.
    #[error("Failed to set version {version} as the active policy in backend database {:?}", path.display())]
    SetActive {
        path:    PathBuf,
        version: u64,
        #[source]
        err:     diesel::result::Error,
    },
    /// Failed to spawn a background blocking task.
    #[error("Failed to spawn a blocking task")]
    SpawnBlocking {
        #[source]
        err: tokio::task::JoinError,
    },
    /// Failed to start a transaction with the database.
    #[error("Failed to start a transaction with the backend database")]
    Transaction {
        #[source]
        err: diesel::result::Error,
    },
}
// Note: implemented to always error for transaction
impl From<diesel::result::Error> for ConnectionError {
    #[inline]
    fn from(value: diesel::result::Error) -> Self { Self::Transaction { err: value } }
}





/***** LIBRARY *****/
/// A [`DatabaseConnector`] that can interface with SQLite databases.
#[derive(Clone)]
pub struct SQLiteDatabase<C> {
    /// The path to the file that we represent. Only retained during runtime for debugging.
    path:     PathBuf,
    /// The pool of connections.
    pool:     Pool<ConnectionManager<SqliteConnection>>,
    /// Remembers the type of content used.
    _content: PhantomData<C>,
}
impl<C> SQLiteDatabase<C> {
    /// Constructor for the SQLiteDatabase.
    ///
    /// # Arguments
    /// - `path`: The path of the database to connect to.
    /// - `migrations`: A [`MigrationSource`] with migrations to apply when creating a new database.
    ///
    /// # Returns
    /// A new SQLiteDatabase struct that can be used to connect to the backend file.
    ///
    /// # Errors
    /// This function may fail if we failed to setup a connection pool to the given path, or if we
    /// failed to apply the migrations in case it's a new file.
    pub async fn new_async(path: impl Into<PathBuf>, migrations: impl MigrationSource<Sqlite>) -> Result<Self, DatabaseError> {
        let path: PathBuf = path.into();
        debug!("Creating new SQLite connector to {:?}...", path.display());

        // Check if we need to create it first
        if !path.exists() {
            info!("Database {:?} doesn't exist; creating...", path.display());

            // Touch the database file
            if let Some(dir) = path.parent() {
                if !dir.exists() {
                    if let Err(err) = fs::create_dir(&dir).await {
                        return Err(DatabaseError::DatabaseDirCreate { path: dir.into(), err });
                    }
                }
            }
            if let Err(err) = fs::File::create(&path).await {
                return Err(DatabaseError::DatabaseCreate { path, err });
            }

            // Apply them by connecting to the database
            let mut conn: SqliteConnection = match SqliteConnection::establish(&path.display().to_string()) {
                Ok(conn) => conn,
                Err(err) => return Err(DatabaseError::ConnectDatabase { path, err }),
            };
            if let Err(err) = conn.run_pending_migrations(migrations) {
                return Err(DatabaseError::MigrationsApply { path, err });
            }
        } else {
            debug!("Database {:?} already exists", path.display());
        }

        // Create the pool
        debug!("Connecting to database {:?}...", path.display());
        let pool: Pool<_> = match Pool::new(ConnectionManager::new(path.display().to_string())) {
            Ok(pool) => pool,
            Err(err) => return Err(DatabaseError::PoolCreate { path, err }),
        };

        // OK, now create self
        Ok(Self { path, pool, _content: PhantomData })
    }

    /// Constructor for the SQLiteDatabase that reads migrations from the given file.
    ///
    /// # Arguments
    /// - `path`: The path of the database to connect to.
    /// - `migrations_dir`: A directory with migrations to apply when creating a new database.
    ///
    /// # Returns
    /// A new SQLiteDatabase struct that can be used to connect to the backend file.
    ///
    /// # Errors
    /// This function may fail if we failed to setup a connection pool to the given path, or if we
    /// failed to apply the migrations in case it's a new file.
    pub async fn with_migrations_from_dir_async(path: impl Into<PathBuf>, migrations_dir: impl AsRef<Path>) -> Result<Self, DatabaseError> {
        let migrations_dir: &Path = migrations_dir.as_ref();
        debug!("Reading migrations from {:?}...", migrations_dir.display());
        let migrations: FileBasedMigrations = match FileBasedMigrations::find_migrations_directory_in_path(migrations_dir) {
            Ok(migrations) => migrations,
            Err(err) => return Err(DatabaseError::MigrationsFind { migrations_dir: migrations_dir.into(), err }),
        };

        // Delegate to the normal one
        Self::new_async(path, migrations).await
    }
}
impl<C: Send + Sync + DeserializeOwned + Serialize> DatabaseConnector for SQLiteDatabase<C> {
    type Connection<'s>
        = SQLiteConnection<'s, C>
    where
        Self: 's;
    type Content = C;
    type Error = DatabaseError;

    #[inline]
    fn connect<'s>(&'s self, user: &'s specifications::metadata::User) -> impl Send + Future<Output = Result<Self::Connection<'s>, Self::Error>> {
        async move {
            // Attempt to get a connection from the pool
            debug!("Creating new connection to SQLite database {:?}...", self.path.display());
            match self.pool.get() {
                Ok(conn) => Ok(SQLiteConnection { path: &self.path, conn, user, _content: PhantomData }),
                Err(err) => Err(DatabaseError::Connect { path: self.path.clone(), err }),
            }
        }
    }
}



/// Represents the connection created by [`SQLiteDatabase::connect()`].
pub struct SQLiteConnection<'a, C> {
    /// The path to the file that we represent. Only retained during runtime for debugging.
    path:     &'a Path,
    /// The connection we wrap.
    conn:     PooledConnection<ConnectionManager<SqliteConnection>>,
    /// The user that is doing everything in this connection.
    user:     &'a User,
    /// Remembers the type of content chosen for this connection.
    _content: PhantomData<C>,
}
impl<'a, C> SQLiteConnection<'a, C> {
    /// Helper function for doing the non-async active version retrieval.
    ///
    /// # Arguments
    /// - `path`: The path where the backend SQLite database lives. Only given for debugging purposes.
    /// - `conn`: Some [`LoadConnection`] that we use to talk to the file.
    ///
    /// # Returns
    /// An activate version if there was one (else, [`None`]).
    ///
    /// # Errors
    /// This function errors if we failed to get the active version.
    fn _get_active_version<C2>(path: &Path, conn: &mut C2) -> Result<Option<u64>, ConnectionError>
    where
        C2: LoadConnection<Backend = Sqlite>,
    {
        use crate::schema::active_version::dsl::active_version;

        debug!("Fetching active version...");
        match active_version
            .limit(1)
            .order_by(crate::schema::active_version::dsl::activated_on.desc())
            .select(SqliteActiveVersion::as_select())
            .load(conn)
        {
            Ok(mut r) => match r.pop() {
                Some(av) => {
                    if av.deactivated_on.is_some() {
                        Ok(None)
                    } else {
                        Ok(Some(av.version as u64))
                    }
                },
                None => Ok(None),
            },
            Err(err) => return Err(ConnectionError::GetActiveVersion { path: path.into(), err }),
        }
    }
}
impl<'a, C: Send + DeserializeOwned + Serialize> DatabaseConnection for SQLiteConnection<'a, C> {
    type Content = C;
    type Error = ConnectionError;


    // Mutable
    fn add_version(&mut self, metadata: AttachedMetadata, content: Self::Content) -> impl Send + Future<Output = Result<u64, Self::Error>> {
        use crate::schema::policies::dsl::policies;

        async move {
            let span = span!(Level::INFO, "SQLiteConnection::add_version", policy = metadata.name,);

            debug!("Starting transaction...");
            tokio::task::block_in_place(move || {
                self.conn.exclusive_transaction(|conn| -> Result<u64, Self::Error> {
                    // Trick the compiler into moving the span too
                    let _span = span;

                    debug!("Retrieving latest policy version...");
                    let latest: i64 = policies::select(policies, crate::schema::policies::dsl::version)
                        .order_by(crate::schema::policies::dsl::created_at.desc())
                        .limit(1)
                        .load(conn)
                        .map_err(|err| ConnectionError::GetLatestVersion { path: self.path.into(), err })?
                        .pop()
                        .unwrap_or(0);

                    // up to next version
                    let next_version: i64 = latest + 1;

                    // Construct the policy itself
                    debug!("Adding new policy {next_version}...");
                    let content = match serde_json::to_string(&content) {
                        Ok(content) => content,
                        Err(err) => return Err(ConnectionError::ContentSerialize { name: metadata.name, err }),
                    };
                    let model = SqlitePolicy {
                        name: metadata.name,
                        description: metadata.description,
                        language: metadata.language,
                        version: next_version,
                        creator: self.user.id.clone(),
                        created_at: Utc::now().naive_utc(),
                        content,
                    };

                    // Submit it
                    match diesel::insert_into(policies).values(&model).execute(conn) {
                        Ok(_) => Ok(next_version as u64),
                        Err(err) => Err(ConnectionError::AddVersion { path: self.path.into(), err }),
                    }
                })
            })
        }
    }

    fn activate(&mut self, version: u64) -> impl Send + Future<Output = Result<(), Self::Error>> {
        use crate::schema::active_version::dsl::active_version;

        async move {
            let span = span!(Level::INFO, "SQLiteConnection::activate", version = version);

            debug!("Starting transaction...");
            tokio::task::block_in_place(move || {
                self.conn.exclusive_transaction(|conn| -> Result<(), Self::Error> {
                    // Trick the compiler into moving the span too
                    let _span = span;

                    // Get the information about what to activate
                    let av = Self::_get_active_version(&self.path, conn)?;

                    // They may already be the same, ez
                    if av.is_some_and(|v| v == version) {
                        info!("Activated already-active version {version}");
                        return Ok(());
                    }

                    // Otherwise, build the model and submit it
                    debug!("Activating policy {version}...");
                    let model = SqliteActiveVersion::new(version as i64, self.user.id.clone());
                    if let Err(err) = diesel::insert_into(active_version).values(&model).execute(conn) {
                        return Err(ConnectionError::SetActive { path: self.path.into(), version, err });
                    }
                    Ok(())
                })
            })
        }
    }

    fn deactivate(&mut self) -> impl Send + Future<Output = Result<(), Self::Error>> {
        use crate::schema::active_version::dsl::{active_version, deactivated_by, deactivated_on, version};

        async move {
            let _span = span!(Level::INFO, "SQLiteConnection::deactivate");

            debug!("Starting transaction...");
            tokio::task::block_in_place(move || {
                self.conn.exclusive_transaction(|conn| -> Result<(), Self::Error> {
                    // Get the current active version, if any
                    let av = match Self::_get_active_version(&self.path, conn)? {
                        Some(av) => av,
                        None => {
                            info!("Deactivated a policy whilst none were active");
                            return Ok(());
                        },
                    };

                    // If we found one, then update it
                    debug!("Deactivating active policy {av}...");
                    if let Err(err) = diesel::update(active_version)
                        .filter(version.eq(av as i64))
                        .set((deactivated_on.eq(Utc::now().naive_local()), deactivated_by.eq(&self.user.id)))
                        .execute(conn)
                    {
                        return Err(ConnectionError::DeactivateVersion { path: self.path.into(), version: av, err });
                    }
                    Ok(())
                })
            })
        }
    }


    // Immutable
    fn get_versions(&mut self) -> impl Send + Future<Output = Result<HashMap<u64, Metadata>, Self::Error>> {
        use crate::schema::policies::dsl as policy;

        async move {
            let _span = span!(Level::INFO, "SQLiteConnection::get_versions");

            debug!("Retrieving all policy versions...");
            match policy::policies
                .order_by(crate::schema::policies::dsl::created_at.desc())
                .select((policy::description, policy::name, policy::language, policy::version, policy::creator, policy::created_at))
                .load::<(String, String, String, i64, String, NaiveDateTime)>(&mut self.conn)
            {
                Ok(r) => Ok(r
                    .into_iter()
                    .map(|(description, name, language, version, creator, created_at)| {
                        (version as u64, Metadata {
                            attached: AttachedMetadata { name, description, language },
                            version:  version as u64,
                            creator:  User { id: creator, name: "John Smith".into() },
                            created:  created_at.and_utc(),
                        })
                    })
                    .collect()),
                Err(err) => Err(ConnectionError::GetVersions { path: self.path.into(), err }),
            }
        }
    }

    fn get_active_version(&mut self) -> impl Send + Future<Output = Result<Option<u64>, Self::Error>> {
        async move {
            let _span = span!(Level::INFO, "SQLiteConnection::get_active");

            // Do a call to get the active, if any
            tokio::task::block_in_place(move || Self::_get_active_version(&self.path, &mut self.conn))
        }
    }

    fn get_activator(&mut self) -> impl Send + Future<Output = Result<Option<User>, Self::Error>> {
        use crate::schema::active_version::dsl::active_version;

        async move {
            let _span = span!(Level::INFO, "SQLiteConnection::get_active");

            // Do a call to get the active, if any
            debug!("Fetching active version...");
            match active_version
                .limit(1)
                .order_by(crate::schema::active_version::dsl::activated_on.desc())
                .select(SqliteActiveVersion::as_select())
                .load(&mut self.conn)
            {
                Ok(mut r) => match r.pop() {
                    Some(av) => {
                        if av.deactivated_on.is_some() {
                            Ok(None)
                        } else {
                            Ok(Some(User { id: av.activated_by, name: "John Smith".into() }))
                        }
                    },
                    None => Ok(None),
                },
                Err(err) => return Err(ConnectionError::GetActiveVersion { path: self.path.into(), err }),
            }
        }
    }

    fn get_version_metadata(&mut self, version: u64) -> impl Send + Future<Output = Result<Option<Metadata>, Self::Error>> {
        use crate::schema::policies::dsl as policy;

        async move {
            let _span = span!(Level::INFO, "SQLiteConnection::get_version_metadata", version = version);

            debug!("Retrieving metadata for version {version}...");
            match policy::policies
                .limit(1)
                .filter(crate::schema::policies::dsl::version.eq(version as i64))
                .order_by(crate::schema::policies::dsl::created_at.desc())
                .select((policy::description, policy::name, policy::language, policy::version, policy::creator, policy::created_at))
                .load::<(String, String, String, i64, String, NaiveDateTime)>(&mut self.conn)
            {
                Ok(mut r) => {
                    // Extract the version itself
                    if r.len() < 1 {
                        return Ok(None);
                    }
                    let (description, name, language, version, creator, created_at) = r.remove(0);

                    // Done, return the thing
                    Ok(Some(Metadata {
                        attached: AttachedMetadata { name, description, language },
                        created:  created_at.and_utc(),
                        creator:  User { id: creator, name: "John Smith".into() },
                        version:  version as u64,
                    }))
                },
                Err(err) => match err {
                    diesel::result::Error::NotFound => Ok(None),
                    err => Err(ConnectionError::GetVersion { path: self.path.into(), version, err }),
                },
            }
        }
    }

    fn get_version_content(&mut self, version: u64) -> impl Send + Future<Output = Result<Option<Self::Content>, Self::Error>> {
        use crate::schema::policies::dsl as policy;

        async move {
            let _span = span!(Level::INFO, "SQLiteConnection::get_version_content", version = version);

            tokio::task::block_in_place(move || {
                debug!("Retrieving content for version {version}...");
                match policy::policies
                    .limit(1)
                    .filter(crate::schema::policies::dsl::version.eq(version as i64))
                    .order_by(crate::schema::policies::dsl::created_at.desc())
                    .select((policy::name, policy::content))
                    .load::<(String, String)>(&mut self.conn)
                {
                    Ok(mut r) => {
                        // Extract the version itself
                        if r.len() < 1 {
                            return Ok(None);
                        }
                        let (name, content) = r.remove(0);

                        // Deserialize the content
                        match serde_json::from_str(&content) {
                            Ok(content) => Ok(Some(content)),
                            Err(err) => Err(ConnectionError::ContentDeserialize { name, version, err }),
                        }
                    },
                    Err(err) => match err {
                        diesel::result::Error::NotFound => Ok(None),
                        err => Err(ConnectionError::GetVersion { path: self.path.into(), version, err }),
                    },
                }
            })
        }
    }
}
