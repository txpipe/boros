use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use itertools::Itertools;
use serde::Deserialize;

use crate::storage::sqlite::SqliteTransaction;
use crate::storage::{Transaction, TransactionStatus};

pub const DEFAULT_QUEUE: &str = "default";
pub const DEFAULT_WEIGHT: usize = 1;
const DEFAULT_QUOTE: usize = 40;
fn default_quote() -> usize {
    DEFAULT_QUOTE
}

pub struct Priority {
    storage: Arc<SqliteTransaction>,
    config: Config,
}
impl Priority {
    pub fn new(storage: Arc<SqliteTransaction>, config: Config) -> Self {
        Self { storage, config }
    }

    fn quotes(&self, queues: HashMap<String, usize>) -> HashMap<String, usize> {
        let total_weight: f32 = queues.values().map(|weight| *weight as f32).sum();

        queues
            .iter()
            .map(|(name, weight)| {
                let quote =
                    ((*weight as f32 / total_weight) * self.config.quote as f32).round() as usize;
                (name.clone(), quote)
            })
            .sorted_by(|a, b| Ord::cmp(&b.1, &a.1))
            .collect()
    }

    pub async fn next(&self, status: TransactionStatus) -> anyhow::Result<Vec<Transaction>> {
        let state = self.storage.state(status).await?;
        let active_queues = state
            .iter()
            .map(|s| {
                let weight = self
                    .config
                    .queues
                    .get(&s.queue)
                    .map(|q| q.weight)
                    .unwrap_or(DEFAULT_WEIGHT);
                (s.queue.clone(), weight)
            })
            .collect();
        let quotes = self.quotes(active_queues);
        let mut leftover = 0;
        let mut queue_limit: HashMap<String, usize> = HashMap::new();
        for (queue, quote) in quotes {
            let current_quote = quote + leftover;
            let current_state = state.iter().find(|s| s.queue.eq(&queue)).unwrap();
            let used_quote = std::cmp::min(current_state.count, current_quote);
            queue_limit.insert(queue, used_quote);
            leftover = current_quote - used_quote;
        }

        dbg!(queue_limit);

        Ok(Default::default())
    }
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
            name: DEFAULT_QUEUE.into(),
            weight: DEFAULT_WEIGHT,
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
impl Borrow<String> for Queue {
    fn borrow(&self) -> &String {
        &self.name
    }
}
