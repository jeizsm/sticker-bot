mod state_machine;
mod db;

use bincode::deserialize;
use hyper_rustls::HttpsConnector;
use hyper::Client;
use serde::de::DeserializeOwned;
pub(crate) use self::state_machine::{Event, State};
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
    pub(crate) telegram_token: String,
    pub(crate) user_id: i64,
    pub(crate) bot_name: String,
    pub(crate) sled_db_dir: String,
}

pub(crate) struct MyOption<T>(pub Option<T>);

impl<V> From<Option<Vec<u8>>> for MyOption<V>
where V: DeserializeOwned {
    fn from(vec: Option<Vec<u8>>) -> MyOption<V> {
        MyOption(vec.map(|a| deserialize(&a).unwrap()))
    }
}

impl<V> Into<Option<V>> for MyOption<V> {
    fn into(self) -> Option<V> {
        self.0
    }
}
