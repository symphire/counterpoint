/// Example demonstrating how to call the public server interfaces.
///
/// ⚠️ Required for execution:
/// This demo expects a fixed captcha (UUID NIL + code `"123456"`).
/// To enable this, uncomment the following block in `captcha_service_impl.rs`:
///
/// ```
/// let id = domain::CaptchaId(uuid::Uuid::nil());
/// let code = "123456";
/// ```
///
/// This is intended only for manual testing and should not be enabled in production.

use std::sync::Arc;
use std::time::Duration;
use futures_util::future::join_all;
use nanoid::nanoid;
use sqlx::{MySql, Pool};
use tokio::io;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_util::sync::CancellationToken;
use tracing_subscriber::{fmt, EnvFilter};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use counterpoint::domain::{Argon2PasswordHasher, AuthService, CaptchaId, CaptchaService, ConversationId, ConversationService, CredentialHasher, IdempotencyKey, JwtConfig, JwtHs256Codec, LoginInput, LoginResult, MessageId, RealConversationService, PageSize, RealAuthService, RealCaptchaService, RealRelationshipService, RelationshipService, SignupInput, TokenCodec, TxManager, UserId, ValidationInput};
use counterpoint::infra::{AuthRepo, AuthSessionStore, C2SCommand, CaptchaStore, ChatMessageSend, ConversationRepo, ConversationRoleRepo, FriendshipRepo, GroupIdemRepo, GroupRepo, MessageRepo, MySqlConversationRepo, MySqlConversationRoleRepo, MySqlFriendshipRepo, MySqlGroupIdemRepo, MySqlGroupRepo, MySqlMessageRepo, MySqlOutboxRepo, MySqlTxManager, MySqlUserRepo, OutboxRepo, RedisCaptchaStore, RedisSessionStore, S2CEvent, MySqlAuthRepo, UserRepo};
use counterpoint::server::{ConnMessage, ConnReceiver, ConnSender, ConnectionAcceptor, EventConsumer, EventHandler, EventPublisher, OutboundQueue, ServiceRegistry, SessionHub};
use counterpoint::server::KafkaConsumer;
use counterpoint::server::ConnFanoutHandler;
use counterpoint::server::KafkaPublisher;
use counterpoint::server::Notifier;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let filter = EnvFilter::new("infra_demo=debug");

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer())
        .init();


    let alphabet: [char; 16] = [
        '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', 'a', 'b', 'c', 'd', 'e', 'f',
    ];
    let run_id = nanoid!(10, &alphabet);


    // region prepare connection

    const REDIS_DSN: &str = "redis://:mysecret@127.0.0.1:6379";
    let redis_client = redis::Client::open(REDIS_DSN)?;
    let mut redis_manager = redis_client.get_connection_manager().await?;

    let pong: String = redis::cmd("PING").query_async(&mut redis_manager).await?;
    println!("PING -> {}", pong);

    const MYSQL_DSN: &str = "mysql://counterpoint_app:user_secret_pw@localhost:3306/counterpoint_db";
    let pool = Pool::<MySql>::connect(MYSQL_DSN).await?;

    let value: i32 = sqlx::query_scalar("SELECT 1").fetch_one(&pool).await?;
    println!("MySQL -> {}", value);

    // endregion


    // region initialization

    let captcha_store: Arc<dyn CaptchaStore> = Arc::new(RedisCaptchaStore::new(
        redis_manager.clone(),
        "captcha".to_string(),
    ));
    let captcha_service: Arc<dyn CaptchaService> = Arc::new(RealCaptchaService::new(
        captcha_store,
        "my-secret-key".into(),
    ));

    let credential_hasher: Arc<dyn CredentialHasher> = Arc::new(Argon2PasswordHasher {});
    let key = std::env::var("JWT_SIGNING_KEY")
        .unwrap_or_else(|_| "my-dev-secret-key".to_string())
        .into_bytes();
    let token_codec: Arc<dyn TokenCodec> = Arc::new(JwtHs256Codec::new(JwtConfig {
        issuer: "serveroxide.auth".to_string(),
        audience: "chat-client".to_string(),
        access_ttl: Duration::from_secs(15 * 60), // 15 minutes
        refresh_ttl: Duration::from_secs(7 * 24 * 60 * 60), // 7 days
        signing_key: key,
    }));

    let session_store: Arc<dyn AuthSessionStore> = Arc::new(RedisSessionStore::new(
        redis_manager.clone(),
        format!("auth:{}", run_id),
    ));


    let tx_manager: Arc<dyn TxManager> = Arc::new(MySqlTxManager::new(pool.clone()));

    let auth_repo: Arc<dyn AuthRepo> = Arc::new(MySqlAuthRepo::new(pool.clone()));
    let user_repo: Arc<dyn UserRepo> = Arc::new(MySqlUserRepo::new(pool.clone()));
    let friendship_repo: Arc<dyn FriendshipRepo> = Arc::new(MySqlFriendshipRepo::new(pool.clone()));
    let group_repo: Arc<dyn GroupRepo> = Arc::new(MySqlGroupRepo::new(pool.clone()));
    let group_idem_repo: Arc<dyn GroupIdemRepo> = Arc::new(MySqlGroupIdemRepo::new(pool.clone()));
    let conversation_repo: Arc<dyn ConversationRepo> =
        Arc::new(MySqlConversationRepo::new(pool.clone()));
    let conversation_role_repo: Arc<dyn ConversationRoleRepo> =
        Arc::new(MySqlConversationRoleRepo::new(pool.clone()));
    let message_repo: Arc<dyn MessageRepo> = Arc::new(MySqlMessageRepo::new(pool.clone()));
    let outbox_repo: Arc<dyn OutboxRepo> = Arc::new(MySqlOutboxRepo::new(pool.clone()));

    let auth_service: Arc<dyn AuthService> = Arc::new(RealAuthService::new(
        auth_repo,
        user_repo.clone(),
        credential_hasher,
        token_codec,
        session_store,
        tx_manager.clone(),
    ));
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

    let fanout_handler: Arc<dyn EventHandler> = Arc::new(ConnFanoutHandler::new(outbound_queue.clone()));
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
            .run(&format!("ws-fanout-{}", run_id_clone), &[&topic], fanout_handler)
            .await;
    });
    let notifier_handle = tokio::spawn(async move {
        let _ = notifier.run().await;
    });

    // endregion


    // use cases

    captcha_service.generate().await?;
    captcha_service
        .validate(ValidationInput {
            id: CaptchaId(uuid::Uuid::nil()),
            answer: "123456".to_string(),
        })
        .await?;

    const USERNAME_PREFIX: &str = "testuser";
    const PASSWORD: &str = "testpass";
    let mut users: Vec<(UserId, LoginResult)> = Vec::new();

    for i in 0..3 {
        let id = auth_service
            .signup(SignupInput {
                username: format!("{}{}_{}", USERNAME_PREFIX, i, run_id),
                password: PASSWORD.to_string(),
            })
            .await?;
        tracing::debug!("user_id: {}", id);

        let result = auth_service
            .login(LoginInput {
                username: format!("{}{}_{}", USERNAME_PREFIX, i, run_id),
                password: PASSWORD.to_string(),
            })
            .await?;
        tracing::debug!("login_result: {:?}", result);

        users.push((id, result));
    }

    let verify_result = auth_service
        .verify_token(users[0].1.tokens.access_token.0.as_str())
        .await?;
    tracing::debug!("verify_result: {}", verify_result);

    let refresh_result = auth_service
        .refresh_token(users[0].1.tokens.refresh_token.0.as_str())
        .await?;
    tracing::debug!("refresh_result: {:?}", refresh_result);

    let mut c2s = Vec::new();
    let mut handles = Vec::new();
    for i in 0..3 {
        let (c2s_tx, c2s_rx) = tokio::sync::mpsc::channel::<ConnMessage>(256);
        let (s2c_tx, mut s2c_rx) = tokio::sync::mpsc::channel::<ConnMessage>(256);
        let c2s_channel: Box<dyn ConnReceiver> = Box::new(c2s_rx);
        let s2c_channel: Box<dyn ConnSender> = Box::new(s2c_tx);
        connection_acceptor.accept_connection(s2c_channel, c2s_channel, users[i].1.user_id).await?;
        c2s.push(c2s_tx.clone());
        let handle = tokio::spawn(async move {
            while let Some(message) = s2c_rx.recv().await {
                tracing::info!("received from ws server ({}): {:?}", i, message);
            }
        });
        handles.push(handle);
    }

    let mut conversations: Vec<ConversationId> = Vec::new();

    let conv = relationship_service
        .add_friend(
            users[0].1.user_id,
            users[1].1.user_id,
            IdempotencyKey(uuid::Uuid::new_v4()),
        )
        .await?;
    conversations.push(conv);
    let conv = relationship_service
        .add_friend(
            users[0].1.user_id,
            users[2].1.user_id,
            IdempotencyKey(uuid::Uuid::new_v4()),
        )
        .await?;
    conversations.push(conv);
    let conv = relationship_service
        .add_friend(
            users[1].1.user_id,
            users[2].1.user_id,
            IdempotencyKey(uuid::Uuid::new_v4()),
        )
        .await?;
    conversations.push(conv);

    let friends = relationship_service
        .list_friends(users[0].1.user_id, PageSize(10), None)
        .await?;
    tracing::debug!("friends of testuser0: {:?}", friends);

    let (gid, cid) = relationship_service
        .create_group(
            users[0].1.user_id,
            &format!("group012_{}", run_id),
            None,
            IdempotencyKey(uuid::Uuid::new_v4()),
        )
        .await?;
    conversations.push(cid);

    relationship_service
        .invite_to_group(gid, users[0].1.user_id, users[1].1.user_id)
        .await?;
    relationship_service
        .invite_to_group(gid, users[0].1.user_id, users[2].1.user_id)
        .await?;

    let groups = relationship_service
        .list_groups(users[0].1.user_id, PageSize(10), None)
        .await?;
    tracing::debug!("groups of testuser0: {:?}", groups);

    let members = relationship_service
        .list_group_members(users[0].1.user_id, gid, PageSize(10), None)
        .await?;
    tracing::debug!("members of group: {:?}", members);

    for i in [0, 1, 3] {  // 0-1, 0-2, 0-1-2
        let command = C2SCommand::ChatMessageSend(ChatMessageSend {
            conversation_id: conversations[i],
            message_id: MessageId(uuid::Uuid::new_v4()),
            content: format!("hello from testuser0 ({run_id})"),
        });
        let s = serde_json::to_string(&command)?;
        c2s[0].send(ConnMessage::Text(s)).await?;
    }

    let mut reader = BufReader::new(io::stdin()).lines();
    println!(r#"
    **********************************************************************
    ** Press Enter to logout clients, show history and exit...
    **********************************************************************
    "#);
    let _ = reader.next_line().await;

    for tx in c2s {
        drop(tx);
    }

    join_all(handles).await;
    tracing::info!("All client tasks finished.");

    for (i, j) in [(0, 0), (2, 1), (1, 3)] {  // 0-1, 0-2, 0-1-2
        let history = conversation_service
            .get_history(users[i].1.user_id, conversations[j], PageSize(10), None)
            .await?;
        tracing::debug!("history ({:?}, {:?}): {:?}", users[i].1.user_id, conversations[i], history);
    }

    let recent = conversation_service
        .recent_conversations(users[0].1.user_id, PageSize(10), None)
        .await?;
    tracing::debug!("recent conversations: {:?}", recent);


    Ok(())
}
