//  API.rs
//    by Lut99
//
//  Created:
//    23 Oct 2024, 10:28:29
//  Last edited:
//    23 Oct 2024, 16:22:20
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines the API itself.
//

use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::connect_info::IntoMakeServiceWithConnectInfo;
use axum::routing::post;
use axum::Router;
use never_say_never::Never;
use specifications::{AuthResolver, DatabaseConnector, Server};
use thiserror::Error;
use tracing::{debug, info, span, Level};

use crate::paths;


/***** ERRORS *****/
/// Defines errors emitted by the [`AxumServer`].
#[derive(Debug, Error)]
pub enum Error {}





/***** LIBRARY *****/
/// Defines the policy store compliant [`axum`] [`Server`].
pub struct AxumServer<A, D> {
    /// The address on which to bind the server.
    pub(crate) addr: SocketAddr,
    /// The auth resolver for resolving auth.
    pub(crate) auth: A,
    /// The database connector for connecting to databases.
    pub(crate) data: D,
}
impl<A, D> AxumServer<A, D> {
    /// Constructor for the AxumServer.
    ///
    /// # Arguments
    /// - `addr`: The address on which to listen once [`serve()`](AxumServer::serve())ing.
    /// - `auth`: The [`AuthResolver`] used to authorize incoming requsts.
    /// - `data`: The [`DatabaseConnector`] used to interact with the backend database.
    ///
    /// # Returns
    /// A new AxumServer, ready to serve its opponents.
    #[inline]
    pub fn new(addr: impl Into<SocketAddr>, auth: A, data: D) -> Self { Self { addr: addr.into(), auth, data } }
}
impl<A, D> Server for AxumServer<A, D>
where
    A: 'static + Send + Sync + AuthResolver,
    A::Context: 'static + Send + Sync + Clone,
    D: 'static + Send + Sync + DatabaseConnector,
{
    type Error = Error;

    fn serve(self) -> impl Future<Output = Result<Never, Self::Error>> {
        let this: Arc<Self> = Arc::new(self);
        async move {
            let mut span = span!(Level::INFO, "AxumServer::serve", state = "starting");

            // First, define the axum paths
            debug!("Building axum paths...");
            let add_version: Router = Router::new()
                .route("/policies", post(Self::add_version))
                .layer(axum::middleware::from_fn_with_state(this.clone(), Self::check))
                .with_state(this);
            let router: IntoMakeServiceWithConnectInfo<Router, SocketAddr> =
                Router::new().nest("/", add_version).into_make_service_with_connect_info();

            // DOne
            todo!()
        }
    }
}
