//  LIB.rs
//    by Lut99
//
//  Created:
//    06 Dec 2024, 17:59:58
//  Last edited:
//    06 Dec 2024, 18:32:22
//  Auto updated?
//    Yes
//
//  Description:
//!   Pseudo-server that defines the API endpoint locations, methods and
//!   request/response bodies for the `axum-server`.
//

use core::str;
use std::borrow::Cow;
use std::collections::HashMap;
#[cfg(feature = "axum")]
use std::convert::Infallible;

#[cfg(feature = "axum")]
use axum::handler::Handler;
#[cfg(feature = "axum")]
use axum::routing::MethodRouter;
#[cfg(feature = "axum")]
use axum::routing::method_routing::{delete, get, post, put};
use http::Method;
use itertools::Itertools as _;
use serde::{Deserialize, Serialize};
use specifications::metadata::{AttachedMetadata, Metadata, User};


/***** AUXILLARY *****/
/// Defines where to find an endpoint in the API.
pub struct EndpointPath {
    /// The method to apply.
    pub method: Method,
    /// The path where to find it.
    ///
    /// You can use path arguments to allow clients to instantiate them. For example, the path
    /// ```plan
    /// /v2/policy/{version}
    /// ```
    /// will cause the user to have to given an argument in [`EndpointPath::instantiated_path()`]. Note
    /// that path arguments are defined as path segments beginning with a colon.
    pub path:   &'static str,
}
impl EndpointPath {
    /// Runs the appropriate [`axum`] function on this endpointpath.
    ///
    /// # Arguments
    /// - `handler`: Some handler to call when the path + method is matched.
    ///
    /// # Returns
    /// A new [`MethodRouter`] that encodes to axum when to call the given `handler`.
    #[cfg(feature = "axum")]
    pub fn handler<H, T, S>(&self, handler: H) -> MethodRouter<S, Infallible>
    where
        H: Handler<T, S>,
        T: 'static,
        S: Clone + Send + Sync + 'static,
    {
        match self.method {
            Method::GET => get(handler),
            Method::POST => post(handler),
            Method::PUT => put(handler),
            Method::DELETE => delete(handler),
            _ => unimplemented!(),
        }
    }

    /// Returns a string that find the path where this route may be found.
    ///
    /// Note that, if there are any parameters in it, these are instantiated by the given list of
    /// values. Therefore, this function tends to be used when using the API.
    ///
    /// # Returns
    /// A [`Cow<'static, str>`] that encodes the location of this endpoint.
    ///
    /// # Panics
    /// This function panics if:
    /// - any of the input arguments has a '/' in it; or
    /// - the number of arguments given does not match the number of arguments in the path.
    #[inline]
    #[track_caller]
    pub fn instantiated_path<'a>(&self, args: impl IntoIterator<Item = &'a str>) -> Cow<'static, str> {
        let mut args = args.into_iter();
        let mut replace_count: usize = 0;
        let path = self
            .path
            .split("/")
            .map(|component| {
                if component.starts_with("{") && component.ends_with("}") {
                    let res = args.next().unwrap_or_else(|| panic!("Not enough arguments given for path {:?} (got {replace_count})", self.path));
                    replace_count += 1;
                    res
                } else {
                    component
                }
            })
            .join("/");

        // Assert none are left
        if args.next().is_some() {
            panic!("Too many arguments given for path {:?} which has no arguments", self.path);
        }

        if replace_count == 0 { Cow::Borrowed(self.path) } else { Cow::Owned(path) }
    }
}





/***** LIBRARY *****/
/// Path of the endpoint to add a new policy version.
pub const ADD_VERSION_PATH: EndpointPath = EndpointPath { method: Method::POST, path: "/v2/policies" };

/// What to send in the body of a request when [adding](axum-server::server::AxumServer::add_version())
/// a new version.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AddVersionRequest<C> {
    /// The metadata for this policy.
    pub metadata: AttachedMetadata,
    /// The contents of the policy itself.
    pub contents: C,
}

/// Replied when [adding](axum-server::server::AxumServer::add_version()) a new version.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct AddVersionResponse {
    /// The newly assigned ID of the version.
    pub version: u64,
}



/// Path of the endpoint to activate an already submitted policy version.
pub const ACTIVATE_PATH: EndpointPath = EndpointPath { method: Method::PUT, path: "/v2/policies/active" };

/// What to send in the body of a request when [activating](axum-server::server::AxumServer::activate())
/// a version.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct ActivateRequest {
    /// The version to activate.
    pub version: u64,
}



/// Path of the endpoint to deactivate any active policy version.
pub const DEACTIVATE_PATH: EndpointPath = EndpointPath { method: Method::DELETE, path: "/v2/policies/active" };



/// Path of the endpoint to retrieve the metadata of all submitted policy versions.
pub const GET_VERSIONS_PATH: EndpointPath = EndpointPath { method: Method::GET, path: "/v2/policies" };

/// Replied when [listing](axum-server::server::AxumServer::get_versions()) all versions.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GetVersionsResponse {
    /// The versions in the reasoner.
    pub versions: HashMap<u64, Metadata>,
}



/// Path of the endpoint to retrieve the currently active policy version, if any.
pub const GET_ACTIVE_VERSION_PATH: EndpointPath = EndpointPath { method: Method::GET, path: "/v2/policies/active" };

/// Replied when [retrieving the active policy](axum-server::server::AxumServer::get_active_version()).
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct GetActiveVersionResponse {
    /// The version of the active policy, if any.
    pub version: Option<u64>,
}



/// Path of the endpoint to retrieve the person who activated the currently active policy version, if any.
pub const GET_ACTIVATOR_VERSION_PATH: EndpointPath = EndpointPath { method: Method::GET, path: "/v2/policies/active/activator" };

/// Replied when [retrieving the activator](axum-server::server::AxumServer::get_activator()).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GetActivatorResponse {
    /// The person who activated the active policy, if any.
    pub user: Option<User>,
}



/// Path of the endpoint to retrieve the metadata of a particular policy version.
pub const GET_VERSION_METADATA_PATH: EndpointPath = EndpointPath { method: Method::GET, path: "/v2/policies/{version}" };

/// Replied when [retrieving metadata](axum-server::server::AxumServer::get_version_metadata()).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GetVersionMetadataResponse {
    /// The metadata of the requested policy.
    pub metadata: Metadata,
}



/// Path of the endpoint to retrieve the contents of a particular policy version.
pub const GET_VERSION_CONTENT_PATH: EndpointPath = EndpointPath { method: Method::GET, path: "/v2/policies/{version}/content" };

/// Replied when [retrieving content](axum-server::server::AxumServer::get_version_content()).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GetVersionContentResponse<C> {
    /// The content of the requested policy.
    pub content: C,
}
