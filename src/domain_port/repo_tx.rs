#[async_trait::async_trait]
pub trait TxManager: Send + Sync {
    async fn begin<'t>(&'t self) -> anyhow::Result<Box<dyn StorageTx<'t> + 't>>;
}

#[async_trait::async_trait]
pub trait StorageTx<'t>: Send {
    async fn commit(self: Box<Self>) -> anyhow::Result<()>;
    async fn rollback(self: Box<Self>) -> anyhow::Result<()>;
}
