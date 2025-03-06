use std::borrow::Borrow;
use std::hash::{Hash, Hasher};

use serde::Deserialize;

pub mod chaining;
pub mod priority;

pub const DEFAULT_QUEUE: &str = "default";
pub const DEFAULT_WEIGHT: u8 = 1;

#[derive(Debug, Deserialize, Clone, Eq)]
pub struct Config {
    pub name: String,
    pub weight: u8,
    #[serde(default)]
    pub chained: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            name: DEFAULT_QUEUE.into(),
            weight: DEFAULT_WEIGHT,
            chained: false,
        }
    }
}
impl Hash for Config {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}
impl PartialEq for Config {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}
impl Borrow<String> for Config {
    fn borrow(&self) -> &String {
        &self.name
    }
}
