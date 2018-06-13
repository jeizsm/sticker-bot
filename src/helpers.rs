use types::Config;
use envy;

fn config() -> Config {
    match envy::from_env() {
        Ok(config) => config,
        Err(error) => panic!("{:#?}", error)
    }
}

lazy_static! {
    pub(crate) static ref CONFIG: Config = config();
}
