use failure::{err_msg, Error, Fail};
use futures::future::{ok, Either, Future};
use futures::Stream;
use helpers::{nullify, CONFIG, STICKER_DB, STICKER_PACK_DB};
use magick_rust::MagickWand;
use telebot::functions::{FunctionDeleteStickerFromSet, FunctionGetFile, FunctionMessage};
use telebot::objects::{File, Message, Update};
use telebot::RcBot;
use types::{ErrorKind, Event, HttpsClient, State};

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
            if user.id == CONFIG.user_id {
                user.id
            } else {
                return Either::A(Either::A(
                    bot.message(user.id, "sorry bot is not working for you".to_string())
                        .send()
                        .map(nullify),
                ));
            }
        }
        None => {
            return Either::A(Either::A(
                bot.message(message.chat.id, "cannot find user id(you shouldn't be here)".to_string())
                    .send()
                    .map(nullify),
            ))
        }
    };
    let send_message = move |(bot, file): (RcBot, File)| send_message((bot, file), user_id, &client);
    let get_file_and_send_message = |id: String| bot.get_file(id).send().and_then(send_message);
    match message {
        Message { text: Some(text), .. } => {
            let future = text_message(bot, text, user_id);
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
                Either::A(Either::A(bot.message(user_id, "it's not image".to_string()).send().map(nullify)))
            }
        }

        Message {
            sticker: Some(sticker), ..
        } => {
            let id = sticker.file_id;
            let set_from_bot = sticker.set_name.map_or(false, |a| a.ends_with(&CONFIG.bot_name));
            if set_from_bot {
                let future = bot
                    .delete_sticker_from_set(id)
                    .send()
                    .and_then(move |(bot, _)| bot.message(user_id, "sticker deleted".to_string()).send().map(nullify));
                Either::A(Either::B(future))
            } else {
                let future = get_file_and_send_message(id);
                Either::B(Either::B(future))
            }
        }
        _ => Either::A(Either::A(
            bot.message(user_id, "something went wrong".to_string()).send().map(nullify),
        )),
    }
}

fn send_message((bot, file): (RcBot, File), user_id: i64, client: &HttpsClient) -> impl Future<Item = (), Error = Error> {
    let url = format!(
        "https://api.telegram.org/file/bot{}/{}",
        CONFIG.telegram_token,
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
            let state = STICKER_DB.get(&user_id).unwrap().unwrap();
            let state = state.next(Event::AddSticker { file: image });
            let future = state.run(&bot, user_id);
            STICKER_DB.set(&user_id, &state).unwrap();
            future
        })
}

fn text_message(bot: &RcBot, text: String, user_id: i64) -> impl Future<Item = (), Error = Error> {
    if text.starts_with("/new_pack") || text.starts_with("/add_to_pack") {
        let state = State::new(text.parse().unwrap());
        let future = state.run(bot, user_id);
        STICKER_DB.set(&user_id, &state).unwrap();
        Either::A(Either::A(future))
    } else if text.starts_with("/publish") {
        let state = STICKER_DB.get(&user_id).unwrap();
        match state {
            Some(state @ State::Emojis { .. }) | Some(state @ State::Title { .. }) => {
                let state = state.next(Event::AddUserId { user_id });
                Either::A(Either::B(state.publish(bot, user_id)))
            }
            Some(_) => {
                let future = bot.message(user_id, "cannot publish yet".to_string()).send().map(nullify);
                Either::B(future)
            }
            None => {
                let future = bot.message(user_id, "cannot publish yet".to_string()).send().map(nullify);
                Either::B(future)
            }
        }
    } else if text.starts_with("/add_sticker_pack") {
        let mut names = text.trim_left_matches("/add_sticker_pack ").split_whitespace();
        if let Some(name) = names.next() {
            if name.ends_with(&format!("by_{}", CONFIG.bot_name)) {
                let name = name.to_string();
                STICKER_PACK_DB.merge(&user_id, &name).unwrap();
                let future = bot.message(user_id, format!("added to user sticker packs {}", name)).send().map(nullify);
                return Either::B(future);
            }
        }
        let future = bot.message(user_id, "name not found".to_string()).send().map(nullify);
        Either::B(future)
    } else {
        let state = STICKER_DB.get(&user_id).unwrap();
        match state {
            Some(state) => {
                let event = match state {
                    State::Start(_) => Event::AddName {
                        name: format!("{}_by_{}", text, CONFIG.bot_name),
                    },
                    State::Sticker { .. } => Event::AddEmojis { emojis: text },
                    State::Emojis { .. } => Event::AddTitle { title: text },
                    _ => Event::DoNothing,
                };
                let state = state.next(event);
                let future = state.run(bot, user_id);
                STICKER_DB.set(&user_id, &state).unwrap();
                Either::A(Either::A(future))
            }
            None => {
                let future = bot.message(user_id, text).send().map(nullify);
                Either::B(future)
            }
        }
    }
}
