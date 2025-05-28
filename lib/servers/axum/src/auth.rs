//  AUTH.rs
//    by Lut99
//
//  Created:
//    23 Oct 2024, 11:58:43
//  Last edited:
//    02 Dec 2024, 15:17:13
//  Auto updated?
//    Yes
//
//  Description:
//!   Implements the server's authorization middleware.
//

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use error_trace::ErrorTrace as _;
use specifications::AuthResolver;
use specifications::authresolver::HttpError;
use thiserror::Error;
use tracing::{Level, error, info, span};

use crate::server::AxumServer;


/***** ERRORS *****/
/// Simple wrapper for erroring and freezing the result.
#[derive(Debug, Error)]
enum Error<E> {
    #[error("Failed to authorize incoming request")]
    AuthorizeFailed {
        #[source]
        err: E,
    },
}
impl<E: 'static + HttpError> HttpError for Error<E> {
    #[inline]
    fn status_code(&self) -> StatusCode {
        match self {
            Self::AuthorizeFailed { err } => err.status_code(),
        }
    }
}





/***** LIBRARY *****/
impl<A, D> AxumServer<A, D>
where
    A: AuthResolver,
    A::Context: 'static + Send + Sync + Clone,
    A::ClientError: 'static,
    A::ServerError: 'static,
{
    pub async fn check(State(context): State<Arc<Self>>, ConnectInfo(client): ConnectInfo<SocketAddr>, mut request: Request, next: Next) -> Response {
        let _span = span!(Level::INFO, "AxumServer::check", client = client.to_string());

        // Do the auth thingy
        let user: A::Context = match context.auth.authorize(request.headers()).await {
            Ok(Ok(user)) => user,
            Ok(Err(err)) => {
                let err = Error::AuthorizeFailed { err };
                info!("{}", err.trace());
                let mut res =
                    Response::new(Body::from(serde_json::to_string(&err.freeze()).unwrap_or_else(|err| panic!("Failed to serialize Trace: {err}"))));
                *res.status_mut() = err.status_code();
                return res;
            },
            Err(err) => {
                let err = Error::AuthorizeFailed { err };
                error!("{}", err.trace());
                let mut res = Response::new(Body::from(err.to_string()));
                *res.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                return res;
            },
        };

        // If we found a context, then inject it in the request as an extension; then continue
        request.extensions_mut().insert(user);
        next.run(request).await
    }
}
