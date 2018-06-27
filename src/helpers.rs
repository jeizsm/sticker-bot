use byteorder::{ByteOrder, NativeEndian};
use envy;
use sled::Tree;
use sled::{Config as SledConfig, ConfigBuilder};
use std::mem::size_of;
use std::path::Path;
use telebot::objects::Message;
use telebot::RcBot;
use types::Config;
use types::State;
use types::TypedDB;

fn config() -> Config {
    match envy::from_env() {
        Ok(config) => config,
        Err(error) => panic!("{:#?}", error),
    }
}

fn concatenate_merge(_key: &[u8], old_value: Option<&[u8]>, merged_bytes: &[u8]) -> Option<Vec<u8>> {
    let size = size_of::<usize>();
    let mut ret = old_value.map(|ov| ov.to_vec()).unwrap_or_else(|| vec![0; size]);

    if ret.len() >= size {
        let a = NativeEndian::read_uint(&ret, size);
        NativeEndian::write_uint(&mut ret, a + 1, size);
    } else {
        NativeEndian::write_uint(&mut ret, 1, size);
    }

    ret.extend_from_slice(merged_bytes);

    Some(ret)
}

fn sled_config(db_name: &str, merge: bool) -> SledConfig {
    let path = Path::new(&CONFIG.sled_db_dir).join(db_name);
    let config = ConfigBuilder::new().path(path);
    let config = if merge { config.merge_operator(concatenate_merge) } else { config };
    config.build()
}

type StickerDB<'a> = TypedDB<'a, i64, State>;
type StickerPackDB<'a> = TypedDB<'a, i64, Vec<String>>;

lazy_static! {
    pub(crate) static ref CONFIG: Config = config();
    static ref STICKER_TREE: Tree = Tree::start(sled_config("sticker.db", false)).unwrap();
    pub(crate) static ref STICKER_DB: StickerDB<'static> = StickerDB::new(&STICKER_TREE);
    pub(crate) static ref STICKER_PACK_TREE: Tree = Tree::start(sled_config("sticker_pack.db", true)).unwrap();
    pub(crate) static ref STICKER_PACK_DB: StickerPackDB<'static> = StickerPackDB::new(&STICKER_PACK_TREE);
}

pub(crate) fn nullify((_, _): (RcBot, Message)) {
    ()
}
