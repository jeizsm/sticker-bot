use std::collections::HashMap;
use std::env;
use std::sync::Mutex;

use failure::{err_msg, Error, Fail};
use futures::Stream;
use futures::future::{ok, Either, Future};
use hyper::Client;
use hyper_rustls::HttpsConnector;
use magick_rust::MagickWand;
use telebot::RcBot;
use telebot::functions::{FunctionDeleteStickerFromSet, FunctionGetFile, FunctionMessage};
use telebot::objects::{File, Message, Update};
use types::{nullify, ErrorKind, Event, State};

lazy_static! {
    static ref HASHMAP: Mutex<HashMap<i64, State>> = { Mutex::new(HashMap::new()) };
    static ref USER_ID: i64 = { env::var("USER_ID").unwrap().parse().unwrap() };
    pub(super) static ref TELEGRAM_TOKEN: String = { env::var("TELEGRAM_TOKEN").unwrap() };
    static ref BOT_NAME: String = { env::var("BOT_NAME").unwrap() };
}

pub(super) fn process(
    bot: &RcBot,
    client: Client<HttpsConnector>,
    update: Update,
) -> impl Future<Item = (), Error = Error> {
    if let Some(message) = update.message {
        Either::A(message_process(bot, client, message))
    } else {
        Either::B(ok(()))
    }
}

fn message_process(
    bot: &RcBot,
    client: Client<HttpsConnector>,
    message: Message,
) -> impl Future<Item = (), Error = Error> {
    let (user_id, user_name) = match message.from.as_ref() {
        Some(user) => {
            if user.id == *USER_ID {
                (user.id, user.username.clone().unwrap())
            } else {
                return Either::A(Either::A(ok(())));
            }
        }
        None => return Either::A(Either::A(ok(()))),
    };
    let chat_id = message.chat.id;
    let send_message =
        move |(bot, file): (RcBot, File)| send_message((bot, file), user_id, chat_id, &client);
    match message {
        Message {
            text: Some(text), ..
        } => {
            let future = text_message(bot, text, user_id, chat_id, &user_name);
            Either::B(Either::A(future))
        }
        Message {
            photo: Some(photos),
            ..
        } => {
            let photo = photos.last().unwrap();
            let id = photo.file_id.clone();
            let future = bot.get_file(id).send().and_then(send_message);
            Either::B(Either::B(future))
        }

        Message {
            document: Some(document),
            ..
        } => {
            let id = document.file_id.clone();
            let future = bot.get_file(id).send().and_then(send_message);
            Either::B(Either::B(future))
        }

        Message {
            sticker: Some(sticker),
            ..
        } => {
            let set_name = format!("{}_by_{}", user_name, *BOT_NAME);
            let id = sticker.file_id.clone();
            let sticker_set_name = sticker.set_name.unwrap();
            if set_name == sticker_set_name {
                let future = bot.delete_sticker_from_set(sticker.file_id)
                    .send()
                    .and_then(move |(bot, _)| {
                        bot.message(chat_id, "sticker deleted".to_string())
                            .send()
                            .map(nullify)
                    });
                Either::A(Either::B(future))
            } else {
                let future = bot.get_file(id).send().and_then(send_message);
                Either::B(Either::B(future))
            }
        }
        _ => Either::A(Either::A(ok(()))),
    }
}

fn send_message(
    (bot, file): (RcBot, File),
    user_id: i64,
    chat_id: i64,
    client: &Client<HttpsConnector>,
) -> impl Future<Item = (), Error = Error> {
    let url = format!(
        "https://api.telegram.org/file/bot{}/{}",
        *TELEGRAM_TOKEN,
        file.file_path.unwrap()
    );
    client
        .get(url.parse().unwrap())
        .and_then(|res| res.body().concat2().from_err())
        .map_err(|e| Error::from(e.context(ErrorKind::Hyper)))
        .and_then(|ref body| {
            let wand = MagickWand::new();
            wand.read_image_blob(&body.to_vec()).map_err(err_msg)?;
            wand.fit(512, 512);
            wand.write_image_blob("png").map_err(err_msg)
        })
        .and_then(move |image| {
            let state = HASHMAP.lock().unwrap().remove(&user_id).unwrap();
            let state = state.next(Event::AddSticker { file: image });
            let future = state.run(&bot, chat_id);
            HASHMAP.lock().unwrap().insert(user_id, state);
            future
        })
}

fn text_message(
    bot: &RcBot,
    text: String,
    user_id: i64,
    chat_id: i64,
    user_name: &str,
) -> impl Future<Item = (), Error = Error> {
    if text.starts_with("/new_pack") || text.starts_with("/add_to_pack") {
        let state = State::new();
        let future = state.run(bot, chat_id);
        HASHMAP.lock().unwrap().insert(user_id, state);
        Either::A(Either::A(future))
    } else if text.starts_with("/publish") {
        let mut hashmap = HASHMAP.lock().unwrap();
        let state = hashmap.remove(&user_id);
        match state {
            Some(state @ State::Emojis { .. }) | Some(state @ State::Title { .. }) => {
                let name = format!("{}_by_{}", user_name, *BOT_NAME);
                let state = state.next(Event::AddName { name, user_id });
                Either::A(Either::B(state.publish(bot, chat_id)))
            }
            Some(state) => {
                hashmap.insert(user_id, state);
                let future = bot.message(chat_id, "cannot publish yet".to_string())
                    .send()
                    .map(nullify);
                Either::B(future)
            }
            _ => {
                let future = bot.message(chat_id, "cannot publish yet".to_string())
                    .send()
                    .map(nullify);
                Either::B(future)
            }
        }
    } else {
        let mut hashmap = HASHMAP.lock().unwrap();
        let state = hashmap.remove(&user_id);
        match state {
            Some(state) => {
                let event = match state {
                    State::Sticker { .. } => Event::AddEmojis { emojis: text },
                    State::Emojis { .. } => Event::AddTitle { title: text },
                    _ => Event::DoNothing,
                };
                let state = state.next(event);
                let future = state.run(bot, chat_id);
                hashmap.insert(user_id, state);
                Either::A(Either::A(future))
            }
            None => {
                let future = bot.message(chat_id, text).send().map(nullify);
                Either::B(future)
            }
        }
    }
}
