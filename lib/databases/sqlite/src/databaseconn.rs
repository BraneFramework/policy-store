//  DATABASECONN.rs
//    by Lut99
//
//  Created:
//    22 Oct 2024, 14:37:56
//  Last edited:
//    22 Oct 2024, 16:24:19
//  Auto updated?
//    Yes
//
//  Description:
//!   Implements the actual [`DatabaseConnector`].
//

use std::future::Future;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use chrono::{DateTime, TimeZone, Utc};
use diesel::connection::LoadConnection;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::sqlite::Sqlite;
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, SelectableHelper as _, SqliteConnection};
use serde::de::DeserializeOwned;
use serde::Serialize;
use specifications::databaseconn::DatabaseConnection;
use specifications::metadata::{AttachedMetadata, Metadata, User};
use specifications::DatabaseConnector;
use thiserror::Error;
use tracing::{debug, info, span, Level};

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
    /// Failed to serialize the given content as JSON.
    #[error("Failed to serialize the given content as JSON")]
    SerializeContent {
        #[source]
        err: serde_json::Error,
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
    /// Encountered an non-existing version.
    #[error("Version {version:?} not found in backend database {:?}", path.display())]
    VersionNotFound { path: PathBuf, version: u64 },
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
impl<C: DeserializeOwned + Serialize> DatabaseConnector for SQLiteDatabase<C> {
    type Connection<'s> = SQLiteConnection<'s, C> where Self: 's;
    type Error = DatabaseError;

    #[inline]
    fn connect<'s>(&'s self, user: &'s specifications::metadata::User) -> impl Future<Output = Result<Self::Connection<'s>, Self::Error>> {
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

    /// Helper function for doing the non-async version metadata retrieval.
    ///
    /// # Arguments
    /// - `path`: The path where the backend SQLite database lives. Only given for debugging purposes.
    /// - `conn`: Some [`LoadConnection`] that we use to talk to the file.
    /// - `version`: The version to (hopefully) retrieve.
    ///
    /// # Returns
    /// The [`Metadata`] of the given `version`.
    ///
    /// # Errors
    /// This function errors if we failed to get the active version or if we didn't find the version.
    fn _get_version_metadata<C2>(path: &Path, conn: &mut C2, version: u64) -> Result<Metadata, ConnectionError>
    where
        C2: LoadConnection<Backend = Sqlite>,
    {
        use crate::schema::policies::dsl::policies;

        debug!("Retrieving metadata for version {version}...");
        match policies
            .limit(1)
            .filter(crate::schema::policies::dsl::version.eq(version as i64))
            .order_by(crate::schema::policies::dsl::created_at.desc())
            .select(SqlitePolicy::as_select())
            .load::<SqlitePolicy>(conn)
        {
            Ok(mut r) => {
                // Extract the version itself
                if r.len() < 1 {
                    return Err(ConnectionError::VersionNotFound { path: path.into(), version });
                }
                let item: SqlitePolicy = r.remove(0);

                // Done, return the thing
                Ok(Metadata {
                    attached: AttachedMetadata { name: item.name, description: item.description },
                    created:  item.created_at.and_utc(),
                    creator:  User { id: item.creator, name: "John Smith".into() },
                    version:  item.version as u64,
                })
            },
            Err(err) => Err(match err {
                diesel::result::Error::NotFound => ConnectionError::VersionNotFound { path: path.into(), version },
                err => ConnectionError::GetVersion { path: path.into(), version, err },
            }),
        }
    }
}
impl<'a, C: DeserializeOwned + Serialize> DatabaseConnection for SQLiteConnection<'a, C> {
    type Content = C;
    type Error = ConnectionError;


    // Mutable
    fn add_version(&mut self, metadata: AttachedMetadata, content: Self::Content) -> impl Future<Output = Result<u64, Self::Error>> {
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
                    let content = match serde_json::to_string(&content) {
                        Ok(content) => content,
                        Err(err) => return Err(ConnectionError::SerializeContent { err }),
                    };
                    let model = SqlitePolicy {
                        name: metadata.name,
                        description: metadata.description,
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

    fn activate(&mut self, version: u64) -> impl Future<Output = Result<(), Self::Error>> {
        use crate::schema::active_version::dsl::active_version;

        async move {
            let span = span!(Level::INFO, "SQLiteConnection::activate", version = version);

            debug!("Starting transaction...");
            tokio::task::block_in_place(move || {
                self.conn.exclusive_transaction(|conn| -> Result<(), Self::Error> {
                    // Trick the compiler into moving the span too
                    let _span = span;

                    // Get the information about what to activate
                    let policy = Self::_get_version_metadata(&self.path, conn, version)?;
                    let av = Self::_get_active_version(&self.path, conn)?;

                    // They may already be the same, ez
                    if av.is_some_and(|v| v == version) {
                        info!("Activated already-active version {version}");
                        return Ok(());
                    }

                    // Otherwise, build the model and submit it
                    let model = SqliteActiveVersion::new(version as i64, self.user.id.clone());
                    if let Err(err) = diesel::insert_into(active_version).values(&model).execute(conn) {
                        return Err(ConnectionError::SetActive { path: self.path.into(), version, err });
                    }
                    Ok(())
                })
            })
        }
    }

    fn deactivate(&mut self) -> impl Future<Output = Result<(), Self::Error>> {
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
    fn get_versions(&mut self) -> impl Future<Output = Result<HashMap<u64, Metadata>, Self::Error>> {
        use crate::schema::policies::dsl::{created_at, creator, policies, reasoner_connector_context, version, version_description};
        let mut conn = self.pool.get().unwrap();

        match policies
            .order_by(crate::schema::policies::dsl::created_at.desc())
            .select((version, version_description, creator, created_at, reasoner_connector_context))
            .load::<(i64, String, String, i64, String)>(&mut conn)
        {
            Ok(r) => {
                let items: Vec<PolicyVersion> = r
                    .into_iter()
                    .map(|x| PolicyVersion {
                        version: Some(x.0),
                        version_description: x.1,
                        creator: Some(x.2),
                        created_at: DateTime::from_timestamp_micros(x.3).unwrap().into(),
                        reasoner_connector_context: x.4,
                    })
                    .collect();

                return Ok(items);
            },
            Err(err) => Err(match err {
                Error::NotFound => PolicyDataError::NotFound,
                _ => PolicyDataError::GeneralError(err.to_string()),
            }),
        }
    }

    fn get_active_version(&mut self) -> impl Future<Output = Result<Option<u64>, Self::Error>> {
        async move {
            let _span = span!(Level::INFO, "SQLiteConnection::get_active");

            // Do a call to get the active, if any
            tokio::task::block_in_place(move || Self::_get_active_version(&self.path, &mut self.conn))
        }
    }

    fn get_activator(&mut self) -> impl Future<Output = Result<Option<User>, Self::Error>> { todo!() }

    fn get_version_metadata(&mut self, version: u64) -> impl Future<Output = Result<Metadata, Self::Error>> {
        async move {
            let _span = span!(Level::INFO, "SQLiteConnection::get_version_metadata", version = version);

            tokio::task::block_in_place(move || Self::_get_version_metadata(&self.path, &mut self.conn, version))
        }
    }

    fn get_version_content(&mut self, version: u64) -> impl Future<Output = Result<Self::Content, Self::Error>> {
        async move {
            let _span = span!(Level::INFO, "SQLiteConnection::get_version_metadata", version = version);

            tokio::task::block_in_place(move || Self::_get_version_metadata(&self.path, &mut self.conn, version))
        }
    }
}
