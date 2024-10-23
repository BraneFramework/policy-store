//  SERVER.rs
//    by Lut99
//
//  Created:
//    23 Oct 2024, 11:37:44
//  Last edited:
//    23 Oct 2024, 11:48:46
//  Auto updated?
//    Yes
//
//  Description:
//!   Implements some abstraction over something waiting for requests and
//!   serving policies.
//

use std::error::Error;
use std::future::Future;

use never_say_never::Never;


/***** LIBRARY *****/
/// Abstracts over the "frontend" of the store; i.e., some API or other interface that listens for
/// requests and interacts with the backend as necessary.
pub trait Server {
    /// The type of errors emitted by this server.
    type Error: Error;


    /// Runs this server.
    ///
    /// This will hijack the current codeflow and keep serving the server until the end of the
    /// universe (or until the server itself quits).
    ///
    /// # Returns
    /// [`Never`].
    ///
    /// # Errors
    /// This function may error if the server failed to listen of if a fatal server errors comes
    /// along as it serves. However, client-side errors should not trigger errors at this level.
    fn serve(self) -> impl Future<Output = Result<Never, Self::Error>>;
}
