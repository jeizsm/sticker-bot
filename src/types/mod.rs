mod state_machine;
mod db;

use hyper_rustls::HttpsConnector;
use hyper::Client;
use hyper::Error as HyperError;
use sled::Error as SledError;
pub(crate) use self::state_machine::{Event, State, nullify};
pub(crate) use self::db::TypedDB;

pub(crate) type HttpsClient = Client<HttpsConnector>;

#[derive(Debug, Fail)]
pub(crate) enum ErrorKind {
    // indicates some failure in Hyper, missing network connection, etc.
    #[fail(display = "There was an error fetching the content")]
    Hyper,
    #[fail(display = "There was an error in Sled")]
    Sled,
}

#[derive(Deserialize)]
pub(crate) struct Config {
    pub telegram_token: String,
    pub user_id: i64,
    pub bot_name: String,
}
