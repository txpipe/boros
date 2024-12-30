pub mod chain;
pub mod fanout;
pub mod ingest;
pub mod model;
pub mod query;
pub mod sources;
pub mod state;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("configuration error: {0}")]
    ConfigError(String),
}
