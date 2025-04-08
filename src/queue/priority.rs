use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use itertools::Itertools;

use crate::storage::sqlite::SqliteTransaction;
use crate::storage::{Transaction, TransactionStatus};

use super::{Config, DEFAULT_WEIGHT};

pub struct Priority {
    storage: Arc<SqliteTransaction>,
    queues: HashSet<Config>,
}
impl Priority {
    pub fn new(storage: Arc<SqliteTransaction>, queues: HashSet<Config>) -> Self {
        Self { storage, queues }
    }

    fn quota(&self, queues: HashMap<String, u8>, cap: u16) -> HashMap<String, u16> {
        let total_weight: f32 = queues.values().map(|weight| *weight as f32).sum();

        queues
            .iter()
            .map(|(name, weight)| {
                let quota = ((*weight as f32 / total_weight) * cap as f32).round() as u16;
                (name.clone(), quota)
            })
            .sorted_by(|a, b| Ord::cmp(&b.1, &a.1))
            .collect()
    }

    pub async fn next(
        &self,
        status: TransactionStatus,
        cap: u16,
    ) -> anyhow::Result<Vec<Transaction>> {
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

        let quota = self.quota(state_queues, cap);
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

#[cfg(test)]
mod priority_tests {
    use std::{
        collections::{HashMap, HashSet},
        sync::Arc,
    };

    use crate::{
        queue::{priority::Priority, Config},
        storage::{
            sqlite::sqlite_utils_tests::{mock_sqlite_transaction, TransactionList},
            TransactionStatus,
        },
    };

    #[tokio::test]
    async fn it_should_calculate_quota() {
        let storage = Arc::new(mock_sqlite_transaction().await);
        let priority = Priority::new(storage, Default::default());

        let queues = HashMap::from_iter([
            ("default".into(), 1),
            ("banana".into(), 2),
            ("orange".into(), 2),
        ]);
        let quota = priority.quota(queues, 10);

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
        let priority = Priority::new(
            storage.clone(),
            HashSet::from_iter([
                Config {
                    name: "default".into(),
                    weight: 1,
                    chained: false,
                    server_signing: false,
                },
                Config {
                    name: "banana".into(),
                    weight: 2,
                    chained: false,
                    server_signing: false,
                },
                Config {
                    name: "orange".into(),
                    weight: 1,
                    chained: false,
                    server_signing: false,
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

        let transactions = priority.next(TransactionStatus::Pending, 4).await.unwrap();
        assert!(transactions.len() == 4);
        assert!(transactions.iter().filter(|t| t.queue == "banana").count() == 2);
        assert!(transactions.iter().filter(|t| t.queue == "orange").count() == 1);
        assert!(transactions.iter().filter(|t| t.queue == "default").count() == 1);
    }

    #[tokio::test]
    async fn it_should_return_next_transactions_when_a_queue_is_removed_from_config() {
        let storage = Arc::new(mock_sqlite_transaction().await);

        let priority = Priority::new(
            storage.clone(),
            HashSet::from_iter([Config {
                name: "default".into(),
                weight: 1,
                chained: false,
                server_signing: false,
            }]),
        );

        let data = vec![("1", "banana"), ("2", "banana"), ("3", "default")];
        let transaction_list: TransactionList = data.into();
        storage.create(&transaction_list.0).await.unwrap();

        let transactions = priority.next(TransactionStatus::Pending, 4).await.unwrap();
        assert!(transactions.len() == 3);
    }
}
