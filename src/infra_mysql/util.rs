use super::repo_tx_mysql::MySqlTx;
use crate::domain_port::*;
use sqlx::mysql::MySqlDatabaseError;

pub fn downcast<'a, 't>(tx: &'a mut dyn StorageTx<'t>) -> &'a mut MySqlTx<'t> {
    unsafe {
        let p = tx as *mut dyn StorageTx<'t>;
        let p = p as *mut MySqlTx<'t>;
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
