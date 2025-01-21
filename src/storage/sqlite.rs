use anyhow::Result;
use sqlx::{sqlite::SqliteRow, FromRow, Row};
use std::{path::Path, sync::Arc};

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
            tx_cbor: row.try_get("tx_cbor")?,
            status: row.try_get("status")?,
            priority: row.try_get("priority")?,
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

    async fn create(&self, tx: &TransactionStorage) -> Result<()> {
        sqlx::query!(
            r#"
                INSERT INTO tx (
                    tx_cbor,
                    status,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4)
            "#,
            tx.tx_cbor,
            tx.status,
            tx.created_at,
            tx.updated_at
        )
        .execute(&self.sqlite.db)
        .await?;

        Ok(())
    }
}
