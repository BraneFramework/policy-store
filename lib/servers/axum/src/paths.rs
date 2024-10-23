//  PATHS.rs
//    by Lut99
//
//  Created:
//    23 Oct 2024, 11:56:03
//  Last edited:
//    23 Oct 2024, 14:58:33
//  Auto updated?
//    Yes
//
//  Description:
//!   Implements the handlers for the various API paths.
//

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::{Extension, Json};
use specifications::metadata::AttachedMetadata;
use specifications::{AuthResolver, DatabaseConnector};

use crate::server::{AxumServer, Error};


/***** LIBRARIES *****/
impl<A, D> AxumServer<A, D>
where
    A: AuthResolver,
{
    /// Handler for `POST /policies` (i.e., uploading a new policy).
    pub(crate) async fn add_version(
        State(this): State<Arc<Self>>,
        Extension(auth): Extension<A::Context>,
        Json(request): Json<AttachedMetadata>,
    ) -> (StatusCode, String)
    where
        A: AuthResolver,
    {
        todo!()
    }
}
