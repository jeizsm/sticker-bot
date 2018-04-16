use std::fmt::{Display, Formatter, Result as DisplayResult};
use std::io::Cursor;

use failure::Error;
use futures::future::{Either, Future};
use telebot::functions::{FunctionAddStickerToSet, FunctionCreateNewStickerSet, FunctionMessage};
use telebot::objects::Message;
use telebot::RcBot;

#[derive(Debug)]
pub(crate) enum State {
    Start,
    Name {
        name: String,
    },
    Sticker {
        name: String,
        file: Vec<u8>,
    },
    Emojis {
        name: String,
        emojis: String,
        file: Vec<u8>,
    },
    Title {
        name: String,
        title: String,
        emojis: String,
        file: Vec<u8>,
    },
    End {
        title: Option<String>,
        emojis: String,
        file: Vec<u8>,
        name: String,
        user_id: i64,
    }, // Error(Error)
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
    pub(crate) fn new() -> State {
        State::Start
    }

    pub(crate) fn next(self, event: Event) -> State {
        match (self, event) {
            (State::Start, Event::AddName { name }) => State::Name { name },
            (State::Name { name }, Event::AddSticker { file }) => State::Sticker { name, file },
            (State::Sticker { name, file }, Event::AddEmojis { emojis }) => State::Emojis { name, file, emojis },
            (State::Emojis { file, emojis, name }, Event::AddTitle { title }) => State::Title { file, emojis, title, name },
            (State::Emojis { file, emojis, name }, Event::AddUserId { user_id }) => State::End {
                file,
                emojis,
                name,
                user_id,
                title: None,
            },
            (State::Title { file, emojis, title, name }, Event::AddUserId { user_id }) => State::End {
                file,
                emojis,
                name,
                user_id,
                title: Some(title),
            },
            (state, _) => state,
        }
    }

    pub(crate) fn run(&self, bot: &RcBot, chat_id: i64) -> impl Future<Item = (), Error = Error> {
        match *self {
            State::Start | State::Name { .. } | State::Sticker { .. } | State::Emojis { .. } | State::Title { .. } => {
                bot.message(chat_id, self.to_string()).send().map(nullify)
            }
            State::End { .. } => bot.message(chat_id, "something went wrong".to_string()).send().map(nullify),
        }
    }

    pub(crate) fn publish(self, bot: &RcBot, chat_id: i64) -> impl Future<Item = (), Error = Error> {
        match self {
            State::End {
                file,
                emojis,
                name,
                title,
                user_id,
            } => match title {
                Some(title) => {
                    let url = format!("https://t.me/addstickers/{}", name);
                    let future = bot.create_new_sticker_set(user_id, name, title, emojis)
                        .file(("test.png", Cursor::new(file)))
                        .send()
                        .and_then(move |(bot, _)| bot.message(chat_id, url).send())
                        .map(nullify);
                    Either::A(Either::A(future))
                }
                None => {
                    let url = format!("https://t.me/addstickers/{}", name);
                    let future = bot.add_sticker_to_set(user_id, name, emojis)
                        .file(("test.png", Cursor::new(file)))
                        .send()
                        .and_then(move |(bot, _)| bot.message(chat_id, url).send())
                        .map(nullify);
                    Either::A(Either::B(future))
                }
            },
            _ => {
                let future = bot.message(chat_id, "cannot publish yet".to_string()).send().map(nullify);
                Either::B(future)
            }
        }
    }
}

impl Display for State {
    fn fmt(&self, f: &mut Formatter) -> DisplayResult {
        match self {
            State::Start => write!(f, "Send name: "),
            State::Name { .. } => write!(f, "Send photo or sticker: "),
            State::Sticker { .. } => write!(f, "Send emoji: "),
            State::Emojis { .. } => write!(f, "Send title or /publish: "),
            State::Title { .. } => write!(f, "Send /publish"),
            State::End { .. } => write!(f, "Send /publish"),
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub(crate) enum ErrorKind {
    // indicates some failure in Hyper, missing network connection, etc.
    #[fail(display = "There was an error fetching the content")]
    Hyper,
}

pub(crate) fn nullify((_, _): (RcBot, Message)) {
    ()
}
