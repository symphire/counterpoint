use crate::application_impl::*;
use crate::application_port::*;
use crate::domain_port::*;
use crate::infra_mysql::*;
use crate::infra_redis::*;
use crate::logger::*;
use crate::server::*;
use crate::settings::Settings;
use nanoid::nanoid;
use sqlx::{MySql, Pool};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub struct Server {
    pub auth_service: Arc<dyn AuthService>,
    pub captcha_service: Arc<dyn CaptchaService>,
    pub user_service: Arc<dyn UserService>,
    pub relationship_service: Arc<dyn RelationshipService>,
    pub conversation_service: Arc<dyn ConversationService>,
    pub connection_acceptor: Arc<dyn ConnectionAcceptor>,
    fanout_handle: Mutex<Option<JoinHandle<()>>>,
    notifier_handle: Mutex<Option<JoinHandle<()>>>,
    cancel: CancellationToken,
    session_hub: Arc<SessionHub>,
    pool: Pool<MySql>,
}

impl Server {
    pub async fn try_new(settings: &Settings) -> anyhow::Result<Self> {
        let alphabet: [char; 16] = [
            '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', 'a', 'b', 'c', 'd', 'e', 'f',
        ];
        let run_id = nanoid!(10, &alphabet);

        const REDIS_DSN: &str = "redis://:mysecret@127.0.0.1:6379";
        let redis_client = redis::Client::open(REDIS_DSN)?;
        let redis_manager = redis_client.get_connection_manager().await?;
        let captcha_store = Arc::new(RedisCaptchaStore::new(
            redis_manager.clone(),
            "captcha".to_string(),
        ));

        const MYSQL_DSN: &str =
            "mysql://counterpoint_app:user_secret_pw@localhost:3306/counterpoint_db";
        let pool = Pool::<MySql>::connect(MYSQL_DSN).await?;
        let tx_manager: Arc<dyn TxManager> = Arc::new(MySqlTxManager::new(pool.clone()));

        let credential_hasher: Arc<dyn CredentialHasher> = Arc::new(Argon2PasswordHasher {});
        let key = std::env::var("JWT_SIGNING_KEY")
            .unwrap_or_else(|_| "my-dev-secret-key".to_string())
            .into_bytes();
        let token_codec: Arc<dyn TokenCodec> = Arc::new(JwtHs256Codec::new(JwtConfig {
            issuer: "serveroxide.auth".to_string(),
            audience: "chat-client".to_string(),
            access_ttl: Duration::from_secs(7 * 24 * 60 * 60), // 1 day
            refresh_ttl: Duration::from_secs(7 * 24 * 60 * 60), // 7 days
            signing_key: key,
        }));

        let session_store: Arc<dyn AuthSessionStore> = Arc::new(RedisAuthSessionStore::new(
            redis_manager.clone(),
            format!("auth:{}", run_id),
        ));

        let auth_repo: Arc<dyn AuthRepo> = Arc::new(MySqlAuthRepo::new(pool.clone()));
        let user_repo: Arc<dyn UserRepo> = Arc::new(MySqlUserRepo::new(pool.clone()));
        let friendship_repo: Arc<dyn FriendshipRepo> =
            Arc::new(MySqlFriendshipRepo::new(pool.clone()));
        let group_repo: Arc<dyn GroupRepo> = Arc::new(MySqlGroupRepo::new(pool.clone()));
        let group_idem_repo: Arc<dyn GroupIdemRepo> =
            Arc::new(MySqlGroupIdemRepo::new(pool.clone()));
        let conversation_repo: Arc<dyn ConversationRepo> =
            Arc::new(MySqlConversationRepo::new(pool.clone()));
        let conversation_role_repo: Arc<dyn ConversationRoleRepo> =
            Arc::new(MySqlConversationRoleRepo::new(pool.clone()));
        let message_repo: Arc<dyn MessageRepo> = Arc::new(MySqlMessageRepo::new(pool.clone()));
        let outbox_repo: Arc<dyn OutboxRepo> = Arc::new(MySqlOutboxRepo::new(pool.clone()));

        let captcha_service: Arc<dyn CaptchaService> = match settings.captcha.backend.as_str() {
            "fake" => Arc::new(FakeCaptchaService::new()),
            "real" => Arc::new(RealCaptchaService::new(
                captcha_store,
                "my-secret-key".into(),
            )),
            other => return Err(anyhow::anyhow!("Unknown captcha backend: {}", other)),
        };

        let auth_service: Arc<dyn AuthService> = match settings.auth.backend.as_str() {
            // "fake" => Arc::new(FakeAuthService::new()),
            "real" => Arc::new(RealAuthService::new(
                auth_repo,
                user_repo.clone(),
                credential_hasher,
                token_codec,
                session_store,
                tx_manager.clone(),
            )),
            other => return Err(anyhow::anyhow!("Unknown auth backend: {}", other)),
        };
        // debug!(?auth_service);

        let user_service = match settings.user.backend.as_str() {
            // "fake" => Arc::new(FakeUserService::new()),
            "real" => Arc::new(RealUserService::new(user_repo.clone(), tx_manager.clone())),
            other => return Err(anyhow::anyhow!("Unknown user backend: {}", other)),
        };
        // debug!(?user_service);

        let relationship_service: Arc<dyn RelationshipService> =
            Arc::new(RealRelationshipService::new(
                user_repo.clone(),
                friendship_repo,
                group_repo,
                group_idem_repo,
                conversation_repo.clone(),
                conversation_role_repo.clone(),
                outbox_repo.clone(),
                tx_manager.clone(),
            ));

        let conversation_service: Arc<dyn ConversationService> =
            Arc::new(RealConversationService::new(
                user_repo.clone(),
                message_repo,
                conversation_repo,
                conversation_role_repo,
                outbox_repo.clone(),
                tx_manager.clone(),
            ));

        // region runtime infra
        let cancel = CancellationToken::new();

        let topic = format!("chat.event.{}", run_id);

        let publisher: Arc<dyn EventPublisher> = Arc::new(KafkaPublisher::new(
            "localhost:9092",
            &format!("chat-pub-{}", run_id),
        )?);
        let consumer: Arc<dyn EventConsumer> = Arc::new(KafkaConsumer::new(
            "localhost:9092",
            &format!("chat-sub-{}", run_id),
            cancel.clone(),
        ));

        let service_registry = Arc::new(ServiceRegistry {
            conversation_service: conversation_service.clone(),
        });
        let session_hub = Arc::new(SessionHub::new(service_registry.clone()));
        let connection_acceptor: Arc<dyn ConnectionAcceptor> = session_hub.clone();
        let outbound_queue: Arc<dyn OutboundQueue> = session_hub.clone();

        let fanout_handler: Arc<dyn EventHandler> =
            Arc::new(ConnFanoutHandler::new(outbound_queue.clone()));
        let notifier = Notifier::new(
            tx_manager.clone(),
            outbox_repo.clone(),
            publisher.clone(),
            &topic,
            cancel.clone(),
        );

        let run_id_clone = run_id.clone();
        let fanout_handle = tokio::spawn(async move {
            let _ = consumer
                .run(
                    &format!("ws-fanout-{}", run_id_clone),
                    &[&topic],
                    fanout_handler,
                )
                .await;
        });
        let notifier_handle = tokio::spawn(async move {
            let _ = notifier.run().await;
        });

        // endregion

        info!("server started");

        Ok(Self {
            auth_service,
            captcha_service,
            user_service,
            relationship_service,
            conversation_service,
            connection_acceptor,
            fanout_handle: Mutex::new(Some(fanout_handle)),
            notifier_handle: Mutex::new(Some(notifier_handle)),
            cancel,
            session_hub,
            pool: pool,
        })
    }

    pub async fn shutdown(&self) {
        info!("server shutting down...");

        self.cancel.cancel();

        if let Ok(mut lock) = self.notifier_handle.lock() {
            if let Some(handle) = lock.take() {
                let r = handle.await;
                info!("notifier handle dropped: {:?}", r);
            }
        }
        if let Ok(mut lock) = self.fanout_handle.lock() {
            if let Some(handle) = lock.take() {
                let r = handle.await;
                info!("fanout handle dropped: {:?}", r);
            }
        }

        self.session_hub.shutdown().await;
        self.pool.close().await;
    }
}
