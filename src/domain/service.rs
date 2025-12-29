use crate::infra::CaptchaStoreError;
use chrono::{DateTime, Utc};
use std::fmt;
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use crate::domain::{ConversationId, UserId};
// region captcha service

#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct CaptchaId(pub uuid::Uuid);

impl fmt::Display for CaptchaId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug)]
pub struct CaptchaResult {
    pub id: CaptchaId,
    pub image_base64: String,
    pub expire_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct ValidationInput {
    pub id: CaptchaId,
    pub answer: String,
}

#[derive(Debug, thiserror::Error)]
pub enum CaptchaError {
    #[error("incorrect code, {remaining_attempts} attempt(s) left")]
    Incorrect { remaining_attempts: u32 },
    #[error("Captcha not found or expired")]
    NotFoundOrExpired,
    #[error("infra error: {0}")]
    Store(String),
    #[error("Internal error: {0}")]
    InternalError(#[from] anyhow::Error),
}

impl From<CaptchaStoreError> for CaptchaError {
    fn from(err: CaptchaStoreError) -> Self {
        match err {
            CaptchaStoreError::Incorrect {
                remaining_attempts: retry,
            } => CaptchaError::Incorrect {
                remaining_attempts: retry,
            },
            CaptchaStoreError::NotFoundOrExpired => CaptchaError::NotFoundOrExpired,
            CaptchaStoreError::Store(e) => CaptchaError::Store(e),
            CaptchaStoreError::InternalError(e) => CaptchaError::InternalError(e),
        }
    }
}

#[async_trait::async_trait]
pub trait CaptchaService: Send + Sync {
    async fn generate(&self) -> Result<CaptchaResult, CaptchaError>;
    async fn validate(&self, input: ValidationInput) -> Result<(), CaptchaError>;
}

// endregion


// region auth service

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("user already exists")]
    UserExists,
    #[error("user not found")]
    UserNotFound,
    #[error("token invalid")]
    TokenInvalid,
    #[error("token expired")]
    TokenExpired,
    #[error("captcha error: {0}")]
    Captcha(String),
    #[error("store error: {0}")]
    Store(String),
    #[error("internal error: {0}")]
    InternalError(String),
}

#[derive(Debug, Clone)]
pub struct SignupInput {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone)]
pub struct LoginInput {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone)]
pub struct LoginResult {
    pub user_id: UserId,
    pub tokens: AuthTokens,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccessToken(pub String);
#[derive(Debug, Clone, Serialize)]
pub struct RefreshToken(pub String);

#[derive(Debug, Clone, Serialize)]
pub struct AuthTokens {
    pub access_token: AccessToken,
    pub refresh_token: RefreshToken,
    pub access_token_expires_at: DateTime<Utc>,
    pub refresh_token_expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TokenVerifyResult {
    pub user_id: UserId,
    pub jti: Option<String>,
}

#[async_trait::async_trait]
pub trait TokenCodec: Send + Sync {
    async fn issue_access_token(
        &self,
        user: UserId,
        jti: Option<String>,
    ) -> Result<(AccessToken, DateTime<Utc>), AuthError>;
    async fn issue_refresh_token(
        &self,
        user: UserId,
        jti: String,
    ) -> Result<(RefreshToken, DateTime<Utc>), AuthError>;
    async fn verify_access_token(
        &self,
        token: &AccessToken,
    ) -> Result<TokenVerifyResult, AuthError>;
    async fn verify_refresh_token(
        &self,
        token: &RefreshToken,
    ) -> Result<TokenVerifyResult, AuthError>;
}

#[async_trait::async_trait]
pub trait CredentialHasher: Send + Sync {
    async fn hash_password(&self, password: &str) -> Result<String, AuthError>;
    async fn verify_password(&self, password: &str, password_hash: &str)
                             -> Result<bool, AuthError>;
}

#[async_trait::async_trait]
pub trait AuthService: Send + Sync {
    async fn signup(&self, request: SignupInput) -> Result<UserId, AuthError>;
    async fn login(&self, request: LoginInput) -> Result<LoginResult, AuthError>;
    async fn verify_token(&self, token: &str) -> Result<UserId, AuthError>;
    async fn refresh_token(&self, refresh_token: &str) -> Result<AuthTokens, AuthError>;
}

// endregion


// region user service

#[async_trait::async_trait]
pub trait UserService: Send + Sync {
    async fn resolve_username(&self, username: &str) -> Result<UserId, AuthError>;
}

// endregion


// region relationship service
#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct GroupId(pub uuid::Uuid);

impl fmt::Display for GroupId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct FriendCursor {
    pub since: DateTime<Utc>,
    pub other_user: UserId, // tiebreaker
}

impl FromStr for FriendCursor {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (date_str, user_str) = s
            .split_once('~')
            .ok_or("invalid cursor format")?;

        let since = date_str
            .parse::<DateTime<Utc>>()
            .map_err(|e| e.to_string())?;

        let other_user = uuid::Uuid::parse_str(user_str)
            .map(UserId)
            .map_err(|e| e.to_string())?;

        Ok(FriendCursor {since, other_user})
    }
}

#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct GroupCursor {
    pub created_at: DateTime<Utc>,
    pub group_id: GroupId, // tiebreaker
}

pub struct MemberCursor {
    pub joined_at: DateTime<Utc>,
    pub user: UserId, // tiebreaker
}

#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct PageSize(pub u16);

#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct IdempotencyKey(pub uuid::Uuid);

#[derive(Debug, Clone, Serialize)]
pub struct FriendSummary {
    pub user_id: UserId,
    pub username: String,
    pub conversation_id: ConversationId,
    pub since: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum GroupMemberRole {
    Owner,
    Member,
}

#[derive(Debug, Clone)]
pub struct GroupSummary {
    pub group_id: GroupId,
    pub name: String,
    pub my_role: GroupMemberRole,  // smell hint: this field seems redundant
    pub conversation_id: ConversationId,
    pub member_count: u32,  // smell hint: this field seems redundant
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct MemberSummary {
    pub user_id: UserId,
    pub username: String,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum RelationError {
    #[error("user not found")]
    UserNotFound,
    #[error("friend request already exists")]
    FriendRequestExists,
    #[error("friendship already established")]
    AlreadyFriends,
    #[error("group not found")]
    GroupNotFound,
    #[error("already a member")]
    AlreadyMember,
    #[error("not a member")]
    NotMember,
    #[error("not an owner")]
    NotOwner,
    #[error("role not found: {0}")]
    RoleNotFound(String),
    #[error("store error: {0}")]
    Store(String),
}

#[async_trait::async_trait]
pub trait RelationshipService: Send + Sync {
    async fn add_friend(
        &self,
        me: UserId,
        other: UserId,
        _idempotency_key: IdempotencyKey,
    ) -> Result<ConversationId, RelationError>;
    async fn list_friends(
        &self,
        user_id: UserId,
        page_size: PageSize,
        after: Option<FriendCursor>,
    ) -> Result<Vec<FriendSummary>, RelationError>;
    async fn create_group(
        &self,
        owner: UserId,
        name: &str,
        description: Option<&str>,
        idempotency_key: IdempotencyKey,
    ) -> Result<(GroupId, ConversationId), RelationError>;
    async fn invite_to_group(&self, group: GroupId, host: UserId, guest: UserId) -> Result<(), RelationError>;
    async fn list_groups(
        &self,
        user_id: UserId,
        page_size: PageSize,
        after: Option<GroupCursor>,
    ) -> Result<Vec<GroupSummary>, RelationError>;
    async fn list_group_members(
        &self,
        user_id: UserId,
        group: GroupId,
        page_size: PageSize,
        after: Option<MemberCursor>,
    ) -> Result<Vec<MemberSummary>, RelationError>;
}

// endregion


// region conversation service

#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct MessageId(pub uuid::Uuid);

#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct MessageOffset(pub u64);

impl FromStr for MessageOffset {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let offset = s.parse::<u64>().map_err(|e| e.to_string())?;
        Ok(Self(offset))
    }
}

/// Cursor for time-ordered lists (recent convos)
#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct TimeCursor {
    pub last_msg_at: DateTime<Utc>,
    pub conversation_id: ConversationId, // tie-breaker for stable pagination
}

/// Cursor for offset-ordered lists (history)
#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct OffsetCursor {
    pub offset: MessageOffset,
}

impl FromStr for OffsetCursor {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let offset = s.parse::<MessageOffset>()
            .map_err(|e| format!("invalid offset: {}", e))?;

        Ok(OffsetCursor {offset})
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageRecord {
    pub message_id: MessageId,
    pub conversation_id: ConversationId,
    pub message_offset: MessageOffset,
    pub sender: UserId,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum ConversationPeer {
    Direct { other_user: UserId, name: String },
    Group { group_id: GroupId, name: String },
}

#[derive(Debug, Clone)]
pub struct RecentConversation {
    pub conversation_id: ConversationId,
    pub peer: ConversationPeer,
    pub last_msg_off: MessageOffset,
    pub last_msg_at: Option<DateTime<Utc>>, // NULL before first message
}

#[derive(Debug, thiserror::Error)]
pub enum ChatError {
    #[error("conversation not found")]
    ConversationNotFound,
    #[error("user not a member of conversation")]
    NotMember,
    #[error("permission denied: {0}")]
    Forbidden(&'static str),
    #[error("idempotency conflict")]
    IdempotentConflict,
    #[error("invalid cursor")]
    BadCursor,
    #[error("conflict: direct conversation already exists")]
    AlreadyExists,
    #[error("store error: {0}")]
    Store(String),
}

#[async_trait::async_trait]
pub trait ConversationService: Send + Sync {
    async fn send_message(
        &self,
        conversation_id: ConversationId,
        sender: UserId,
        content: &str,
        message_id: MessageId,
    ) -> Result<MessageRecord, ChatError>;
    async fn get_history(
        &self,
        user_id: UserId,
        conversation_id: ConversationId,
        page_size: PageSize,
        before: Option<OffsetCursor>,
    ) -> Result<Vec<MessageRecord>, ChatError>;
    async fn recent_conversations(
        &self,
        user_id: UserId,
        page_size: PageSize,
        after: Option<TimeCursor>,
    ) -> Result<Vec<RecentConversation>, ChatError>;
}

// endregion


// region storage utils

#[async_trait::async_trait]
pub trait TxManager: Send + Sync {
    async fn begin<'t>(&'t self) -> anyhow::Result<Box<dyn StorageTx<'t> + 't>>;
}

#[async_trait::async_trait]
pub trait StorageTx<'t>: Send {
    async fn commit(self: Box<Self>) -> anyhow::Result<()>;
    async fn rollback(self: Box<Self>) -> anyhow::Result<()>;
}

// endregion