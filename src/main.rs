extern crate futures;
extern crate telebot;
extern crate tokio_core;
#[macro_use]
extern crate failure;
extern crate hyper;
extern crate hyper_rustls;
extern crate magick_rust;
#[macro_use]
extern crate lazy_static;
extern crate bincode;
extern crate serde;
extern crate sled;
#[macro_use]
extern crate serde_derive;
extern crate envy;
#[macro_use]
extern crate log;
extern crate byteorder;
extern crate env_logger;

use futures::{Future, Stream};
use hyper::Client;
use hyper_rustls::HttpsConnector;
use telebot::RcBot;
use tokio_core::reactor::Core;

mod helpers;
mod types;
mod updates;

fn main() {
    magick_rust::magick_wand_genesis();
    env_logger::init();
    let mut core = Core::new().unwrap();
    let handle = core.handle();
    let bot = RcBot::new(handle.clone(), &helpers::CONFIG.telegram_token).update_interval(200);

    let client = Client::configure().connector(HttpsConnector::new(4, &handle)).build(&handle);

    let stream = bot.get_stream().for_each(|(bot, msg)| {
        let client = client.clone();
        debug!("{:?}", msg);
        let future = updates::process(&bot, client, msg);
        handle.spawn(future.map_err(|a| error!("{:?}", a)));
        Ok(())
    });

    core.run(stream).unwrap();
}
