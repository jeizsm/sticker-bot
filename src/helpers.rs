use types::Config;
use envy;
use sled::{Config as SledConfig, ConfigBuilder};
use std::path::Path;
use types::State;
use types::TypedDB;
use sled::Tree;
use telebot::objects::Message;
use telebot::RcBot;

fn config() -> Config {
    match envy::from_env() {
        Ok(config) => config,
        Err(error) => panic!("{:#?}", error)
    }
}

fn sled_config(db_name: &str) -> SledConfig {
    let path = Path::new(&CONFIG.sled_db_dir).join(db_name);
    ConfigBuilder::new().path(path).build()
}

type StickerDB<'a> = TypedDB<'a, i64, State>;

lazy_static! {
    pub(crate) static ref CONFIG: Config = config();
    static ref TREE: Tree = Tree::start(sled_config("sticker.db")).unwrap();
    pub(crate) static ref STICKER_DB: StickerDB<'static> = StickerDB::new(&TREE);
}

pub(crate) fn nullify((_, _): (RcBot, Message)) {
    ()
}
