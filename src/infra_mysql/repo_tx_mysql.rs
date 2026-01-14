use crate::domain_port::{StorageTx, TxManager};
use anyhow::anyhow;
use sqlx::{MySql, MySqlConnection, MySqlPool, Transaction};

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
        Ok(Box::new(MySqlTx::new(tx)))
    }
}

pub struct MySqlTx<'t> {
    inner: Transaction<'t, MySql>,
}

impl<'t> MySqlTx<'t> {
    pub fn new(inner: Transaction<'t, MySql>) -> Self {
        MySqlTx { inner }
    }

    pub fn conn(&mut self) -> &mut MySqlConnection {
        self.inner.as_mut()
    }
}

#[async_trait::async_trait]
impl<'t> StorageTx<'t> for MySqlTx<'t> {
    async fn commit(self: Box<Self>) -> anyhow::Result<()> {
        self.inner.commit().await.map_err(|e| anyhow!(e))?;
        Ok(())
    }

    async fn rollback(self: Box<Self>) -> anyhow::Result<()> {
        self.inner.rollback().await.map_err(|e| anyhow!(e))?;
        Ok(())
    }
}
