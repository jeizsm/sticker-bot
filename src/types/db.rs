use std::marker::PhantomData;
use bincode::{serialize, deserialize};
use serde::Serialize;
use serde::de::DeserializeOwned;
use sled::{Tree, DbResult};

pub(crate) struct TypedDB<'a, K, V> {
    db: &'a Tree,
    phantom_key: PhantomData<K>,
    phantom_value: PhantomData<V>,
}

impl<'a, K, V> TypedDB<'a, K, V>
where
    K: Serialize,
    V: Serialize + DeserializeOwned,
    Option<V>: From<Option<Vec<u8>>>
{
    pub(crate) fn new(db: &'a Tree) -> Self {
        Self {
            db,
            phantom_key: PhantomData,
            phantom_value: PhantomData,
        }
    }

    pub(crate) fn get(&self, key: &K) -> DbResult<Option<V>, ()> {
        let key = serialize(&key).unwrap();
        self.db.get(&key).map(|v| v.map(|a| deserialize(&a).unwrap()))
    }

    pub(crate) fn cas(&self, key: &K, old: Option<&V>, new: Option<&V>) -> DbResult<(), Option<V>> {
        let key = serialize(key).unwrap();
        let old = old.map(|value| serialize(value).unwrap());
        let new = new.map(|value| serialize(value).unwrap());
        self.db.cas(key, old, new).map_err(|e| e.cast())
    }

    pub(crate) fn set(&self, key: &K, value: &V) -> DbResult<(), ()> {
        let key = serialize(key).unwrap();
        let value = serialize(value).unwrap();
        self.db.set(key, value)
    }
}
