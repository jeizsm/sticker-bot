use std::collections::HashMap;
use std::env;
use std::sync::Mutex;

use failure::{err_msg, Error, Fail};
use futures::future::{ok, Either, Future};
use futures::Stream;
use magick_rust::MagickWand;
use telebot::functions::{FunctionDeleteStickerFromSet, FunctionGetFile, FunctionMessage};
use telebot::objects::{File, Message, Update};
use telebot::RcBot;
use types::{nullify, ErrorKind, Event, HttpsClient, State};

lazy_static! {
    static ref HASHMAP: Mutex<HashMap<i64, State>> = { Mutex::new(HashMap::new()) };
    static ref USER_ID: i64 = { env::var("USER_ID").unwrap().parse().unwrap() };
    pub(super) static ref TELEGRAM_TOKEN: String = { env::var("TELEGRAM_TOKEN").unwrap() };
    static ref BOT_NAME: String = { env::var("BOT_NAME").unwrap() };
}

pub(super) fn process(bot: &RcBot, client: HttpsClient, update: Update) -> impl Future<Item = (), Error = Error> {
    if let Some(message) = update.message {
        Either::A(message_process(bot, client, message))
    } else {
        Either::B(ok(()))
    }
}

fn message_process(bot: &RcBot, client: HttpsClient, message: Message) -> impl Future<Item = (), Error = Error> {
    let user_id = match message.from.as_ref() {
        Some(user) => {
            if user.id == *USER_ID {
                user.id
            } else {
                return Either::A(Either::A(ok(())));
            }
        }
        None => return Either::A(Either::A(ok(()))),
    };
    let chat_id = message.chat.id;
    let send_message = move |(bot, file): (RcBot, File)| send_message((bot, file), user_id, chat_id, &client);
    let get_file_and_send_message = |id: String| bot.get_file(id).send().and_then(send_message);
    match message {
        Message { text: Some(text), .. } => {
            let future = text_message(bot, text, user_id, chat_id);
            Either::B(Either::A(future))
        }
        Message { photo: Some(photos), .. } => {
            let photo = photos.last().unwrap();
            let future = get_file_and_send_message(photo.file_id.clone());
            Either::B(Either::B(future))
        }

        Message {
            document: Some(document), ..
        } => {
            if document.mime_type.map_or(false, |mime| mime.starts_with("image/")) {
                let future = get_file_and_send_message(document.file_id);
                Either::B(Either::B(future))
            } else {
                Either::A(Either::A(ok(())))
            }
        }

        Message {
            sticker: Some(sticker), ..
        } => {
            let id = sticker.file_id;
            let set_from_bot = sticker.set_name.map_or(false, |a| a.ends_with(&*BOT_NAME));
            if set_from_bot {
                let future = bot.delete_sticker_from_set(id)
                    .send()
                    .and_then(move |(bot, _)| bot.message(chat_id, "sticker deleted".to_string()).send().map(nullify));
                Either::A(Either::B(future))
            } else {
                let future = get_file_and_send_message(id);
                Either::B(Either::B(future))
            }
        }
        _ => Either::A(Either::A(ok(()))),
    }
}

fn send_message((bot, file): (RcBot, File), user_id: i64, chat_id: i64, client: &HttpsClient) -> impl Future<Item = (), Error = Error> {
    let url = format!("https://api.telegram.org/file/bot{}/{}", *TELEGRAM_TOKEN, file.file_path.unwrap());
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

fn text_message(bot: &RcBot, text: String, user_id: i64, chat_id: i64) -> impl Future<Item = (), Error = Error> {
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
                let state = state.next(Event::AddUserId { user_id });
                Either::A(Either::B(state.publish(bot, chat_id)))
            }
            Some(state) => {
                hashmap.insert(user_id, state);
                let future = bot.message(chat_id, "cannot publish yet".to_string()).send().map(nullify);
                Either::B(future)
            }
            _ => {
                let future = bot.message(chat_id, "cannot publish yet".to_string()).send().map(nullify);
                Either::B(future)
            }
        }
    } else {
        let mut hashmap = HASHMAP.lock().unwrap();
        let state = hashmap.remove(&user_id);
        match state {
            Some(state) => {
                let event = match state {
                    State::Start => Event::AddName {
                        name: format!("{}_by_{}", text, *BOT_NAME),
                    },
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
