//  PATHS.rs
//    by Lut99
//
//  Created:
//    23 Oct 2024, 11:56:03
//  Last edited:
//    24 Oct 2024, 13:37:07
//  Auto updated?
//    Yes
//
//  Description:
//!   Implements the handlers for the various API paths.
//

use std::future::Future;
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::{Extension, Json};
use error_trace::trace;
use specifications::databaseconn::DatabaseConnection;
use specifications::metadata::User;
use specifications::DatabaseConnector;
use tracing::{error, span, Level};

use crate::server::AxumServer;
use crate::spec::AddVersionRequest;


/***** LIBRARIES *****/
impl<A, D> AxumServer<A, D>
where
    A: 'static + Send + Sync,
    D: 'static + Send + Sync + DatabaseConnector,
    D::Content: Send,
    for<'s> D::Connection<'s>: Send,
{
    /// Handler for `POST /policies` (i.e., uploading a new policy).
    pub(crate) fn add_version(
        State(this): State<Arc<Self>>,
        Extension(auth): Extension<User>,
        Json(request): Json<AddVersionRequest<D::Content>>,
    ) -> impl 'static + Send + Future<Output = (StatusCode, String)> {
        async move {
            let _span = span!(Level::INFO, "AxumServer::add_version", user = auth.id);

            // Just try to send it to the DB
            let mut conn: D::Connection<'_> = match this.data.connect(&auth).await {
                Ok(conn) => conn,
                Err(err) => {
                    let msg: String = format!("Failed to add policy {}", request.metadata.name);
                    error!("{}", trace!(("{msg}"), err));
                    return (StatusCode::INTERNAL_SERVER_ERROR, msg);
                },
            };
            let name: String = request.metadata.name.clone();
            let version: u64 = match conn.add_version(request.metadata, request.contents).await {
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
