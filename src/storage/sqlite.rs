use std::{path::Path, sync::Arc};

use anyhow::Result;
use sqlx::{sqlite::SqliteRow, FromRow, Row};

use super::TransactionStorage;

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

impl FromRow<'_, SqliteRow> for TransactionStorage {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            raw: row.try_get("raw")?,
            status: row.try_get("status")?,
            priority: row.try_get("priority")?,
            dependences: None,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
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

    async fn create(&self, txs: &Vec<TransactionStorage>) -> Result<()> {
        let mut db_tx = self.sqlite.db.begin().await?;

        for tx in txs {
            sqlx::query!(
                r#"
                    INSERT INTO tx (
                        id,
                        raw,
                        status,
                        priority,
                        created_at,
                        updated_at
                    )
                    VALUES ($1, $2, $3, $4, $5, $6)
                "#,
                tx.id,
                tx.raw,
                tx.status,
                tx.priority,
                tx.created_at,
                tx.updated_at
            )
            .execute(&mut *db_tx)
            .await?;

            if let Some(dependences) = &tx.dependences {
                for required_id in dependences {
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
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::storage::TransactionStorage;

    use super::{SqliteStorage, SqliteTransaction};

    async fn mock_sqlite() -> SqliteTransaction {
        let sqlite_storage = Arc::new(SqliteStorage::ephemeral().await.unwrap());
        SqliteTransaction::new(sqlite_storage)
    }

    #[tokio::test]
    async fn it_should_create_tx() {
        let storage = mock_sqlite().await;
        let transaction = TransactionStorage::default();

        let result = storage.create(&vec![transaction]).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_should_create_tx_with_dependence() {
        let storage = mock_sqlite().await;
        let mut transaction_1 = TransactionStorage::default();
        transaction_1.id = "hex1".into();

        let mut transaction_2 = TransactionStorage::default();
        transaction_2.id = "hex2".into();
        transaction_2.dependences = Some(vec![transaction_1.id.clone()]);

        let result = storage.create(&vec![transaction_1, transaction_2]).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_should_fail_create_tx_with_invalid_dependence() {
        let storage = mock_sqlite().await;

        let mut transaction = TransactionStorage::default();
        transaction.dependences = Some(vec!["something".into()]);

        let result = storage.create(&vec![transaction]).await;
        assert!(result.is_err());
    }
}
