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
        let state = self.storage.state(status.clone()).await?;
        let state_queues = state
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

        let quotes = self.quotes(state_queues);
        dbg!(&quotes);
        let mut leftover = 0;
        let mut queue_limit = HashMap::new();
        for (queue, quote) in quotes {
            let current_quote = quote + leftover;
            let current_state = state.iter().find(|s| s.queue.eq(&queue)).unwrap();
            let used_quote = std::cmp::min(current_state.count, current_quote);
            queue_limit.insert(queue, used_quote);
            leftover = current_quote - used_quote;
        }

        let transactions = self.storage.next(status, queue_limit).await?;
        Ok(transactions)
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

#[cfg(test)]
mod priority_tests {
    use std::collections::{HashMap, HashSet};

    use priority::{Config, Priority, Queue};
    use storage::{
        sqlite::sqlite_utils_tests::{mock_sqlite_transaction, TransactionList},
        TransactionStatus,
    };

    use crate::*;

    #[tokio::test]
    async fn it_should_calculate_quotes() {
        let storage = mock_sqlite_transaction().await;
        let priority = Priority::new(
            Arc::new(storage),
            Config {
                quote: 10,
                ..Default::default()
            },
        );

        let queues = HashMap::from_iter([
            ("default".into(), 1),
            ("banana".into(), 2),
            ("orange".into(), 2),
        ]);
        let quotes = priority.quotes(queues);

        let default = quotes.get("default").unwrap();
        assert!(*default == 2);
        let banana = quotes.get("banana").unwrap();
        assert!(*banana == 4);
        let orange = quotes.get("orange").unwrap();
        assert!(*orange == 4);
    }

    #[tokio::test]
    async fn it_should_return_next_transactions() {
        let storage = Arc::new(mock_sqlite_transaction().await);

        let priority = Priority::new(
            storage.clone(),
            Config {
                quote: 4,
                queues: HashSet::from_iter([
                    Queue {
                        name: "default".into(),
                        weight: 1,
                    },
                    Queue {
                        name: "banana".into(),
                        weight: 2,
                    },
                    Queue {
                        name: "orange".into(),
                        weight: 1,
                    },
                ]),
            },
        );

        let data = vec![
            ("1", "banana"),
            ("2", "banana"),
            ("3", "orange"),
            ("4", "orange"),
            ("5", "default"),
            ("6", "default"),
        ];
        let transaction_list: TransactionList = data.into();
        storage.create(&transaction_list.0).await.unwrap();

        let transactions = priority.next(TransactionStatus::Pending).await.unwrap();
        assert!(transactions.len() == 4);
        assert!(transactions.iter().filter(|t| t.queue == "banana").count() == 2);
        assert!(transactions.iter().filter(|t| t.queue == "orange").count() == 1);
        assert!(transactions.iter().filter(|t| t.queue == "default").count() == 1);
    }

    #[tokio::test]
    async fn it_should_return_next_transactions_when_a_queue_is_removed_from_config() {
        let env_filter = EnvFilter::builder()
            .with_default_directive(Level::INFO.into())
            .with_env_var("RUST_LOG")
            .from_env_lossy();

        tracing_subscriber::registry()
            .with(fmt::layer())
            .with(env_filter)
            .init();

        let storage = Arc::new(mock_sqlite_transaction().await);

        let priority = Priority::new(
            storage.clone(),
            Config {
                quote: 4,
                queues: HashSet::from_iter([Queue {
                    name: "default".into(),
                    weight: 1,
                }]),
            },
        );

        let data = vec![("1", "banana"), ("2", "banana"), ("3", "default")];
        let transaction_list: TransactionList = data.into();
        storage.create(&transaction_list.0).await.unwrap();

        let transactions = priority.next(TransactionStatus::Pending).await.unwrap();
        assert!(transactions.len() == 3);
        assert!(transactions.iter().filter(|t| t.queue == "banana").count() == 2);
        assert!(transactions.iter().filter(|t| t.queue == "default").count() == 1);
    }
}
