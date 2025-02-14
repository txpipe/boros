use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use serde::Deserialize;

const DEFAULT_QUOTE: usize = 40;
fn default_quote() -> usize {
    DEFAULT_QUOTE
}

pub fn distribute_quote(config: Config) -> HashMap<String, usize> {
    let total_weight: f32 = config.queues.iter().map(|q| q.weight as f32).sum();

    config
        .queues
        .iter()
        .map(|q| {
            let quote = (q.weight as f32 / total_weight).round() as usize * config.quote;
            (q.name.clone(), quote)
        })
        .collect()
}

#[derive(Debug, Deserialize, Clone)]
#[allow(unused)]
pub struct Config {
    #[serde(default = "default_quote")]
    pub quote: usize,
    #[serde(default)]
    pub queues: HashSet<Queue>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            quote: DEFAULT_QUOTE,
            queues: Default::default(),
        }
    }
}

#[derive(Debug, Deserialize, Clone, Eq)]
#[allow(unused)]
pub struct Queue {
    pub name: String,
    pub weight: usize,
}
impl Default for Queue {
    fn default() -> Self {
        Self {
            name: "default".into(),
            weight: 1,
        }
    }
}
impl Hash for Queue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}
impl PartialEq for Queue {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}
