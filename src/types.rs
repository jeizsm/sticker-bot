use std::fmt::{Display, Formatter, Result as DisplayResult};
use std::io::Cursor;

use failure::Error;
use futures::future::{Either, Future};
use telebot::RcBot;
use telebot::functions::{FunctionAddStickerToSet, FunctionCreateNewStickerSet, FunctionMessage};
use telebot::objects::Message;

#[derive(Debug)]
pub(crate) enum State {
    Start,
    Sticker {
        file: Vec<u8>,
    },
    Emojis {
        emojis: String,
        file: Vec<u8>,
    },
    Title {
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
    AddSticker { file: Vec<u8> },
    AddEmojis { emojis: String },
    AddTitle { title: String },
    AddName { name: String, user_id: i64 },
    DoNothing,
}

impl State {
    pub(crate) fn new() -> State {
        State::Start
    }

    pub(crate) fn next(self, event: Event) -> State {
        match (self, event) {
            (State::Start, Event::AddSticker { file }) => State::Sticker { file },
            (State::Sticker { file }, Event::AddEmojis { emojis }) => {
                State::Emojis { file, emojis }
            }
            (State::Emojis { file, emojis }, Event::AddTitle { title }) => State::Title {
                file,
                emojis,
                title,
            },
            (State::Emojis { file, emojis }, Event::AddName { name, user_id }) => State::End {
                file,
                emojis,
                name,
                user_id,
                title: None,
            },
            (
                State::Title {
                    file,
                    emojis,
                    title,
                },
                Event::AddName { name, user_id },
            ) => State::End {
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
            State::Start => bot.message(chat_id, self.to_string()).send().map(nullify),
            State::Sticker { .. } => bot.message(chat_id, self.to_string()).send().map(nullify),
            State::Emojis { .. } => bot.message(chat_id, self.to_string()).send().map(nullify),
            State::Title { .. } => bot.message(chat_id, self.to_string()).send().map(nullify),
            _ => bot.message(chat_id, "something went wrong".to_string())
                .send()
                .map(nullify),
        }
    }

    pub(crate) fn publish(
        self,
        bot: &RcBot,
        chat_id: i64,
    ) -> impl Future<Item = (), Error = Error> {
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
                let future = bot.message(chat_id, "cannot publish yet".to_string())
                    .send()
                    .map(nullify);
                Either::B(future)
            }
        }
    }
}

impl Display for State {
    fn fmt(&self, f: &mut Formatter) -> DisplayResult {
        match self {
            State::Start { .. } => write!(f, "Send photo: "),
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
