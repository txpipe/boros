pub mod model;
pub mod sources;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("configuration error: {0}")]
    ConfigError(String),
}
