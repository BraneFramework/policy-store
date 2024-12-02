//  API.rs
//    by Lut99
//
//  Created:
//    23 Oct 2024, 10:28:29
//  Last edited:
//    02 Dec 2024, 15:14:11
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
use axum::routing::{delete, get, post, put};
use axum::Router;
use error_trace::trace;
use hyper::body::Incoming;
use hyper::Request;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder as HyperBuilder;
use serde::de::DeserializeOwned;
use serde::Serialize;
use specifications::{AuthResolver, DatabaseConnector, Server};
use thiserror::Error;
use tokio::net::{TcpListener, TcpStream};
use tower_service::Service as _;
use tracing::field::Empty;
use tracing::{debug, error, info, span, Level};


/***** ERRORS *****/
/// Defines errors emitted by the [`AxumServer`].
#[derive(Debug, Error)]
pub enum Error {
    /// Failed to bind on the given address.
    #[error("Failed to bind server on address '{addr}'")]
    ListenerBind {
        addr: SocketAddr,
        #[source]
        err:  std::io::Error,
    },
}





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
impl<A, D> AxumServer<A, D>
where
    A: 'static + Send + Sync + AuthResolver,
    A::Context: 'static + Send + Sync + Clone,
    A::ClientError: 'static,
    A::ServerError: 'static,
    D: 'static + Send + Sync + DatabaseConnector,
    D::Content: Send + DeserializeOwned + Serialize,
    for<'s> D::Connection<'s>: Send,
{
    /// Builds an [`axum`] [`Router`] that encodes the paths of this server.
    ///
    /// # Arguments
    /// - `this`: Is like `self`, but then wrapped in an [`Arc`].
    ///
    /// # Returns
    /// A [`Router`] that can be extended with additional paths, if preferred.
    pub fn routes(this: Arc<Self>) -> Router<()> {
        let _span = span!(Level::INFO, "AxumServer::routes");

        // First, define the axum paths
        debug!("Building axum paths...");
        let add_version: Router = Router::new()
            .route("/policies", post(Self::add_version))
            .layer(axum::middleware::from_fn_with_state(this.clone(), Self::check))
            .with_state(this.clone());
        let activate: Router = Router::new()
            .route("/policies/active", put(Self::activate))
            .layer(axum::middleware::from_fn_with_state(this.clone(), Self::check))
            .with_state(this.clone());
        let deactivate: Router = Router::new()
            .route("/policies/active", delete(Self::deactivate))
            .layer(axum::middleware::from_fn_with_state(this.clone(), Self::check))
            .with_state(this.clone());
        let get_versions: Router = Router::new()
            .route("/policies", get(Self::get_versions))
            .layer(axum::middleware::from_fn_with_state(this.clone(), Self::check))
            .with_state(this.clone());
        let get_active_version: Router = Router::new()
            .route("/policies/active", get(Self::get_active_version))
            .layer(axum::middleware::from_fn_with_state(this.clone(), Self::check))
            .with_state(this.clone());
        let get_activator: Router = Router::new()
            .route("/policies/active/activator", get(Self::get_activator))
            .layer(axum::middleware::from_fn_with_state(this.clone(), Self::check))
            .with_state(this.clone());
        let get_version_metadata: Router = Router::new()
            .route("/policies/:version", get(Self::get_version_metadata))
            .layer(axum::middleware::from_fn_with_state(this.clone(), Self::check))
            .with_state(this.clone());
        let get_version_content: Router = Router::new()
            .route("/policies/:version/content", get(Self::get_version_content))
            .layer(axum::middleware::from_fn_with_state(this.clone(), Self::check))
            .with_state(this.clone());
        Router::<()>::new()
            .nest("/v2/", add_version)
            .nest("/v2/", activate)
            .nest("/v2/", deactivate)
            .nest("/v2/", get_versions)
            .nest("/v2/", get_active_version)
            .nest("/v2/", get_activator)
            .nest("/v2/", get_version_metadata)
            .nest("/v2/", get_version_content)
    }
}
impl<A, D> AxumServer<A, D> {
    /// Runs the given [`axum`] [`Router`].
    ///
    /// # Arguments
    /// - `this`: Is like `self`, but then wrapped in an [`Arc`].
    /// - `router`: The [`Router`] to run.
    ///
    /// # Returns
    /// This function does not return for as long as the server runs.
    ///
    /// # Errors
    /// This function may fail if it failed to bind the server at the internal address.
    pub fn serve_router(this: Arc<Self>, router: Router<()>) -> impl Future<Output = Result<(), Error>> {
        async move {
            let span = span!(Level::INFO, "AxumServer::serve_router", state = "starting", client = Empty);
            let router: IntoMakeServiceWithConnectInfo<Router, SocketAddr> = Router::<()>::into_make_service_with_connect_info(router);

            // Bind the TCP Listener
            debug!("Binding server on '{}'...", this.addr);
            let listener: TcpListener = match TcpListener::bind(this.addr).await {
                Ok(listener) => listener,
                Err(err) => return Err(Error::ListenerBind { addr: this.addr, err }),
            };

            // Accept new connections!
            info!("Initialization OK, awaiting connections...");
            span.record("state", "running");
            loop {
                // Accept a new connection
                let (socket, remote_addr): (TcpStream, SocketAddr) = match listener.accept().await {
                    Ok(res) => res,
                    Err(err) => {
                        error!("{}", trace!(("Failed to accept incoming connection"), err));
                        continue;
                    },
                };
                span.record("client", remote_addr.to_string());

                // Move the rest to a separate task
                let router: IntoMakeServiceWithConnectInfo<_, _> = router.clone();
                tokio::spawn(async move {
                    debug!("Handling incoming connection from '{remote_addr}'");

                    // Build  the service
                    let service = hyper::service::service_fn(|request: Request<Incoming>| {
                        // Sadly, we must `move` again because this service could be called multiple times (at least according to the typesystem)
                        let mut router = router.clone();
                        async move {
                            // SAFETY: We can call `unwrap()` because the call returns an infallible.
                            router.call(remote_addr).await.unwrap().call(request).await
                        }
                    });

                    // Create a service that handles this for us
                    let socket: TokioIo<_> = TokioIo::new(socket);
                    if let Err(err) = HyperBuilder::new(TokioExecutor::new()).serve_connection_with_upgrades(socket, service).await {
                        error!("{}", trace!(("Failed to serve incoming connection"), *err));
                    }
                });
            }
        }
    }
}
impl<A, D> Server for AxumServer<A, D>
where
    A: 'static + Send + Sync + AuthResolver,
    A::Context: 'static + Send + Sync + Clone,
    A::ClientError: 'static,
    A::ServerError: 'static,
    D: 'static + Send + Sync + DatabaseConnector,
    D::Content: Send + DeserializeOwned + Serialize,
    for<'s> D::Connection<'s>: Send,
{
    type Error = Error;

    fn serve(self) -> impl Future<Output = Result<(), Self::Error>> {
        let this: Arc<Self> = Arc::new(self);
        async move {
            let _span = span!(Level::INFO, "AxumServer::serve");

            // Simply depend on the two halves of the equation
            let router: Router<()> = Self::routes(this.clone());
            Self::serve_router(this, router).await
        }
    }
}
