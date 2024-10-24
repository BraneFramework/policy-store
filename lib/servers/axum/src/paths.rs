//  PATHS.rs
//    by Lut99
//
//  Created:
//    23 Oct 2024, 11:56:03
//  Last edited:
//    24 Oct 2024, 14:43:20
//  Auto updated?
//    Yes
//
//  Description:
//!   Implements the handlers for the various API paths.
//

use std::future::Future;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::Extension;
use error_trace::trace;
use futures::StreamExt;
use serde::de::DeserializeOwned;
use specifications::databaseconn::DatabaseConnection;
use specifications::metadata::User;
use specifications::DatabaseConnector;
use tracing::{error, info, span, Level};

use crate::server::AxumServer;
use crate::spec::AddVersionRequest;


/***** LIBRARIES *****/
impl<A, D> AxumServer<A, D>
where
    A: 'static + Send + Sync,
    D: 'static + Send + Sync + DatabaseConnector,
    D::Content: Send + DeserializeOwned,
    for<'s> D::Connection<'s>: Send,
{
    /// Handler for `POST /policies` (i.e., uploading a new policy).
    pub(crate) fn add_version(
        State(this): State<Arc<Self>>,
        Extension(auth): Extension<User>,
        request: Request,
    ) -> impl 'static + Send + Future<Output = (StatusCode, String)> {
        async move {
            let _span = span!(Level::INFO, "AxumServer::add_version", user = auth.id);

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
                        return (StatusCode::INTERNAL_SERVER_ERROR, msg.into());
                    },
                };

                // Append it
                req.extend(next);
            }

            // Deserialize the request contents
            let req: AddVersionRequest<D::Content> = match serde_json::from_slice(&req) {
                Ok(req) => req,
                Err(err) => {
                    let error: String = format!(
                        "{}Raw body:\n{}\n{}\n{}\n",
                        trace!(("Failed to deserialize request body"), err),
                        (0..80).map(|_| '-').collect::<String>(),
                        String::from_utf8_lossy(&req),
                        (0..80).map(|_| '-').collect::<String>()
                    );
                    info!("{error}");
                    return (StatusCode::BAD_REQUEST, error);
                },
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
            (StatusCode::OK, version.to_string())
        }
    }
}
