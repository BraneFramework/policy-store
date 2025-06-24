//  PATHS.rs
//    by Lut99
//
//  Created:
//    23 Oct 2024, 11:56:03
//  Last edited:
//    06 Dec 2024, 14:38:58
//  Auto updated?
//    Yes
//
//  Description:
//!   Implements the handlers for the various API paths.
//

use std::collections::HashMap;
use std::sync::Arc;

use axum::Extension;
use axum::body::Bytes;
use axum::extract::{Path, Request, State};
use axum::http::StatusCode;
use error_trace::trace;
use futures::StreamExt;
use serde::Serialize;
use serde::de::DeserializeOwned;
use specifications::DatabaseConnector;
use specifications::databaseconn::DatabaseConnection;
use specifications::metadata::{Metadata, User};
use tracing::{error, info, instrument};

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
        let next: Bytes = next.map_err(|source| {
            let msg: &'static str = "Failed to download request body";
            error!("{}", trace!(("{msg}"), source));
            (StatusCode::INTERNAL_SERVER_ERROR, msg.into())
        })?;

        // Append it
        req.extend(next);
    }

    // Deserialize the request contents
    serde_json::from_slice(&req).map_err(|source| {
        let error: String = format!(
            "{}Raw body:\n{}\n{}\n{}\n",
            trace!(("Failed to deserialize request body"), source),
            (0..80).map(|_| '-').collect::<String>(),
            String::from_utf8_lossy(&req),
            (0..80).map(|_| '-').collect::<String>()
        );
        info!("{error}");
        (StatusCode::BAD_REQUEST, error)
    })
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
    #[instrument(name = "AxumServer::add_version", skip_all, fields(user = auth.id))]
    pub async fn add_version(
        State(this): State<Arc<Self>>,
        Extension(auth): Extension<User>,
        request: Request,
    ) -> Result<(StatusCode, String), (StatusCode, String)> {
        // Get the request
        let req: AddVersionRequest<D::Content> = download_request(request).await?;

        // Just try to send it to the DB
        let mut conn: D::Connection<'_> = this.data.connect(&auth).await.map_err(|source| {
            let msg: String = format!("Failed to add policy {}", req.metadata.name);
            error!("{}", trace!(("{msg}"), source));
            (StatusCode::INTERNAL_SERVER_ERROR, msg)
        })?;

        let name: String = req.metadata.name.clone();
        let version: u64 = conn.add_version(req.metadata, req.contents).await.map_err(|source| {
            let msg: String = format!("Failed to add policy {name}");
            error!("{}", trace!(("{msg}"), source));
            (StatusCode::INTERNAL_SERVER_ERROR, msg)
        })?;

        // Return the version
        Ok((StatusCode::OK, serde_json::to_string(&AddVersionResponse { version }).unwrap()))
    }

    /// Handler for `PUT /v2/policies/active` (i.e., activating a policy).
    ///
    /// In:
    /// - A [`ActivateRequest`] encoding the policy to activate.
    ///
    /// Out:
    /// - 200 OK;
    /// - 404 BAD REQUEST with the reason why we failed to parse the request; or
    /// - 500 INTERNAL SERVER ERROR with a message what went wrong.
    #[instrument(name = "AxumServer::activate", skip_all, fields(user = auth.id))]
    pub async fn activate(
        State(this): State<Arc<Self>>,
        Extension(auth): Extension<User>,
        request: Request,
    ) -> Result<(StatusCode, String), (StatusCode, String)> {
        // Get the request
        let version: ActivateRequest = download_request(request).await?;

        // Just try to send it to the DB
        let mut conn: D::Connection<'_> = this.data.connect(&auth).await.map_err(|source| {
            let msg: String = format!("Failed to activate policy {}", version.version);
            error!("{}", trace!(("{msg}"), source));
            (StatusCode::INTERNAL_SERVER_ERROR, msg)
        })?;

        conn.activate(version.version).await.map_err(|source| {
            let msg: String = format!("Failed to activate policy {}", version.version);
            error!("{}", trace!(("{msg}"), source));
            (StatusCode::INTERNAL_SERVER_ERROR, msg)
        })?;

        // Done
        Ok((StatusCode::OK, String::new()))
    }

    /// Handler for `DELETE /v2/policies/active` (i.e., deactivating a policy).
    ///
    /// Out:
    /// - 200 OK; or
    /// - 500 INTERNAL SERVER ERROR with a message what went wrong.
    #[instrument(name = "AxumServer::deactivate", skip_all, fields(user = auth.id))]
    pub async fn deactivate(State(this): State<Arc<Self>>, Extension(auth): Extension<User>) -> Result<(StatusCode, String), (StatusCode, String)> {
        // Just try to send it to the DB
        let mut conn: D::Connection<'_> = this.data.connect(&auth).await.map_err(|source| {
            let msg: String = "Failed to deactivate any active policy".to_string();
            error!("{}", trace!(("{msg}"), source));
            (StatusCode::INTERNAL_SERVER_ERROR, msg)
        })?;
        conn.deactivate().await.map_err(|source| {
            let msg: String = "Failed to deactivate any active policy".to_string();
            error!("{}", trace!(("{msg}"), source));
            (StatusCode::INTERNAL_SERVER_ERROR, msg)
        })?;

        // Done
        Ok((StatusCode::OK, String::new()))
    }



    /// Handler for `GET /v2/policies` (i.e., listing all policy).
    ///
    /// Out:
    /// - 200 OK with an [`GetVersionsResponse`] mapping version numbers ([`u64`]) to [`Metadata`];
    ///   or
    /// - 500 INTERNAL SERVER ERROR with a message what went wrong.
    #[instrument(name = "AxumServer::get_versions", skip_all, fields(user = auth.id))]
    pub async fn get_versions(State(this): State<Arc<Self>>, Extension(auth): Extension<User>) -> Result<(StatusCode, String), (StatusCode, String)> {
        // Just try to send it to the DB
        let mut conn: D::Connection<'_> = this.data.connect(&auth).await.map_err(|source| {
            let msg: String = "Failed to deactivate any active policy".to_string();
            error!("{}", trace!(("{msg}"), source));
            (StatusCode::INTERNAL_SERVER_ERROR, msg)
        })?;

        let versions: HashMap<u64, Metadata> = conn.get_versions().await.map_err(|source| {
            let msg: String = "Failed to deactivate any active policy".to_string();
            error!("{}", trace!(("{msg}"), source));
            (StatusCode::INTERNAL_SERVER_ERROR, msg)
        })?;

        // Serialize the result
        let output = serde_json::to_string(&GetVersionsResponse { versions }).map_err(|source| {
            let msg: String = "Failed to serialize result".to_string();
            error!("{}", trace!(("{msg}"), source));
            (StatusCode::INTERNAL_SERVER_ERROR, msg)
        })?;

        Ok((StatusCode::OK, output))
    }

    /// Handler for `GET /v2/policies/active` (i.e., get active policy).
    ///
    /// Out:
    /// - 200 OK with a [`GetActiveVersionResponse`] describing the version; or
    /// - 500 INTERNAL SERVER ERROR with a message what went wrong.
    #[instrument(name = "AxumServer::get_active_version", skip_all, fields(user = auth.id))]
    pub async fn get_active_version(
        State(this): State<Arc<Self>>,
        Extension(auth): Extension<User>,
    ) -> Result<(StatusCode, String), (StatusCode, String)> {
        // Just try to send it to the DB
        let mut conn: D::Connection<'_> = this.data.connect(&auth).await.map_err(|source| {
            let msg: String = "Failed to get active policy".to_string();
            error!("{}", trace!(("{msg}"), source));
            (StatusCode::INTERNAL_SERVER_ERROR, msg)
        })?;

        let version: Option<u64> = conn.get_active_version().await.map_err(|source| {
            let msg: String = "Failed to get active policy".to_string();
            error!("{}", trace!(("{msg}"), source));
            (StatusCode::INTERNAL_SERVER_ERROR, msg)
        })?;

        // Serialize the result
        let res = serde_json::to_string(&GetActiveVersionResponse { version }).map_err(|source| {
            let msg: String = "Failed to serialize result".to_string();
            error!("{}", trace!(("{msg}"), source));
            (StatusCode::INTERNAL_SERVER_ERROR, msg)
        })?;

        Ok((StatusCode::OK, res))
    }

    /// Handler for `GET /v2/policies/active/activator` (i.e., get activator).
    ///
    /// Out:
    /// - 200 OK with a [`GetActivatorResponse`] describing the version; or
    /// - 500 INTERNAL SERVER ERROR with a message what went wrong.
    #[instrument(name = "AxumServer::get_activator", skip_all, fields(user = auth.id))]
    pub async fn get_activator(
        State(this): State<Arc<Self>>,
        Extension(auth): Extension<User>,
    ) -> Result<(StatusCode, String), (StatusCode, String)> {
        // Just try to send it to the DB
        let mut conn: D::Connection<'_> = this.data.connect(&auth).await.map_err(|source| {
            let msg: String = "Failed to get activator".to_string();
            error!("{}", trace!(("{msg}"), source));
            (StatusCode::INTERNAL_SERVER_ERROR, msg)
        })?;

        let user: Option<User> = conn.get_activator().await.map_err(|source| {
            let msg: String = "Failed to get activator".to_string();
            error!("{}", trace!(("{msg}"), source));
            (StatusCode::INTERNAL_SERVER_ERROR, msg)
        })?;

        // Serialize the result
        let activator = serde_json::to_string(&GetActivatorResponse { user }).map_err(|source| {
            let msg: String = "Failed to serialize result".to_string();
            error!("{}", trace!(("{msg}"), source));
            (StatusCode::INTERNAL_SERVER_ERROR, msg)
        })?;

        Ok((StatusCode::OK, activator))
    }

    /// Handler for `GET /v2/policy/:version` (i.e., get version metadata).
    ///
    /// Out:
    /// - 200 OK with a [`GetVersionMetadataResponse`] describing the version's metadata;
    /// - 404 NOT FOUND if there was no policy with version `:version`; or
    /// - 500 INTERNAL SERVER ERROR with a message what went wrong.
    #[instrument(name = "AxumServer::get_version_metadata", skip_all, fields(user = auth.id))]
    pub async fn get_version_metadata(
        State(this): State<Arc<Self>>,
        Extension(auth): Extension<User>,
        Path(version): Path<u64>,
    ) -> Result<(StatusCode, String), (StatusCode, String)> {
        // Just try to send it to the DB
        let mut conn: D::Connection<'_> = this.data.connect(&auth).await.map_err(|source| {
            let msg: String = "Failed to get policy metadata".to_string();
            error!("{}", trace!(("{msg}"), source));
            (StatusCode::INTERNAL_SERVER_ERROR, msg)
        })?;

        let metadata: Metadata = conn
            .get_version_metadata(version)
            .await
            .map_err(|source| {
                let msg: String = "Failed to get policy metadata".to_string();
                error!("{}", trace!(("{msg}"), source));
                (StatusCode::INTERNAL_SERVER_ERROR, msg)
            })?
            .ok_or_else(|| (StatusCode::NOT_FOUND, String::new()))?;

        // Serialize the result
        let metadata = serde_json::to_string(&GetVersionMetadataResponse { metadata }).map_err(|source| {
            let msg: String = "Failed to serialize result".to_string();
            error!("{}", trace!(("{msg}"), source));
            (StatusCode::INTERNAL_SERVER_ERROR, msg)
        })?;

        Ok((StatusCode::OK, metadata))
    }

    /// Handler for `GET /v2/policy/:version/content` (i.e., get version content).
    ///
    /// Out:
    /// - 200 OK with a [`GetVersionContentResponse<D::Content>`](GetVersionContentResponse)
    ///   describing the version's content;
    /// - 404 NOT FOUND if there was no policy with version `:version`; or
    /// - 500 INTERNAL SERVER ERROR with a message what went wrong.
    #[instrument(name = "AxumServer::get_version_content", skip_all, fields(user = auth.id))]
    pub async fn get_version_content(
        State(this): State<Arc<Self>>,
        Extension(auth): Extension<User>,
        Path(version): Path<u64>,
    ) -> Result<(StatusCode, String), (StatusCode, String)> {
        // Just try to send it to the DB
        let mut conn: D::Connection<'_> = this.data.connect(&auth).await.map_err(|source| {
            let msg: String = "Failed to get policy content".to_string();
            error!("{}", trace!(("{msg}"), source));
            (StatusCode::INTERNAL_SERVER_ERROR, msg)
        })?;

        let content: D::Content = conn
            .get_version_content(version)
            .await
            .map_err(|source| {
                let msg: String = "Failed to get policy content".to_string();
                error!("{}", trace!(("{msg}"), source));
                (StatusCode::INTERNAL_SERVER_ERROR, msg)
            })?
            .ok_or_else(|| (StatusCode::NOT_FOUND, String::new()))?;

        // Serialize the result
        let content = serde_json::to_string(&GetVersionContentResponse { content }).map_err(|source| {
            let msg: String = "Failed to serialize result".to_string();
            error!("{}", trace!(("{msg}"), source));
            (StatusCode::INTERNAL_SERVER_ERROR, msg)
        })?;

        Ok((StatusCode::OK, content))
    }
}
