use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use itertools::Itertools;
use serde::Deserialize;

use crate::storage::sqlite::SqliteTransaction;
use crate::storage::{Transaction, TransactionStatus};

pub const DEFAULT_QUEUE: &str = "default";
pub const DEFAULT_WEIGHT: u8 = 1;
pub const DEFAULT_QUOTA: u16 = 60;

pub struct Priority {
    storage: Arc<SqliteTransaction>,
    quota: u16,
    queues: HashSet<QueueConfig>,
}
impl Priority {
    pub fn new(storage: Arc<SqliteTransaction>, queues: HashSet<QueueConfig>) -> Self {
        Self {
            storage,
            quota: DEFAULT_QUOTA,
            queues,
        }
    }

    fn quota(&self, queues: HashMap<String, u8>) -> HashMap<String, u16> {
        let total_weight: f32 = queues.values().map(|weight| *weight as f32).sum();

        queues
            .iter()
            .map(|(name, weight)| {
                let quota = ((*weight as f32 / total_weight) * self.quota as f32).round() as u16;
                (name.clone(), quota)
            })
            .sorted_by(|a, b| Ord::cmp(&b.1, &a.1))
            .collect()
    }

    pub async fn next(&self, status: TransactionStatus) -> anyhow::Result<Vec<Transaction>> {
        let state = self.storage.state(status.clone()).await?;
        if state.is_empty() {
            return Ok(Vec::new());
        }

        let state_queues = state
            .iter()
            .map(|s| {
                let weight = self
                    .queues
                    .get(&s.queue)
                    .map(|q| q.weight)
                    .unwrap_or(DEFAULT_WEIGHT);
                (s.queue.clone(), weight)
            })
            .collect();

        let quota = self.quota(state_queues);
        let mut leftover = 0;
        let mut queue_limit = HashMap::new();
        for (queue, quota) in quota {
            let current_quota = quota + leftover;
            let current_state = state.iter().find(|s| s.queue.eq(&queue)).unwrap();
            let used_quota = std::cmp::min(current_state.count, current_quota.into()) as u16;
            queue_limit.insert(queue, used_quota);
            leftover = current_quota - used_quota;
        }

        let transactions = self.storage.next(status, queue_limit).await?;
        Ok(transactions)
    }
}

#[derive(Debug, Deserialize, Clone, Eq)]
pub struct QueueConfig {
    pub name: String,
    pub weight: u8,
}
impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            name: DEFAULT_QUEUE.into(),
            weight: DEFAULT_WEIGHT,
        }
    }
}
impl Hash for QueueConfig {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}
impl PartialEq for QueueConfig {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}
impl Borrow<String> for QueueConfig {
    fn borrow(&self) -> &String {
        &self.name
    }
}

#[cfg(test)]
mod priority_tests {
    use std::collections::{HashMap, HashSet};

    use priority::{Priority, QueueConfig};
    use storage::{
        sqlite::sqlite_utils_tests::{mock_sqlite_transaction, TransactionList},
        TransactionStatus,
    };

    use crate::*;

    impl Priority {
        pub fn new_test(
            storage: Arc<SqliteTransaction>,
            quota: u16,
            queues: HashSet<QueueConfig>,
        ) -> Self {
            Self {
                storage,
                quota,
                queues,
            }
        }
    }

    #[tokio::test]
    async fn it_should_calculate_quota() {
        let storage = Arc::new(mock_sqlite_transaction().await);
        let priority = Priority::new_test(storage, 10, Default::default());

        let queues = HashMap::from_iter([
            ("default".into(), 1),
            ("banana".into(), 2),
            ("orange".into(), 2),
        ]);
        let quota = priority.quota(queues);

        let default = quota.get("default").unwrap();
        assert!(*default == 2);
        let banana = quota.get("banana").unwrap();
        assert!(*banana == 4);
        let orange = quota.get("orange").unwrap();
        assert!(*orange == 4);
    }

    #[tokio::test]
    async fn it_should_return_next_transactions() {
        let storage = Arc::new(mock_sqlite_transaction().await);
        let priority = Priority::new_test(
            storage.clone(),
            4,
            HashSet::from_iter([
                QueueConfig {
                    name: "default".into(),
                    weight: 1,
                },
                QueueConfig {
                    name: "banana".into(),
                    weight: 2,
                },
                QueueConfig {
                    name: "orange".into(),
                    weight: 1,
                },
            ]),
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
        let storage = Arc::new(mock_sqlite_transaction().await);

        let priority = Priority::new_test(
            storage.clone(),
            4,
            HashSet::from_iter([QueueConfig {
                name: "default".into(),
                weight: 1,
            }]),
        );

        let data = vec![("1", "banana"), ("2", "banana"), ("3", "default")];
        let transaction_list: TransactionList = data.into();
        storage.create(&transaction_list.0).await.unwrap();

        let transactions = priority.next(TransactionStatus::Pending).await.unwrap();
        assert!(transactions.len() == 3);
    }
}
