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

use hyper::Client;
use telebot::RcBot;
use futures::{Future, Stream};
use tokio_core::reactor::Core;
use magick_rust::magick_wand_genesis;
use hyper_rustls::HttpsConnector;

mod updates;
mod types;

fn main() {
    magick_wand_genesis();
    let mut core = Core::new().unwrap();
    let handle = core.handle();
    let bot = RcBot::new(handle.clone(), &updates::TELEGRAM_TOKEN).update_interval(200);

    let client = Client::configure()
        .connector(HttpsConnector::new(4, &handle))
        .build(&handle);

    let stream = bot.get_stream().for_each(|(bot, msg)| {
        println!("Received");
        let client = client.clone();
        let future = updates::process(&bot, client, msg);
        handle.spawn(future.map_err(|a| println!("{:?}", a)));
        Ok(())
    });

    core.run(stream).unwrap();
}
