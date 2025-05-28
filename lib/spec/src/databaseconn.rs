//  DATABASECONN.rs
//    by Lut99
//
//  Created:
//    18 Oct 2024, 17:38:33
//  Last edited:
//    11 Nov 2024, 11:26:25
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines an interface to some backend database that stores policies.
//

use std::collections::HashMap;
use std::error::Error;
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

use crate::metadata::{AttachedMetadata, Metadata, User};


/***** LIBRARY *****/
/// Defines how the policy store connects to the backend database that stores them.
///
/// Note that connectors should assume asynchronous usage of their interface. As such, `self` is
/// only passed by non-mutable reference.
pub trait DatabaseConnector {
    /// The type of things stored in the backend database.
    type Content;
    /// The connection that is created and scoped to a user.
    type Connection<'s>: DatabaseConnection<Content = Self::Content>
    where
        Self: 's;
    /// The type of errors returned by the connector.
    type Error: Error;


    /// Creates a connection to the backend that is contextualized to a particular user.
    ///
    /// # Arguments
    /// - `user`: Some [`User`] on who's behalf actions are taken. It is assumed they are already
    ///   authenticated somehow.
    /// - `body`: Some ("asynchronous") closure that can interact with the created connection for as long as it lives.
    ///
    /// # Errors
    /// This function can error if it failed to create the new connection.
    fn connect<'s>(&'s self, user: &'s User) -> impl Send + Future<Output = Result<Self::Connection<'s>, Self::Error>>;
}

// Pointer-like impls
impl<T: DatabaseConnector> DatabaseConnector for &T {
    type Content = T::Content;
    type Connection<'s>
        = T::Connection<'s>
    where
        Self: 's;
    type Error = T::Error;

    #[inline]
    fn connect<'s>(&'s self, user: &'s User) -> impl Send + Future<Output = Result<Self::Connection<'s>, Self::Error>> {
        <T as DatabaseConnector>::connect(self, user)
    }
}
impl<T: DatabaseConnector> DatabaseConnector for &mut T {
    type Content = T::Content;
    type Connection<'s>
        = T::Connection<'s>
    where
        Self: 's;
    type Error = T::Error;

    #[inline]
    fn connect<'s>(&'s self, user: &'s User) -> impl Send + Future<Output = Result<Self::Connection<'s>, Self::Error>> {
        <T as DatabaseConnector>::connect(self, user)
    }
}
impl<T: DatabaseConnector> DatabaseConnector for Rc<T> {
    type Content = T::Content;
    type Connection<'s>
        = T::Connection<'s>
    where
        T: 's;
    type Error = T::Error;

    #[inline]
    fn connect<'s>(&'s self, user: &'s User) -> impl Send + Future<Output = Result<Self::Connection<'s>, Self::Error>> {
        <T as DatabaseConnector>::connect(self, user)
    }
}
impl<T: DatabaseConnector> DatabaseConnector for Arc<T> {
    type Content = T::Content;
    type Connection<'s>
        = T::Connection<'s>
    where
        T: 's;
    type Error = T::Error;

    #[inline]
    fn connect<'s>(&'s self, user: &'s User) -> impl Send + Future<Output = Result<Self::Connection<'s>, Self::Error>> {
        <T as DatabaseConnector>::connect(self, user)
    }
}



/// Defines how to interact with the backend database once a connection has been made.
pub trait DatabaseConnection {
    /// The type of things stored in the backend database.
    type Content;
    /// The type of errors returned by the connection.
    type Error: Error;


    // Mutations
    /// Adds a new policy to the database.
    ///
    /// # Arguments
    /// - `metadata`: The [`AttachedMetadata`] that describes the context of the request.
    /// - `content`: The [`DatabaseConnector::Content`] that is the body of the policy to store.
    ///
    /// # Returns
    /// A version number that can be used to refer to this policy.
    ///
    /// # Errors
    /// This function may error if it failed to add the version to the backend database.
    fn add_version(&mut self, metadata: AttachedMetadata, content: Self::Content) -> impl Send + Future<Output = Result<u64, Self::Error>>;
    /// Marks one particular version of the policy as active.
    ///
    /// Active policy is the one queried by the reasoner.
    ///
    /// # Arguments
    /// - `version`: The version number of the (already submitted) policy to make active.
    ///
    /// # Errors
    /// This function may error if it failed to set the active policy in the backend database or if
    /// `version` does not exist.
    fn activate(&mut self, version: u64) -> impl Send + Future<Output = Result<(), Self::Error>>;
    /// "Panic button" that replaces the currently active policy with a policy that always denies
    /// all incoming requests.
    ///
    /// # Errors
    /// This function may error if it failed to set the active policy in the backend database.
    fn deactivate(&mut self) -> impl Send + Future<Output = Result<(), Self::Error>>;

    // Read-only
    /// Gets a list of all versions in the database together with their metadata.
    ///
    /// # Returns
    /// A map that enumerates all versions and associates them with that verion's [`Metadata`].
    ///
    /// # Errors
    /// This function may error if it failed to get the policies from the backend database.
    fn get_versions(&mut self) -> impl Send + Future<Output = Result<HashMap<u64, Metadata>, Self::Error>>;
    /// Retrieves the active version from the policy database.
    ///
    /// # Returns
    /// The version number currently active, or [`None`] if none is.
    ///
    /// # Errors
    /// This function may error if it failed to get the policies from the backend database.
    fn get_active_version(&mut self) -> impl Send + Future<Output = Result<Option<u64>, Self::Error>>;
    /// Retrieves the person who activated the policy.
    ///
    /// # Returns
    /// The [`User`] who has set the policy to active, or [`None`] if none is.
    ///
    /// # Errors
    /// This function may error if it failed to get the policies from the backend database.
    fn get_activator(&mut self) -> impl Send + Future<Output = Result<Option<User>, Self::Error>>;
    /// Retrieves a particular policy version's metadata from the database.
    ///
    /// # Arguments
    /// - `version`: The policy version to retrieve.
    ///
    /// # Returns
    /// A [`Metadata`] describing the metadata behind the requested policy, or [`None`] if the given version wasn't found.
    ///
    /// # Errors
    /// This function may error if it failed to retrieve the version from the backend database, or
    /// if that version didn't exist.
    fn get_version_metadata(&mut self, version: u64) -> impl Send + Future<Output = Result<Option<Metadata>, Self::Error>>;
    /// Retrieves a particular policy version from the database.
    ///
    /// # Arguments
    /// - `version`: The policy version to retrieve.
    ///
    /// # Returns
    /// A [`DatabaseConnection::Content`] that represents the requested policy, or [`None`] if the given version wasn't found.
    ///
    /// # Errors
    /// This function may error if it failed to retrieve the version from the backend database, or
    /// if that version didn't exist.
    fn get_version_content(&mut self, version: u64) -> impl Send + Future<Output = Result<Option<Self::Content>, Self::Error>>;
}


// Pointer-like impls
impl<T: DatabaseConnection> DatabaseConnection for &mut T {
    type Content = T::Content;
    type Error = T::Error;

    #[inline]
    fn add_version(&mut self, metadata: AttachedMetadata, content: Self::Content) -> impl Send + Future<Output = Result<u64, Self::Error>> {
        <T as DatabaseConnection>::add_version(self, metadata, content)
    }
    #[inline]
    fn activate(&mut self, version: u64) -> impl Send + Future<Output = Result<(), Self::Error>> {
        <T as DatabaseConnection>::activate(self, version)
    }
    #[inline]
    fn deactivate(&mut self) -> impl Send + Future<Output = Result<(), Self::Error>> { <T as DatabaseConnection>::deactivate(self) }

    #[inline]
    fn get_versions(&mut self) -> impl Send + Future<Output = Result<HashMap<u64, Metadata>, Self::Error>> {
        <T as DatabaseConnection>::get_versions(self)
    }
    #[inline]
    fn get_active_version(&mut self) -> impl Send + Future<Output = Result<Option<u64>, Self::Error>> {
        <T as DatabaseConnection>::get_active_version(self)
    }
    #[inline]
    fn get_activator(&mut self) -> impl Send + Future<Output = Result<Option<User>, Self::Error>> { <T as DatabaseConnection>::get_activator(self) }
    #[inline]
    fn get_version_metadata(&mut self, version: u64) -> impl Send + Future<Output = Result<Option<Metadata>, Self::Error>> {
        <T as DatabaseConnection>::get_version_metadata(self, version)
    }
    #[inline]
    fn get_version_content(&mut self, version: u64) -> impl Send + Future<Output = Result<Option<Self::Content>, Self::Error>> {
        <T as DatabaseConnection>::get_version_content(self, version)
    }
}
