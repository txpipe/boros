use std::{collections::HashMap, path::Path, sync::Arc};

use anyhow::{Error, Result};
use chrono::Utc;
use itertools::Itertools;
use sqlx::{sqlite::SqliteRow, FromRow, Row};

use super::{Cursor, Transaction, TransactionState, TransactionStatus};

pub struct SqliteStorage {
    db: sqlx::sqlite::SqlitePool,
}

impl SqliteStorage {
    pub async fn new(path: &Path) -> Result<Self> {
        let url = format!("sqlite:{}?mode=rwc", path.display());
        let db = sqlx::sqlite::SqlitePoolOptions::new().connect(&url).await?;

        Ok(Self { db })
    }

    pub async fn migrate(&self) -> Result<()> {
        sqlx::migrate!("src/storage/migrations")
            .run(&self.db)
            .await?;

        Ok(())
    }

    #[cfg(test)]
    pub async fn ephemeral() -> Result<Self> {
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await?;

        let out = Self { db };
        out.migrate().await?;

        Ok(out)
    }
}

impl FromRow<'_, SqliteRow> for Transaction {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        let status: &str = row.try_get("status")?;

        Ok(Self {
            id: row.try_get("id")?,
            raw: row.try_get("raw")?,
            status: status
                .parse()
                .map_err(|err: Error| sqlx::Error::Decode(err.into()))?,
            slot: row.try_get("slot")?,
            queue: row.try_get("queue")?,
            dependencies: None,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

impl FromRow<'_, SqliteRow> for TransactionState {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        Ok(Self {
            queue: row.try_get("queue")?,
            count: row.try_get("count")?,
        })
    }
}

pub struct SqliteTransaction {
    sqlite: Arc<SqliteStorage>,
}

impl SqliteTransaction {
    pub fn new(sqlite: Arc<SqliteStorage>) -> Self {
        Self { sqlite }
    }

    pub async fn create(&self, txs: &Vec<Transaction>) -> Result<()> {
        let mut db_tx = self.sqlite.db.begin().await?;

        for tx in txs {
            let status = tx.status.clone().to_string();

            sqlx::query!(
                r#"
                    INSERT INTO tx (
                        id,
                        raw,
                        status,
                        queue,
                        created_at,
                        updated_at
                    )
                    VALUES ($1, $2, $3, $4, $5, $6)
                "#,
                tx.id,
                tx.raw,
                status,
                tx.queue,
                tx.created_at,
                tx.updated_at
            )
            .execute(&mut *db_tx)
            .await?;

            if let Some(dependencies) = &tx.dependencies {
                for required_id in dependencies {
                    sqlx::query!(
                        r#"
                            INSERT INTO tx_dependence (
                                dependent_id,
                                required_id
                            )
                            VALUES ($1, $2)
                        "#,
                        tx.id,
                        required_id,
                    )
                    .execute(&mut *db_tx)
                    .await?;
                }
            }
        }

        db_tx.commit().await?;
        Ok(())
    }

    pub async fn find(&self, status: TransactionStatus) -> Result<Vec<Transaction>> {
        let transactions = sqlx::query_as::<_, Transaction>(
            r#"
                    SELECT
                    	id,
                    	raw,
                    	status,
                        slot,
                    	queue,
                    	created_at,
                    	updated_at
                    FROM
                    	tx
                    WHERE
                    	tx.status = $1;
            "#,
        )
        .bind(status.to_string())
        .fetch_all(&self.sqlite.db)
        .await?;

        Ok(transactions)
    }

    pub async fn find_to_rollback(&self, slot: u64) -> Result<Vec<Transaction>> {
        let status = TransactionStatus::Confirmed.to_string();
        let slot = slot as i64;

        let transactions = sqlx::query_as::<_, Transaction>(
            r#"
                    SELECT
                    	id,
                    	raw,
                    	status,
                        slot,
                    	queue,
                    	created_at,
                    	updated_at
                    FROM
                    	tx
                    WHERE
	                    tx.status = $1 AND tx.slot > $2;
            "#,
        )
        .bind(status)
        .bind(slot)
        .fetch_all(&self.sqlite.db)
        .await?;

        Ok(transactions)
    }

    pub async fn latest(&self, queue: &str) -> Result<Option<Transaction>> {
        // TODO: Add conditional to remove invalid transactions when there's an invalid status.
        let transaction = sqlx::query_as::<_, Transaction>(
            r#"
                    SELECT
                    	id,
                    	raw,
                    	status,
                    	slot,
                    	queue,
                    	created_at,
                    	updated_at
                    FROM
                        tx
                    WHERE
                    	tx.queue = $1
                    ORDER BY
                    	created_at DESC
                    LIMIT 1;
            "#,
        )
        .bind(queue)
        .fetch_optional(&self.sqlite.db)
        .await?;

        Ok(transaction)
    }

    pub async fn next(
        &self,
        status: TransactionStatus,
        queues: HashMap<String, u16>,
    ) -> Result<Vec<Transaction>> {
        let mut param_index = 2;
        let query = queues
            .iter()
            .map(|_| {
                let queue_param = param_index;
                let limit_param = param_index + 1;
                param_index += 2;
                format!(
                    r#"
                        SELECT
                        	id,
                        	raw,
                        	status,
                        	slot,
                        	queue,
                        	created_at,
                        	updated_at
                        FROM (
                        	SELECT
                        		*
                        	FROM
                        		tx
                        	WHERE
                        		tx.status = $1 AND tx.queue = ${}
                        	ORDER BY
                        		created_at
                        	LIMIT ${}
                        )
                    "#,
                    queue_param, limit_param
                )
            })
            .join("UNION ALL");

        let mut q = sqlx::query_as::<_, Transaction>(&query).bind(status.to_string());
        for (queue, limit) in queues {
            q = q.bind(queue).bind(limit);
        }
        let transactions = q.fetch_all(&self.sqlite.db).await?;

        Ok(transactions)
    }

    pub async fn state(&self, status: TransactionStatus) -> Result<Vec<TransactionState>> {
        let state = sqlx::query_as::<_, TransactionState>(
            r#"
                    SELECT
                    	queue,
                    	COUNT(*) as count
                    FROM
                    	tx
                    WHERE
                    	tx.status = $1
                    GROUP BY
                    	tx.queue;
            "#,
        )
        .bind(status.to_string())
        .fetch_all(&self.sqlite.db)
        .await?;

        Ok(state)
    }

    pub async fn update(&self, tx: &Transaction) -> Result<()> {
        let status = tx.status.to_string();
        let updated_at = Utc::now();
        // TODO: check the maximium size of i64 and compare with cardano slot.
        let slot = tx.slot.map(|v| v as i64);

        sqlx::query!(
            r#"
                UPDATE
                	tx
                SET
                	raw = $1,
                	status = $2,
                	slot = $3,
                	updated_at = $4
                WHERE
                	id = $5;
            "#,
            tx.raw,
            status,
            slot,
            updated_at,
            tx.id,
        )
        .execute(&self.sqlite.db)
        .await?;

        Ok(())
    }

    pub async fn update_batch(&self, txs: &Vec<Transaction>) -> Result<()> {
        let mut db_tx = self.sqlite.db.begin().await?;

        for tx in txs {
            let status = tx.status.to_string();
            let updated_at = Utc::now();
            // TODO: check the maximium size of i64 and compare with cardano slot.
            let slot = tx.slot.map(|v| v as i64);

            sqlx::query!(
                r#"
                UPDATE
                	tx
                SET
                	raw = $1,
                	status = $2,
                	slot = $3,
                	updated_at = $4
                WHERE
                	id = $5;
            "#,
                tx.raw,
                status,
                slot,
                updated_at,
                tx.id,
            )
            .execute(&mut *db_tx)
            .await?;
        }

        db_tx.commit().await?;
        Ok(())
    }
}

impl FromRow<'_, SqliteRow> for Cursor {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        Ok(Self {
            slot: row.try_get("slot")?,
            hash: row.try_get("hash")?,
        })
    }
}

pub struct SqliteCursor {
    sqlite: Arc<SqliteStorage>,
}
impl SqliteCursor {
    pub fn new(sqlite: Arc<SqliteStorage>) -> Self {
        Self { sqlite }
    }

    pub async fn set(&self, cursor: &Cursor) -> Result<()> {
        let slot = cursor.slot as i64;

        sqlx::query!(
            r#"
                INSERT OR REPLACE INTO cursor(
                    id,
	                slot,
	                hash
                )
                VALUES (0, $1, $2);
            "#,
            slot,
            cursor.hash
        )
        .execute(&self.sqlite.db)
        .await?;

        Ok(())
    }

    pub async fn current(&self) -> Result<Option<Cursor>> {
        let cursor = sqlx::query_as::<_, Cursor>(
            r#"
                    SELECT
                    	slot,
                    	hash
                    FROM
                    	cursor;
            "#,
        )
        .fetch_optional(&self.sqlite.db)
        .await?;

        Ok(cursor)
    }
}

#[cfg(test)]
pub mod sqlite_utils_tests {
    use std::sync::Arc;

    use crate::storage::Transaction;

    use super::{SqliteCursor, SqliteStorage, SqliteTransaction};

    pub async fn mock_sqlite_transaction() -> SqliteTransaction {
        let sqlite_storage = Arc::new(SqliteStorage::ephemeral().await.unwrap());
        SqliteTransaction::new(sqlite_storage)
    }

    pub async fn mock_sqlite_cursor() -> SqliteCursor {
        let sqlite_storage = Arc::new(SqliteStorage::ephemeral().await.unwrap());
        SqliteCursor::new(sqlite_storage)
    }

    pub struct TransactionList(pub Vec<Transaction>);
    impl<T, U> From<Vec<(T, U)>> for TransactionList
    where
        T: Into<String>,
        U: Into<String>,
    {
        fn from(tuples: Vec<(T, U)>) -> Self {
            let transactions = tuples
                .into_iter()
                .map(|(id, queue)| Transaction {
                    id: id.into(),
                    queue: queue.into(),
                    ..Default::default()
                })
                .collect();
            TransactionList(transactions)
        }
    }
}

#[cfg(test)]
mod sqlite_transaction_tests {
    use std::collections::HashMap;

    use chrono::{Days, Utc};

    use crate::{
        queue::DEFAULT_QUEUE,
        storage::{
            sqlite::sqlite_utils_tests::{mock_sqlite_transaction, TransactionList},
            Transaction, TransactionStatus,
        },
    };

    #[tokio::test]
    async fn it_should_create() {
        let storage = mock_sqlite_transaction().await;
        let transaction = Transaction::default();

        let result = storage.create(&vec![transaction]).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_should_create_with_dependencies() {
        let storage = mock_sqlite_transaction().await;
        let mut transaction_1 = Transaction::default();
        transaction_1.id = "hex1".into();

        let mut transaction_2 = Transaction::default();
        transaction_2.id = "hex2".into();
        transaction_2.dependencies = Some(vec![transaction_1.id.clone()]);

        let result = storage.create(&vec![transaction_1, transaction_2]).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_should_fail_create_with_invalid_dependencies() {
        let storage = mock_sqlite_transaction().await;

        let mut transaction = Transaction::default();
        transaction.dependencies = Some(vec!["something".into()]);

        let result = storage.create(&vec![transaction]).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn it_should_find_next() {
        let storage = mock_sqlite_transaction().await;
        let transaction = Transaction::default();

        storage.create(&vec![transaction]).await.unwrap();

        let queues = HashMap::from_iter([("default".into(), 1)]);
        let result = storage.next(TransactionStatus::Pending, queues).await;
        assert!(result.is_ok());
        assert!(!result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_should_find_next_many_queues() {
        let storage = mock_sqlite_transaction().await;

        let data = vec![
            ("1", "banana"),
            ("2", "banana"),
            ("3", "orange"),
            ("4", "default"),
        ];
        let transaction_list: TransactionList = data.into();
        storage.create(&transaction_list.0).await.unwrap();

        let queues = HashMap::from_iter([
            ("banana".into(), 1),
            ("orange".into(), 1),
            ("default".into(), 1),
        ]);
        let result = storage.next(TransactionStatus::Pending, queues).await;
        assert!(result.is_ok());
        assert!(!result.as_ref().unwrap().is_empty());
        assert!(result.unwrap().len() == 3);
    }

    #[tokio::test]
    async fn it_should_update() {
        let storage = mock_sqlite_transaction().await;

        let transaction = Transaction::default();
        storage.create(&vec![transaction]).await.unwrap();

        let mut transaction = Transaction::default();
        transaction.status = TransactionStatus::Validated;
        let result = storage.update(&transaction).await;
        assert!(result.is_ok());

        let queues = HashMap::from_iter([("default".into(), 1)]);
        let result = storage.next(TransactionStatus::Validated, queues).await;
        assert!(result.is_ok());
        assert!(!result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_should_update_batch() {
        let storage = mock_sqlite_transaction().await;

        let mut batch = Vec::new();

        let mut transaction = Transaction::default();
        transaction.id = "hex1".into();
        batch.push(transaction.clone());
        storage.create(&vec![transaction]).await.unwrap();

        let mut transaction = Transaction::default();
        transaction.id = "hex2".into();
        batch.push(transaction.clone());
        storage.create(&vec![transaction]).await.unwrap();

        let batch = batch
            .iter_mut()
            .map(|tx| {
                tx.status = TransactionStatus::Confirmed;
                tx.slot = Some(1);
                tx.clone()
            })
            .collect();
        let result = storage.update_batch(&batch).await;
        assert!(result.is_ok());

        let result = storage.find(TransactionStatus::Confirmed).await;
        assert!(result.is_ok());
        assert!(!result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_should_find() {
        let storage = mock_sqlite_transaction().await;
        let transaction = Transaction::default();

        storage.create(&vec![transaction]).await.unwrap();

        let result = storage.find(TransactionStatus::Pending).await;
        assert!(result.is_ok());
        assert!(result.unwrap().len() == 1);
    }

    #[tokio::test]
    async fn it_should_find_to_rollback() {
        let storage = mock_sqlite_transaction().await;

        let transaction = Transaction::default();
        storage.create(&vec![transaction]).await.unwrap();

        let transaction = Transaction {
            status: TransactionStatus::Confirmed,
            slot: Some(5),
            ..Default::default()
        };
        storage.update(&transaction).await.unwrap();

        let result = storage.find_to_rollback(1).await;
        assert!(result.is_ok());
        assert!(result.unwrap().len() == 1);
    }

    #[tokio::test]
    async fn it_should_return_empty_find_to_rollback_when_tx_slot_lower_than_block_slot() {
        let storage = mock_sqlite_transaction().await;

        let transaction = Transaction::default();
        storage.create(&vec![transaction]).await.unwrap();

        let transaction = Transaction {
            status: TransactionStatus::Confirmed,
            slot: Some(5),
            ..Default::default()
        };
        storage.update(&transaction).await.unwrap();

        let result = storage.find_to_rollback(10).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_should_find_latest() {
        let storage = mock_sqlite_transaction().await;

        let transactions = vec![
            Transaction {
                id: "1".into(),
                created_at: Utc::now().checked_sub_days(Days::new(5)).unwrap(),
                updated_at: Utc::now().checked_sub_days(Days::new(5)).unwrap(),
                ..Default::default()
            },
            Transaction {
                id: "2".into(),
                ..Default::default()
            },
        ];

        storage.create(&transactions).await.unwrap();

        let result = storage.latest(DEFAULT_QUEUE).await;
        assert!(result.is_ok());
        assert!(result.as_ref().unwrap().is_some());
        let transaction = result.unwrap().unwrap();
        assert!(transaction.id == "2".to_string());
    }

    #[tokio::test]
    async fn it_should_return_empty_latest() {
        let storage = mock_sqlite_transaction().await;

        let result = storage.latest(DEFAULT_QUEUE).await;
        assert!(result.is_ok());
        assert!(result.as_ref().unwrap().is_none());
    }
}

#[cfg(test)]
mod sqlite_cursor_tests {

    use crate::storage::{sqlite::sqlite_utils_tests::mock_sqlite_cursor, Cursor};

    #[tokio::test]
    async fn it_should_set() {
        let storage = mock_sqlite_cursor().await;
        let cursor = Cursor::default();

        let result = storage.set(&cursor).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_should_set_when_it_updates() {
        let storage = mock_sqlite_cursor().await;

        let cursor = Cursor::default();
        storage.set(&cursor).await.unwrap();

        let cursor = Cursor {
            slot: 2,
            ..Default::default()
        };
        let result = storage.set(&cursor).await;
        assert!(result.is_ok());

        let result = storage.current().await;
        assert!(result.is_ok());
        assert!(result.as_ref().unwrap().is_some());
        assert!(result.unwrap().unwrap().slot == 2);
    }

    #[tokio::test]
    async fn it_should_find_current() {
        let storage = mock_sqlite_cursor().await;
        let cursor = Cursor::default();
        storage.set(&cursor).await.unwrap();

        let result = storage.current().await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }
}
