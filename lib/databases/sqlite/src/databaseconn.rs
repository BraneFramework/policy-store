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
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use chrono::{NaiveDateTime, Utc};
use deadpool::managed::Object;
use deadpool_diesel::{Manager, Pool, PoolError};
use diesel::connection::LoadConnection;
use diesel::migration::MigrationSource;
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
use tracing::{debug, info, instrument};

use crate::models::{SqliteActiveVersion, SqlitePolicy};


/***** ERRORS *****/
/// Defines errors originating from the [`SQLiteDatabase`].
#[derive(Debug, Error)]
pub enum DatabaseError {
    /// Failed to create a new connection to the backend database.
    #[error("Failed to connect to backend database {:?}", path.display())]
    Connect { path: PathBuf, source: PoolError },
    /// Failed to connect to the database when creating it.
    #[error("Failed to first-time connect to backend database {:?}", path.display())]
    ConnectDatabase { path: PathBuf, source: diesel::ConnectionError },
    /// Failed to create the database.
    #[error("Failed to create database file {:?}", path.display())]
    DatabaseCreate { path: PathBuf, source: std::io::Error },
    /// Failed to create the parent directory of the database.
    #[error("Failed to create database parent directory {:?}", path.display())]
    DatabaseDirCreate { path: PathBuf, source: std::io::Error },
    /// Failed to apply the migrations in a particular folder to a particular database.
    #[error("Failed to apply migrations to new database {:?}", path.display())]
    MigrationsApply { path: PathBuf, source: Box<dyn 'static + std::error::Error> },
    /// Failed to find the migrations for a database in the given folder.
    #[error("Failed to find migrations in migrations folder {:?}", migrations_dir.display())]
    MigrationsFind { migrations_dir: PathBuf, source: diesel_migrations::MigrationError },
    /// Failed to create a new connection pool.
    #[error("Failed to create a connection pool to backend database {:?}", path.display())]
    PoolCreate { path: PathBuf, source: deadpool::managed::BuildError },
}

/// Defines errors originating from the [`SQLiteConnection`].
#[derive(Debug, Error)]
pub enum ConnectionError {
    /// Failed to add a new version to the backend database.
    #[error("Failed to add a new version to backend database {:?}", path.display())]
    AddVersion { path: PathBuf, source: diesel::result::Error },
    /// Failed to deserialize the given content from JSON.
    #[error("Failed to deserialize the given content of policy {name:?} ({version}) from JSON")]
    ContentDeserialize { name: String, version: u64, source: serde_json::Error },
    /// Failed to serialize the given content as JSON.
    #[error("Failed to serialize the content of policy {name:?} as JSON")]
    ContentSerialize { name: String, source: serde_json::Error },
    /// Failed to deactivate an active version.
    #[error("Failed to deactivate active policy version {version} in backend database {:?}", path.display())]
    DeactivateVersion { path: PathBuf, version: u64, source: diesel::result::Error },
    /// Failed to fetch the active version.
    #[error("Failed to get active version from backend database {:?}", path.display())]
    GetActiveVersion { path: PathBuf, source: diesel::result::Error },
    /// Failed to fetch the latest version.
    #[error("Failed to get latest version from backend database {:?}", path.display())]
    GetLatestVersion { path: PathBuf, source: diesel::result::Error },
    /// Failed to get a specific version.
    #[error("Failed to get version {version} from backend database {:?}", path.display())]
    GetVersion { path: PathBuf, version: u64, source: diesel::result::Error },
    /// Failed to get the list of versions.
    #[error("Failed to get the list of versions from backend database {:?}", path.display())]
    GetVersions { path: PathBuf, source: diesel::result::Error },
    /// Failed to set the currently active policy.
    #[error("Failed to set version {version} as the active policy in backend database {:?}", path.display())]
    SetActive { path: PathBuf, version: u64, source: diesel::result::Error },
    /// Failed to spawn a background blocking task.
    #[error("Failed to spawn a blocking task")]
    SpawnBlocking { source: tokio::task::JoinError },
    /// Failed to start a transaction with the database.
    #[error("Failed to start a transaction with the backend database")]
    Transaction { source: diesel::result::Error },
}
// Note: implemented to always error for transaction
impl From<diesel::result::Error> for ConnectionError {
    #[inline]
    fn from(value: diesel::result::Error) -> Self { Self::Transaction { source: value } }
}





/***** LIBRARY *****/
/// A [`DatabaseConnector`] that can interface with SQLite databases.
#[derive(Clone)]
pub struct SQLiteDatabase<C> {
    /// The path to the file that we represent. Only retained during runtime for debugging.
    path:     PathBuf,
    /// The pool of connections.
    pool:     Pool<deadpool_diesel::Manager<SqliteConnection>>,
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
                    fs::create_dir(&dir).await.map_err(|source| DatabaseError::DatabaseDirCreate { path: dir.into(), source })?;
                }
            }
            fs::File::create(&path).await.map_err(|source| DatabaseError::DatabaseCreate { path: path.clone(), source })?;

            // Apply them by connecting to the database
            let mut conn = SqliteConnection::establish(&path.display().to_string())
                .map_err(|source| DatabaseError::ConnectDatabase { path: path.clone(), source })?;
            conn.run_pending_migrations(migrations).map_err(|source| DatabaseError::MigrationsApply { path: path.clone(), source })?;
        } else {
            debug!("Database {:?} already exists", path.display());
        }

        // Create the pool
        debug!("Connecting to database {:?}...", path.display());
        let manager = Manager::new(path.display().to_string(), deadpool::Runtime::Tokio1);
        let pool = Pool::builder(manager).build().map_err(|source| DatabaseError::PoolCreate { path: path.clone(), source })?;

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
        let migrations = FileBasedMigrations::find_migrations_directory_in_path(migrations_dir)
            .map_err(|source| DatabaseError::MigrationsFind { migrations_dir: migrations_dir.into(), source })?;

        // Delegate to the normal one
        Self::new_async(path, migrations).await
    }
}
impl<C: Send + Sync + DeserializeOwned + Serialize + 'static> DatabaseConnector for SQLiteDatabase<C> {
    type Connection<'s>
        = SQLiteConnection<'s, C>
    where
        Self: 's;
    type Content = C;
    type Error = DatabaseError;

    #[inline]
    async fn connect<'s>(&'s self, user: &'s specifications::metadata::User) -> Result<Self::Connection<'s>, Self::Error> {
        // Attempt to get a connection from the pool
        debug!("Creating new connection to SQLite database {:?}...", self.path.display());
        let conn = self.pool.get().await.map_err(|source| DatabaseError::Connect { path: self.path.clone(), source })?;

        Ok(SQLiteConnection { path: &self.path, conn, user, _content: PhantomData })
    }
}



/// Represents the connection created by [`SQLiteDatabase::connect()`].
pub struct SQLiteConnection<'a, C> {
    /// The path to the file that we represent. Only retained during runtime for debugging.
    path:     &'a Path,
    /// The connection we wrap.
    conn:     Object<Manager<SqliteConnection>>,
    /// The user that is doing everything in this connection.
    user:     &'a User,
    /// Remembers the type of content chosen for this connection.
    _content: PhantomData<C>,
}
impl<C> SQLiteConnection<'_, C> {
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
        debug!("Fetching active version...");
        let mut result = crate::schema::active_version::dsl::active_version
            .limit(1)
            .order_by(crate::schema::active_version::dsl::activated_on.desc())
            .select(SqliteActiveVersion::as_select())
            .load(conn)
            .map_err(|source| ConnectionError::GetActiveVersion { path: path.into(), source })?;

        let active_version =
            result.pop().and_then(|last_version| if last_version.deactivated_on.is_some() { None } else { Some(last_version.version as u64) });

        Ok(active_version)
    }
}
impl<C: Send + Sync + DeserializeOwned + Serialize + 'static> DatabaseConnection for SQLiteConnection<'_, C> {
    type Content = C;
    type Error = ConnectionError;


    // Mutable
    #[instrument(name = "SQLiteConnection::add_version", skip_all, fields(policy = metadata.name))]
    async fn add_version(&mut self, metadata: AttachedMetadata, content: Self::Content) -> Result<u64, Self::Error> {
        use crate::schema::policies::dsl::policies;

        debug!("Starting transaction...");
        let user_id = self.user.id.clone();
        let path = self.path.to_owned();
        self.conn
            .interact(move |conn| {
                conn.exclusive_transaction(|conn| -> Result<u64, Self::Error> {
                    debug!("Retrieving latest policy version...");
                    let latest: i64 = policies::select(policies, crate::schema::policies::dsl::version)
                        .order_by(crate::schema::policies::dsl::created_at.desc())
                        .limit(1)
                        .load(conn)
                        .map_err(|source| ConnectionError::GetLatestVersion { path: path.clone(), source })?
                        .pop()
                        .unwrap_or(0);

                    // up to next version
                    let next_version: i64 = latest + 1;

                    // Construct the policy itself
                    debug!("Adding new policy {next_version}...");
                    let content = serde_json::to_string(&content)
                        .map_err(|source| ConnectionError::ContentSerialize { name: metadata.name.clone(), source })?;
                    let model = SqlitePolicy {
                        name: metadata.name,
                        description: metadata.description,
                        language: metadata.language,
                        version: next_version,
                        creator: user_id,
                        created_at: Utc::now().naive_utc(),
                        content,
                    };

                    // Submit it
                    diesel::insert_into(policies).values(&model).execute(conn).map_err(|source| ConnectionError::AddVersion { path, source })?;

                    Ok(next_version as u64)
                })
            })
            .await
            .expect("database transaction should not panic")
    }

    #[instrument(name = "SQLiteConnection::activate", skip(self))]
    async fn activate(&mut self, version: u64) -> Result<(), Self::Error> {
        use crate::schema::active_version::dsl::active_version;

        debug!("Starting transaction...");
        let path = self.path.to_owned();
        let user_id = self.user.id.clone();
        self.conn
            .interact(move |conn| {
                conn.exclusive_transaction(|conn| -> Result<(), Self::Error> {
                    // Get the information about what to activate
                    let av = Self::_get_active_version(&path, conn)?;

                    // They may already be the same, ez
                    if av.is_some_and(|v| v == version) {
                        info!("Activated already-active version {version}");
                        return Ok(());
                    }

                    // Otherwise, build the model and submit it
                    debug!("Activating policy {version}...");
                    let model = SqliteActiveVersion::new(version as i64, user_id);
                    diesel::insert_into(active_version).values(&model).execute(conn).map_err(|source| ConnectionError::SetActive {
                        path,
                        version,
                        source,
                    })?;
                    Ok(())
                })
            })
            .await
            .expect("database transaction should not panic")
    }

    #[instrument(name = "SQLiteConnection::deactivate", skip(self))]
    async fn deactivate(&mut self) -> Result<(), Self::Error> {
        use crate::schema::active_version::dsl::{active_version, deactivated_by, deactivated_on, version};

        debug!("Starting transaction...");
        let path = self.path.to_owned();
        let user_id = self.user.id.clone();
        self.conn
            .interact(move |conn| {
                conn.exclusive_transaction(|conn| -> Result<(), Self::Error> {
                    // Get the current active version, if any
                    let av = match Self::_get_active_version(&path, conn)? {
                        Some(av) => av,
                        None => {
                            info!("Deactivated a policy whilst none were active");
                            return Ok(());
                        },
                    };

                    // If we found one, then update it
                    debug!("Deactivating active policy {av}...");
                    diesel::update(active_version)
                        .filter(version.eq(av as i64))
                        .set((deactivated_on.eq(Utc::now().naive_local()), deactivated_by.eq(&user_id)))
                        .execute(conn)
                        .map_err(|source| ConnectionError::DeactivateVersion { path, version: av, source })?;
                    Ok(())
                })
            })
            .await
            .expect("database transaction should not panic")
    }


    // Immutable
    #[instrument(name = "SQLiteConnection::get_versions", skip(self))]
    async fn get_versions(&mut self) -> Result<HashMap<u64, Metadata>, Self::Error> {
        use crate::schema::policies::dsl as policy;

        let path = self.path.to_owned();
        self.conn
            .interact(move |conn| {
                debug!("Retrieving all policy versions...");
                let r = policy::policies
                    .order_by(crate::schema::policies::dsl::created_at.desc())
                    .select((policy::description, policy::name, policy::language, policy::version, policy::creator, policy::created_at))
                    .load::<(String, String, String, i64, String, NaiveDateTime)>(conn)
                    .map_err(|source| ConnectionError::GetVersions { path, source })?
                    .into_iter()
                    .map(|(description, name, language, version, creator, created_at)| {
                        (version as u64, Metadata {
                            attached: AttachedMetadata { name, description, language },
                            version:  version as u64,
                            creator:  User { id: creator, name: "John Smith".into() },
                            created:  created_at.and_utc(),
                        })
                    })
                    .collect();

                Ok(r)
            })
            .await
            .expect("database transaction should not panic")
    }

    #[instrument(name = "SQLiteConnection::get_active", skip(self))]
    async fn get_active_version(&mut self) -> Result<Option<u64>, Self::Error> {
        // Do a call to get the active, if any
        let path = self.path.to_owned();
        self.conn.interact(move |conn| Self::_get_active_version(&path, conn)).await.expect("database transaction should not panic")
    }

    #[instrument(name = "SQLiteConnection::get_active", skip(self))]
    async fn get_activator(&mut self) -> Result<Option<User>, Self::Error> {
        use crate::schema::active_version::dsl::active_version;

        // Do a call to get the active, if any
        debug!("Fetching active version...");
        let path = self.path.to_owned();
        self.conn
            .interact(move |conn| {
                let mut r = active_version
                    .limit(1)
                    .order_by(crate::schema::active_version::dsl::activated_on.desc())
                    .select(SqliteActiveVersion::as_select())
                    .load(conn)
                    .map_err(|source| ConnectionError::GetActiveVersion { path, source })?;


                Ok(r.pop()
                    .and_then(|av| if av.deactivated_on.is_some() { None } else { Some(User { id: av.activated_by, name: "John Smith".into() }) }))
            })
            .await
            .expect("database transaction should not panic")
    }

    #[instrument(name = "SQLiteConnection::get_version_metadata", skip(self))]
    async fn get_version_metadata(&mut self, version: u64) -> Result<Option<Metadata>, Self::Error> {
        use crate::schema::policies::dsl as policy;

        debug!("Retrieving metadata for version {version}...");
        let path = self.path.to_owned();
        self.conn
            .interact(move |conn| {
                let mut r = match policy::policies
                    .limit(1)
                    .filter(crate::schema::policies::dsl::version.eq(version as i64))
                    .order_by(crate::schema::policies::dsl::created_at.desc())
                    .select((policy::description, policy::name, policy::language, policy::version, policy::creator, policy::created_at))
                    .load::<(String, String, String, i64, String, NaiveDateTime)>(conn)
                {
                    Ok(r) => r,
                    Err(err) => {
                        return match err {
                            diesel::result::Error::NotFound => Ok(None),
                            err => Err(ConnectionError::GetVersion { path, version, source: err }),
                        };
                    },
                };

                // Extract the version itself
                let Some((description, name, language, version, creator, created_at)) = r.pop() else {
                    return Ok(None);
                };

                // Done, return the thing
                Ok(Some(Metadata {
                    attached: AttachedMetadata { name, description, language },
                    created:  created_at.and_utc(),
                    creator:  User { id: creator, name: "John Smith".into() },
                    version:  version as u64,
                }))
            })
            .await
            .expect("database transaction should not panic")
    }

    #[instrument(name = "SQLiteConnection::get_version_content", skip_all)]
    async fn get_version_content(&mut self, version: u64) -> Result<Option<Self::Content>, Self::Error> {
        use crate::schema::policies::dsl as policy;

        let path = self.path.to_owned();
        self.conn
            .interact(move |conn| {
                debug!("Retrieving content for version {version}...");
                let mut r = match policy::policies
                    .limit(1)
                    .filter(crate::schema::policies::dsl::version.eq(version as i64))
                    .order_by(crate::schema::policies::dsl::created_at.desc())
                    .select((policy::name, policy::content))
                    .load::<(String, String)>(conn)
                {
                    Ok(r) => r,
                    Err(err) => {
                        return match err {
                            diesel::result::Error::NotFound => Ok(None),
                            err => Err(ConnectionError::GetVersion { path, version, source: err }),
                        };
                    },
                };

                // Extract the version itself
                let Some((name, content)) = r.pop() else {
                    return Ok(None);
                };

                // Deserialize the content
                let content = serde_json::from_str(&content).map_err(|source| ConnectionError::ContentDeserialize { name, version, source })?;

                Ok(Some(content))
            })
            .await
            .expect("database transaction should not panic")
    }
}
