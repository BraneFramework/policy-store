//  PATHS.rs
//    by Lut99
//
//  Created:
//    23 Oct 2024, 11:56:03
//  Last edited:
//    02 Dec 2024, 16:53:10
//  Auto updated?
//    Yes
//
//  Description:
//!   Implements the handlers for the various API paths.
//

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, Request, State};
use axum::http::StatusCode;
use axum::Extension;
use error_trace::trace;
use futures::StreamExt;
use serde::de::DeserializeOwned;
use serde::Serialize;
use specifications::databaseconn::DatabaseConnection;
use specifications::metadata::{Metadata, User};
use specifications::DatabaseConnector;
use tracing::{error, info, span, Level};

use crate::server::AxumServer;
use crate::spec::{
    ActivateRequest, AddVersionRequest, AddVersionResponse, GetActivatorResponse, GetActiveVersionResponse, GetVersionContentResponse,
    GetVersionMetadataResponse, GetVersionsResponse,
};


/***** HELPER FUNCTIONS *****/
/// Turns the given [`Request`] into a deserialized object.
///
/// This is done instead of using the [`Json`](axum::extract::Json) extractor because we want to
/// log the raw inputs upon failure.
///
/// # Generics
/// - `T`: The thing to deserialize to.
///
/// # Arguments
/// - `request`: The [`Request`] to download and turn into JSON.
///
/// # Returns
/// A parsed `T`.
///
/// # Errors
/// This function errors if we failed to download the request body, or it was not valid JSON.
async fn download_request<T: DeserializeOwned>(request: Request) -> Result<T, (StatusCode, String)> {
    // Download the entire request first
    let mut req: Vec<u8> = Vec::new();
    let mut request = request.into_body().into_data_stream();
    while let Some(next) = request.next().await {
        // Unwrap the chunk
        let next: Bytes = match next {
            Ok(next) => next,
            Err(err) => {
                let msg: &'static str = "Failed to download request body";
                error!("{}", trace!(("{msg}"), err));
                return Err((StatusCode::INTERNAL_SERVER_ERROR, msg.into()));
            },
        };

        // Append it
        req.extend(next);
    }

    // Deserialize the request contents
    match serde_json::from_slice(&req) {
        Ok(req) => Ok(req),
        Err(err) => {
            let error: String = format!(
                "{}Raw body:\n{}\n{}\n{}\n",
                trace!(("Failed to deserialize request body"), err),
                (0..80).map(|_| '-').collect::<String>(),
                String::from_utf8_lossy(&req),
                (0..80).map(|_| '-').collect::<String>()
            );
            info!("{error}");
            Err((StatusCode::BAD_REQUEST, error))
        },
    }
}





/***** LIBRARIES *****/
impl<A, D> AxumServer<A, D>
where
    A: 'static + Send + Sync,
    D: 'static + Send + Sync + DatabaseConnector,
    D::Content: Send + DeserializeOwned + Serialize,
    for<'s> D::Connection<'s>: Send,
{
    /// Handler for `POST /v2/policies` (i.e., uploading a new policy).
    ///
    /// In:
    /// - [`AddVersionRequest<D::Content>`](AddVersionRequest).
    ///
    /// Out:
    /// - 200 OK with an [`AddVersionResponse`] detailling the version number of the new policy;
    /// - 404 BAD REQUEST with the reason why we failed to parse the request; or
    /// - 500 INTERNAL SERVER ERROR with a message what went wrong.
    pub fn add_version(
        State(this): State<Arc<Self>>,
        Extension(auth): Extension<User>,
        request: Request,
    ) -> impl 'static + Send + Future<Output = (StatusCode, String)> {
        async move {
            let _span = span!(Level::INFO, "AxumServer::add_version", user = auth.id);

            // Get the request
            let req: AddVersionRequest<D::Content> = match download_request(request).await {
                Ok(req) => req,
                Err(res) => return res,
            };

            // Just try to send it to the DB
            let mut conn: D::Connection<'_> = match this.data.connect(&auth).await {
                Ok(conn) => conn,
                Err(err) => {
                    let msg: String = format!("Failed to add policy {}", req.metadata.name);
                    error!("{}", trace!(("{msg}"), err));
                    return (StatusCode::INTERNAL_SERVER_ERROR, msg);
                },
            };
            let name: String = req.metadata.name.clone();
            let version: u64 = match conn.add_version(req.metadata, req.contents).await {
                Ok(res) => res,
                Err(err) => {
                    let msg: String = format!("Failed to add policy {name}");
                    error!("{}", trace!(("{msg}"), err));
                    return (StatusCode::INTERNAL_SERVER_ERROR, msg);
                },
            };

            // Return the version
            (StatusCode::OK, serde_json::to_string(&AddVersionResponse { version }).unwrap())
        }
    }

    /// Handler for `PUT /v2/policies/active` (i.e., activating a policy).
    ///
    /// In:
    /// - An unsigned 64-bit integer representing the policy to activate.
    ///
    /// Out:
    /// - 200 OK;
    /// - 404 BAD REQUEST with the reason why we failed to parse the request; or
    /// - 500 INTERNAL SERVER ERROR with a message what went wrong.
    pub fn activate(
        State(this): State<Arc<Self>>,
        Extension(auth): Extension<User>,
        request: Request,
    ) -> impl 'static + Send + Future<Output = (StatusCode, String)> {
        async move {
            let _span = span!(Level::INFO, "AxumServer::activate", user = auth.id);

            // Get the request
            let version: ActivateRequest = match download_request(request).await {
                Ok(req) => req,
                Err(res) => return res,
            };

            // Just try to send it to the DB
            let mut conn: D::Connection<'_> = match this.data.connect(&auth).await {
                Ok(conn) => conn,
                Err(err) => {
                    let msg: String = format!("Failed to activate policy {}", version.version);
                    error!("{}", trace!(("{msg}"), err));
                    return (StatusCode::INTERNAL_SERVER_ERROR, msg);
                },
            };
            if let Err(err) = conn.activate(version).await {
                let msg: String = format!("Failed to activate policy {version}");
                error!("{}", trace!(("{msg}"), err));
                return (StatusCode::INTERNAL_SERVER_ERROR, msg);
            };

            // Done
            (StatusCode::OK, String::new())
        }
    }

    /// Handler for `DELETE /v2/policies/active` (i.e., deactivating a policy).
    ///
    /// Out:
    /// - 200 OK; or
    /// - 500 INTERNAL SERVER ERROR with a message what went wrong.
    pub fn deactivate(
        State(this): State<Arc<Self>>,
        Extension(auth): Extension<User>,
    ) -> impl 'static + Send + Future<Output = (StatusCode, String)> {
        async move {
            let _span = span!(Level::INFO, "AxumServer::deactivate", user = auth.id);

            // Just try to send it to the DB
            let mut conn: D::Connection<'_> = match this.data.connect(&auth).await {
                Ok(conn) => conn,
                Err(err) => {
                    let msg: String = format!("Failed to deactivate any active policy");
                    error!("{}", trace!(("{msg}"), err));
                    return (StatusCode::INTERNAL_SERVER_ERROR, msg);
                },
            };
            if let Err(err) = conn.deactivate().await {
                let msg: String = format!("Failed to deactivate any active policy");
                error!("{}", trace!(("{msg}"), err));
                return (StatusCode::INTERNAL_SERVER_ERROR, msg);
            };

            // Done
            (StatusCode::OK, String::new())
        }
    }



    /// Handler for `GET /v2/policies` (i.e., listing all policy).
    ///
    /// Out:
    /// - 200 OK with an [`GetVersionsResponse`] mapping version numbers ([`u64`]) to [`Metadata`];
    ///   or
    /// - 500 INTERNAL SERVER ERROR with a message what went wrong.
    pub fn get_versions(
        State(this): State<Arc<Self>>,
        Extension(auth): Extension<User>,
    ) -> impl 'static + Send + Future<Output = (StatusCode, String)> {
        async move {
            let _span = span!(Level::INFO, "AxumServer::get_versions", user = auth.id);

            // Just try to send it to the DB
            let mut conn: D::Connection<'_> = match this.data.connect(&auth).await {
                Ok(conn) => conn,
                Err(err) => {
                    let msg: String = format!("Failed to deactivate any active policy");
                    error!("{}", trace!(("{msg}"), err));
                    return (StatusCode::INTERNAL_SERVER_ERROR, msg);
                },
            };
            let versions: HashMap<u64, Metadata> = match conn.get_versions().await {
                Ok(versions) => versions,
                Err(err) => {
                    let msg: String = format!("Failed to deactivate any active policy");
                    error!("{}", trace!(("{msg}"), err));
                    return (StatusCode::INTERNAL_SERVER_ERROR, msg);
                },
            };

            // Serialize the result
            match serde_json::to_string(&GetVersionsResponse { versions }) {
                Ok(versions) => (StatusCode::OK, versions),
                Err(err) => {
                    let msg: String = format!("Failed to serialize result");
                    error!("{}", trace!(("{msg}"), err));
                    (StatusCode::INTERNAL_SERVER_ERROR, msg)
                },
            }
        }
    }

    /// Handler for `GET /v2/policies/active` (i.e., get active policy).
    ///
    /// Out:
    /// - 200 OK with a [`GetActiveVersionResponse`] describing the version; or
    /// - 500 INTERNAL SERVER ERROR with a message what went wrong.
    pub fn get_active_version(
        State(this): State<Arc<Self>>,
        Extension(auth): Extension<User>,
    ) -> impl 'static + Send + Future<Output = (StatusCode, String)> {
        async move {
            let _span = span!(Level::INFO, "AxumServer::get_active_version", user = auth.id);

            // Just try to send it to the DB
            let mut conn: D::Connection<'_> = match this.data.connect(&auth).await {
                Ok(conn) => conn,
                Err(err) => {
                    let msg: String = format!("Failed to get active policy");
                    error!("{}", trace!(("{msg}"), err));
                    return (StatusCode::INTERNAL_SERVER_ERROR, msg);
                },
            };
            let version: Option<u64> = match conn.get_active_version().await {
                Ok(version) => version,
                Err(err) => {
                    let msg: String = format!("Failed to get active policy");
                    error!("{}", trace!(("{msg}"), err));
                    return (StatusCode::INTERNAL_SERVER_ERROR, msg);
                },
            };

            // Serialize the result
            match serde_json::to_string(&GetActiveVersionResponse { version }) {
                Ok(res) => (StatusCode::OK, res),
                Err(err) => {
                    let msg: String = format!("Failed to serialize result");
                    error!("{}", trace!(("{msg}"), err));
                    (StatusCode::INTERNAL_SERVER_ERROR, msg)
                },
            }
        }
    }

    /// Handler for `GET /v2/policies/active/activator` (i.e., get activator).
    ///
    /// Out:
    /// - 200 OK with a [`GetActivatorResponse`] describing the version; or
    /// - 500 INTERNAL SERVER ERROR with a message what went wrong.
    pub fn get_activator(
        State(this): State<Arc<Self>>,
        Extension(auth): Extension<User>,
    ) -> impl 'static + Send + Future<Output = (StatusCode, String)> {
        async move {
            let _span = span!(Level::INFO, "AxumServer::get_activator", user = auth.id);

            // Just try to send it to the DB
            let mut conn: D::Connection<'_> = match this.data.connect(&auth).await {
                Ok(conn) => conn,
                Err(err) => {
                    let msg: String = format!("Failed to get activator");
                    error!("{}", trace!(("{msg}"), err));
                    return (StatusCode::INTERNAL_SERVER_ERROR, msg);
                },
            };
            let user: Option<User> = match conn.get_activator().await {
                Ok(user) => user,
                Err(err) => {
                    let msg: String = format!("Failed to get activator");
                    error!("{}", trace!(("{msg}"), err));
                    return (StatusCode::INTERNAL_SERVER_ERROR, msg);
                },
            };

            // Serialize the result
            match serde_json::to_string(&GetActivatorResponse { user }) {
                Ok(versions) => (StatusCode::OK, versions),
                Err(err) => {
                    let msg: String = format!("Failed to serialize result");
                    error!("{}", trace!(("{msg}"), err));
                    (StatusCode::INTERNAL_SERVER_ERROR, msg)
                },
            }
        }
    }

    /// Handler for `GET /v2/policy/:version` (i.e., get version metadata).
    ///
    /// Out:
    /// - 200 OK with a [`GetVersionMetadataResponse`] describing the version's metadata;
    /// - 404 NOT FOUND if there was no policy with version `:version`; or
    /// - 500 INTERNAL SERVER ERROR with a message what went wrong.
    pub fn get_version_metadata(
        State(this): State<Arc<Self>>,
        Extension(auth): Extension<User>,
        Path(version): Path<u64>,
    ) -> impl 'static + Send + Future<Output = (StatusCode, String)> {
        async move {
            let _span = span!(Level::INFO, "AxumServer::get_version_metadata", user = auth.id);

            // Just try to send it to the DB
            let mut conn: D::Connection<'_> = match this.data.connect(&auth).await {
                Ok(conn) => conn,
                Err(err) => {
                    let msg: String = format!("Failed to get policy metadata");
                    error!("{}", trace!(("{msg}"), err));
                    return (StatusCode::INTERNAL_SERVER_ERROR, msg);
                },
            };
            let metadata: Metadata = match conn.get_version_metadata(version).await {
                Ok(Some(metadata)) => metadata,
                Ok(None) => {
                    return (StatusCode::NOT_FOUND, String::new());
                },
                Err(err) => {
                    let msg: String = format!("Failed to get policy metadata");
                    error!("{}", trace!(("{msg}"), err));
                    return (StatusCode::INTERNAL_SERVER_ERROR, msg);
                },
            };

            // Serialize the result
            match serde_json::to_string(&GetVersionMetadataResponse { metadata }) {
                Ok(versions) => (StatusCode::OK, versions),
                Err(err) => {
                    let msg: String = format!("Failed to serialize result");
                    error!("{}", trace!(("{msg}"), err));
                    (StatusCode::INTERNAL_SERVER_ERROR, msg)
                },
            }
        }
    }

    /// Handler for `GET /v2/policy/:version/content` (i.e., get version content).
    ///
    /// Out:
    /// - 200 OK with a [`GetVersionContentResponse<D::Content>`](GetVersionContentResponse)
    ///   describing the version's content;
    /// - 404 NOT FOUND if there was no policy with version `:version`; or
    /// - 500 INTERNAL SERVER ERROR with a message what went wrong.
    pub fn get_version_content(
        State(this): State<Arc<Self>>,
        Extension(auth): Extension<User>,
        Path(version): Path<u64>,
    ) -> impl 'static + Send + Future<Output = (StatusCode, String)> {
        async move {
            let _span = span!(Level::INFO, "AxumServer::get_version_content", user = auth.id);

            // Just try to send it to the DB
            let mut conn: D::Connection<'_> = match this.data.connect(&auth).await {
                Ok(conn) => conn,
                Err(err) => {
                    let msg: String = format!("Failed to get policy content");
                    error!("{}", trace!(("{msg}"), err));
                    return (StatusCode::INTERNAL_SERVER_ERROR, msg);
                },
            };
            let content: D::Content = match conn.get_version_content(version).await {
                Ok(Some(content)) => content,
                Ok(None) => {
                    return (StatusCode::NOT_FOUND, String::new());
                },
                Err(err) => {
                    let msg: String = format!("Failed to get policy content");
                    error!("{}", trace!(("{msg}"), err));
                    return (StatusCode::INTERNAL_SERVER_ERROR, msg);
                },
            };

            // Serialize the result
            match serde_json::to_string(&GetVersionContentResponse { content }) {
                Ok(content) => (StatusCode::OK, content),
                Err(err) => {
                    let msg: String = format!("Failed to serialize result");
                    error!("{}", trace!(("{msg}"), err));
                    (StatusCode::INTERNAL_SERVER_ERROR, msg)
                },
            }
        }
    }
}
