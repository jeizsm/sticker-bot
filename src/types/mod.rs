mod state_machine;
mod db;

use hyper_rustls::HttpsConnector;
use hyper::Client;
use hyper::Error as HyperError;
use sled::Error as SledError;
pub(crate) use self::state_machine::{Event, State, nullify};

pub(crate) type HttpsClient = Client<HttpsConnector>;

#[derive(Debug, Fail)]
pub(crate) enum ErrorKind {
    // indicates some failure in Hyper, missing network connection, etc.
    #[fail(display = "There was an error fetching the content")]
    Hyper,
}
