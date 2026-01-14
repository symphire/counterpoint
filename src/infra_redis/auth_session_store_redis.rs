use crate::application_port::*;
use crate::domain_model::*;
use crate::domain_port::*;
use redis::aio::ConnectionManager;
use redis::{
    AsyncCommands, FromRedisValue, RedisError, RedisResult, RedisWrite, ToRedisArgs, Value,
};

pub struct RedisAuthSessionStore {
    conn: ConnectionManager,
    prefix: String,
}

impl RedisAuthSessionStore {
    pub fn new(conn: redis::aio::ConnectionManager, prefix: impl Into<String>) -> Self {
        RedisAuthSessionStore {
            conn,
            prefix: prefix.into(),
        }
    }

    fn key(&self, jti: &str) -> String {
        format!("{}:{}", self.prefix, jti)
    }
}

impl ToRedisArgs for UserId {
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + RedisWrite,
    {
        out.write_arg(self.to_string().as_bytes())
    }
}

impl FromRedisValue for UserId {
    fn from_redis_value(v: &Value) -> RedisResult<Self> {
        let s: String = redis::from_redis_value(v)?;
        let user_id = s.parse::<UserId>().map_err(|e| {
            RedisError::from((
                redis::ErrorKind::TypeError,
                "invalid UserId string",
                e.to_string(),
            ))
        })?;
        Ok(user_id)
    }
}

#[async_trait::async_trait]
impl AuthSessionStore for RedisAuthSessionStore {
    async fn save_refresh_jti(
        &self,
        user_id: UserId,
        jti: &str,
        ttl_secs: u64,
    ) -> Result<(), AuthError> {
        let key = self.key(&jti);
        let mut conn = self.conn.clone();
        let _: () = conn
            .set_ex(&key, &user_id, ttl_secs)
            .await
            .map_err(|e| AuthError::Store(e.to_string()))?;
        Ok(())
    }

    async fn check_refresh_jti(
        &self,
        _user_id: UserId,
        jti: &str,
        consume: bool,
    ) -> Result<Option<UserId>, AuthError> {
        let key = self.key(&jti);
        let mut conn = self.conn.clone();
        let val: Option<UserId> = conn
            .get(&key)
            .await
            .map_err(|e| AuthError::Store(e.to_string()))?;
        if let Some(user_id) = val {
            if consume {
                let _: () = conn
                    .del(&key)
                    .await
                    .map_err(|e| AuthError::Store(e.to_string()))?;
            }
            Ok(Some(user_id))
        } else {
            Ok(None)
        }
    }
}
