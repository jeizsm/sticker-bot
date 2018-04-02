use std::io::Cursor;
use std::env;
use std::collections::HashMap;
use std::sync::Mutex;

use futures::future::{ok, Either, Future};
use futures::Stream;
use hyper::Client;
use hyper_rustls::HttpsConnector;
use telebot::RcBot;
use telebot::objects::{File, Message, Update};
use telebot::functions::{FunctionCreateNewStickerSet, FunctionGetFile, FunctionMessage, FunctionAddStickerToSet};
use magick_rust::MagickWand;
use failure::{err_msg, Error, Fail};

use types::{StateMachine, ErrorKind};

lazy_static! {
    static ref HASHMAP: Mutex<HashMap<i64, StateMachine>> = { Mutex::new(HashMap::new()) };
    static ref USER_ID: i64 = { env::var("USER_ID").unwrap().parse().unwrap() };
    pub static ref TELEGRAM_TOKEN: String = { env::var("TELEGRAM_TOKEN").unwrap() };
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
                return Either::A(ok(()));
            }
        }
        None => return Either::A(ok(())),
    };
    let chat_id = message.chat.id;
    let send_message =
        move |(bot, file): (RcBot, File)| send_message((bot, file), user_id, chat_id, &client);
    match message {
        Message {
            text: Some(text), ..
        } => {
            let future = text_message(bot, text, user_id, chat_id, user_name);
            Either::B(Either::A(future))
        }
        Message {
            photo: Some(photos),
            ..
        } => {
            let photo = photos.last().unwrap();
            let id = photo.file_id.clone();
            let future = bot.get_file(id).send().and_then(send_message).map(nullify);
            Either::B(Either::B(future))
        }

        Message {
            document: Some(document),
            ..
        } => {
            let id = document.file_id.clone();
            let future = bot.get_file(id).send().and_then(send_message).map(nullify);
            Either::B(Either::B(future))
        }

        Message {
            sticker: Some(sticker),
            ..
        } => {
            let id = sticker.file_id.clone();
            let future = bot.get_file(id).send().and_then(send_message).map(nullify);
            Either::B(Either::B(future))
        }
        _ => Either::A(ok(())),
    }
}
fn send_message(
    (bot, file): (RcBot, File),
    user_id: i64,
    chat_id: i64,
    client: &Client<HttpsConnector>,
) -> impl Future<Item = (RcBot, Message), Error = Error> {
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
            let mut hashmap = HASHMAP.lock().unwrap();
            let entry = hashmap.remove(&user_id);
            match entry {
                Some(entry) => match entry {
                    StateMachine::Start => {
                        let new_entry = StateMachine::First { file: image };
                        let string = new_entry.to_string();
                        hashmap.insert(user_id, new_entry);
                        bot.message(chat_id, string).send()
                    }
                    _ => bot.message(chat_id, "cannot image now".to_string()).send(),
                },
                _ => bot.message(chat_id, "cannot image now".to_string()).send(),
            }
        })
}

fn nullify((_, _): (RcBot, Message)) {
    ()
}

fn text_message(
    bot: &RcBot,
    text: String,
    user_id: i64,
    chat_id: i64,
    user_name: String,
) -> impl Future<Item = (), Error = Error> {
    if text.starts_with("/new_pack") {
        let step = StateMachine::Start;
        let future = bot.message(chat_id, step.to_string()).send().map(nullify);
        HASHMAP.lock().unwrap().insert(user_id, step);
        Either::A(future)
    } else if text.starts_with("/add_to_pack") {
        let step = StateMachine::Start;
        let future = bot.message(chat_id, step.to_string()).send().map(nullify);
        HASHMAP.lock().unwrap().insert(user_id, step);
        Either::A(future)
    } else if text.starts_with("/publish") {
        let mut hashmap = HASHMAP.lock().unwrap();
        let entry = hashmap.remove(&user_id);
        match entry {
            Some(entry) => Either::B(publish(bot, entry, user_id, chat_id, user_name)),
            _ => {
                let future = bot.message(chat_id, "cannot publish yet".to_string())
                    .send()
                    .map(nullify);
                Either::A(future)
            }
        }
    } else {
        let mut hashmap = HASHMAP.lock().unwrap();
        let entry = hashmap.remove(&user_id);
        match entry {
            Some(entry) => {
                let new_entry = get_new_entry(entry, text);
                let string = new_entry.to_string();
                hashmap.insert(user_id, new_entry);
                let future = bot.message(chat_id, string).send().map(nullify);
                Either::A(future)
            }
            None => {
                let future = bot.message(chat_id, text).send().map(nullify);
                Either::A(future)
            }
        }
    }
}

fn get_new_entry(entry: StateMachine, text: String) -> StateMachine {
    match entry {
        StateMachine::First { file } => StateMachine::Second {
            file,
            emojis: text,
        },
        StateMachine::Second { file, emojis } => StateMachine::End {
            file: file,
            emojis: emojis,
            title: text,
        },
        _ => unreachable!(),
    }
}

fn publish(bot: &RcBot, state: StateMachine, user_id: i64, chat_id: i64, user_name: String) -> impl Future<Item = (), Error = Error> {
    match state {

    StateMachine::Second {
        emojis, file
    } => {
        let text = format!("{}_by_smm_test_bot", user_name);
        let future = bot.add_sticker_to_set(user_id, text, emojis)
            .file(("test.png", Cursor::new(file)))
            .send()
            .and_then(move |(bot, a)| bot.message(chat_id, a.to_string()).send())
            .map(nullify);
        Either::B(Either::A(future))
    },
    StateMachine::End {
        title,
        emojis,
        file,
    } => {
        let text = format!("{}_by_smm_test_bot", user_name);
        let future = bot.create_new_sticker_set(user_id, text, title, emojis)
            .file(("test.png", Cursor::new(file)))
            .send()
            .and_then(move |(bot, a)| bot.message(chat_id, a.to_string()).send())
            .map(nullify);
        Either::B(Either::B(future))
    },
    _ => {
        let future = bot.message(chat_id, "cannot publish yet".to_string())
            .send()
            .map(nullify);
        Either::A(future)
    }
    }
}
