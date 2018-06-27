use std::fmt::{Display, Formatter, Result as DisplayResult};
use std::io::Cursor;

use failure::Error;
use futures::future::{Either, Future};
use helpers::{nullify, STICKER_DB, STICKER_PACK_DB};
use std::str::FromStr;
use telebot::functions::{FunctionAddStickerToSet, FunctionCreateNewStickerSet, FunctionMessage};
use telebot::objects::{KeyboardButton, ReplyKeyboardMarkup, ReplyKeyboardRemove};
use telebot::RcBot;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum Pack {
    New,
    Existing,
}

impl FromStr for Pack {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "/new_pack" => Ok(Pack::New),
            "/add_to_pack" => Ok(Pack::Existing),
            _ => Err("only new_pack and add_to_pack is allowed"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(rustfmt, rustfmt_skip)]
pub(crate) enum State {
    Start(Pack),
    Name { pack: Pack, name: String },
    Sticker { pack: Pack, name: String, file: Vec<u8> },
    Emojis { pack: Pack, name: String, emojis: String, file: Vec<u8> },
    Title { pack: Pack, name: String, title: String, emojis: String, file: Vec<u8> },
    End { pack: Pack, title: Option<String>, emojis: String, file: Vec<u8>, name: String, user_id: i64 }
}

pub(crate) enum Event {
    AddName { name: String },
    AddSticker { file: Vec<u8> },
    AddEmojis { emojis: String },
    AddTitle { title: String },
    AddUserId { user_id: i64 },
    DoNothing,
}

impl State {
    pub(crate) fn new(start: Pack) -> State {
        State::Start(start)
    }

    #[cfg_attr(rustfmt, rustfmt_skip)]
    pub(crate) fn next(self, event: Event) -> State {
        match (self, event) {
            (State::Start(pack), Event::AddName { name }) =>
                State::Name { name, pack },
            (State::Name { name, pack }, Event::AddSticker { file }) =>
                State::Sticker { name, file, pack },
            (State::Sticker { name, file, pack }, Event::AddEmojis { emojis }) =>
                State::Emojis { name, file, emojis, pack },
            (State::Emojis { file, emojis, name, pack: Pack::New }, Event::AddTitle { title }) =>
                State::Title { file, emojis, title, name, pack: Pack::New },
            (State::Emojis { file, emojis, name, pack: Pack::Existing }, Event::AddUserId { user_id }) =>
                State::End { file, emojis, name, user_id, title: None, pack: Pack::Existing },
            (State::Title { file, emojis, title, name, pack: Pack::New }, Event::AddUserId { user_id }) =>
                State::End { file, emojis, name, user_id, title: Some(title), pack: Pack::New },
            (state, _) => state,
        }
    }

    pub(crate) fn run(&self, bot: &RcBot, user_id: i64) -> impl Future<Item = (), Error = Error> {
        match *self {
            State::Start(Pack::Existing) => {
                let sticker_packs = STICKER_PACK_DB.get(&user_id).unwrap().unwrap();
                let inline_keyboard_buttons: Vec<_> = sticker_packs
                    .into_iter()
                    .map(KeyboardButton::new)
                    .collect();
                bot.message(user_id, "choose your pack".to_string())
                    .reply_markup(ReplyKeyboardMarkup::new(vec![inline_keyboard_buttons]).resize_keyboard(true))
                    .send()
                    .map(nullify)
            }
            State::Start(Pack::New) | State::Name { .. } | State::Sticker { .. } | State::Emojis { .. } | State::Title { .. } => bot
                .message(user_id, self.to_string())
                .reply_markup(ReplyKeyboardRemove::new(true))
                .send()
                .map(nullify),
            State::End { .. } => bot.message(user_id, "something went wrong".to_string()).send().map(nullify),
        }
    }

    #[cfg_attr(rustfmt, rustfmt_skip)]
    pub(crate) fn publish(self, bot: &RcBot, user_id: i64) -> impl Future<Item = (), Error = Error> {
        match self {
            State::End { file, emojis, name, title, user_id, pack } => match title {
                Some(title) => {
                    let url = format!("https://t.me/addstickers/{}", name);
                    let future = bot
                        .create_new_sticker_set(user_id, name.clone(), title, emojis)
                        .file(("test.png", Cursor::new(file)))
                        .send()
                        .and_then(move |(bot, _)| bot.message(user_id, url).send())
                        .and_then(move |(_, _)| {
                            if let Pack::New = pack {
                                STICKER_PACK_DB.merge(&user_id, &name).unwrap();
                            }
                            STICKER_DB.del(&user_id).unwrap();
                            Ok(())
                        });
                    Either::A(Either::A(future))
                }
                None => {
                    let url = format!("https://t.me/addstickers/{}", name);
                    let future = bot
                        .add_sticker_to_set(user_id, name, emojis)
                        .file(("test.png", Cursor::new(file)))
                        .send()
                        .and_then(move |(bot, _)| bot.message(user_id, url).send())
                        .map(nullify);
                    Either::A(Either::B(future))
                }
            },
            _ => {
                let future = bot.message(user_id, "cannot publish yet".to_string()).send().map(nullify);
                Either::B(future)
            }
        }
    }
}

impl Display for State {
    fn fmt(&self, f: &mut Formatter) -> DisplayResult {
        match self {
            State::Start(..) => write!(f, "Send name: "),
            State::Name { .. } => write!(f, "Send photo or sticker: "),
            State::Sticker { .. } => write!(f, "Send emoji: "),
            State::Emojis { pack: Pack::New, .. } => write!(f, "Send title: "),
            State::Emojis { pack: Pack::Existing, .. } => write!(f, "Send /publish"),
            State::Title { .. } => write!(f, "Send /publish"),
            State::End { .. } => write!(f, "Send /publish"),
        }
    }
}
