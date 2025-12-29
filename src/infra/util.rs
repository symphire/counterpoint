use anyhow::anyhow;
use sqlx::{MySql, MySqlConnection, MySqlPool, Transaction};
use sqlx::mysql::MySqlDatabaseError;
use crate::domain::{StorageTx, TxManager};

pub struct MySqlTxManager {
    pool: MySqlPool,
}

impl MySqlTxManager {
    pub fn new(pool: MySqlPool) -> Self {
        MySqlTxManager { pool }
    }
}

#[async_trait::async_trait]
impl TxManager for MySqlTxManager {
    async fn begin<'t>(&'t self) -> anyhow::Result<Box<dyn StorageTx<'t> + 't>> {
        let tx = self.pool.begin().await.map_err(|e| anyhow!(e))?;
        Ok(Box::new(SqlxTx::new(tx)))
    }
}

pub struct SqlxTx<'t> {
    inner: Transaction<'t, MySql>,
}

impl<'t> SqlxTx<'t> {
    pub fn new(inner: Transaction<'t, MySql>) -> Self {
        SqlxTx { inner }
    }

    pub fn conn(&mut self) -> &mut MySqlConnection {
        self.inner.as_mut()
    }
}

#[async_trait::async_trait]
impl<'t> StorageTx<'t> for SqlxTx<'t> {
    async fn commit(self: Box<Self>) -> anyhow::Result<()> {
        self.inner.commit().await.map_err(|e| anyhow!(e))?;
        Ok(())
    }

    async fn rollback(self: Box<Self>) -> anyhow::Result<()> {
        self.inner.rollback().await.map_err(|e| anyhow!(e))?;
        Ok(())
    }
}

pub fn downcast<'a, 't>(tx: &'a mut dyn StorageTx<'t>) -> &'a mut SqlxTx<'t> {
    unsafe {
        let p = tx as *mut dyn StorageTx<'t>;
        let p = p as *mut SqlxTx<'t>;
        &mut *p
    }
}

pub fn is_dup_key(err: &sqlx::Error) -> bool {
    if let sqlx::Error::Database(db) = err {
        if let Some(mysql_err) = db.try_downcast_ref::<MySqlDatabaseError>() {
            return mysql_err.number() == 1062; // ER_DUP_ENTRY
        }
    }

    false
}