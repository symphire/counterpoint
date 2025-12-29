use std::sync::Arc;
use crate::domain::{AccessToken, AuthError, AuthService, AuthTokens, CredentialHasher, LoginInput, LoginResult, RefreshToken, SignupInput, TokenCodec, TokenVerifyResult, TxManager, UserId};
use argon2::password_hash::rand_core::OsRng;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use std::time::{Duration};
use chrono::{DateTime, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use jsonwebtoken::errors::ErrorKind;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::infra::{AuthRepo, AuthSessionStore, UserRepo};

pub struct Argon2PasswordHasher;

#[async_trait::async_trait]
impl CredentialHasher for Argon2PasswordHasher {
    async fn hash_password(&self, password: &str) -> Result<String, AuthError> {
        let salt = argon2::password_hash::SaltString::generate(&mut OsRng);
        let argon2 = argon2::Argon2::default();
        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| AuthError::InternalError(e.to_string()))?
            .to_string();
        Ok(hash)
    }

    async fn verify_password(
        &self,
        password: &str,
        password_hash: &str,
    ) -> Result<bool, AuthError> {
        let parsed = PasswordHash::new(password_hash).map_err(|e| {
            AuthError::InternalError(format!("invalid PHC hash: {}", e.to_string()))
        })?;

        match Argon2::default().verify_password(password.as_bytes(), &parsed) {
            Ok(_) => Ok(true),
            Err(argon2::password_hash::Error::Password) => Ok(false),
            Err(e) => Err(AuthError::InternalError(format!(
                "verify error: {}",
                e.to_string()
            ))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct JwtConfig {
    pub issuer: String,
    pub audience: String,
    pub access_ttl: Duration,
    pub refresh_ttl: Duration,
    pub signing_key: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AccessClaims {
    sub: String, // user id as string
    exp: i64,
    iat: i64,
    iss: String,
    aud: String,
    jti: String, // optional: can be used for blacklist
}

#[derive(Debug, Serialize, Deserialize)]
struct RefreshClaims {
    sub: String, // user id as string
    exp: i64,
    iat: i64,    // optional
    iss: String, // optional
    aud: String, // optional
    jti: String, // required: store this in Redis
}

fn encode_access(
    uid: UserId,
    jti: String,
    cfg: &JwtConfig,
) -> Result<(String, DateTime<Utc>), AuthError> {
    let iat_dt = Utc::now();
    let exp_dt = iat_dt + cfg.access_ttl;
    let claims = AccessClaims {
        sub: uid.0.to_string(),
        exp: exp_dt.timestamp(),
        iat: iat_dt.timestamp(),
        iss: cfg.issuer.clone(),
        aud: cfg.audience.clone(),
        jti,
    };
    let token = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(&cfg.signing_key),
    )
        .map_err(|e| AuthError::InternalError(e.to_string()))?;
    Ok((token, exp_dt))
}

fn encode_refresh(
    uid: UserId,
    jti: String,
    cfg: &JwtConfig,
) -> Result<(String, DateTime<Utc>), AuthError> {
    let iat_dt = Utc::now();
    let exp_dt = iat_dt + cfg.refresh_ttl;
    let claims = RefreshClaims {
        sub: uid.0.to_string(),
        exp: exp_dt.timestamp(),
        iat: iat_dt.timestamp(),
        iss: cfg.issuer.clone(),
        aud: cfg.audience.clone(),
        jti,
    };
    let token = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(&cfg.signing_key),
    )
        .map_err(|e| AuthError::InternalError(e.to_string()))?;
    Ok((token, exp_dt))
}

fn decode_access(token: &str, cfg: &JwtConfig) -> Result<AccessClaims, AuthError> {
    let mut v = Validation::new(Algorithm::HS256);
    v.validate_exp = true;
    v.set_audience(&[cfg.audience.clone()]);
    v.set_issuer(&[cfg.issuer.clone()]);
    let data = decode::<AccessClaims>(token, &DecodingKey::from_secret(&cfg.signing_key), &v)
        .map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::TokenExpired,
            _ => AuthError::TokenInvalid,
        })?;
    Ok(data.claims)
}

fn decode_refresh(token: &str, cfg: &JwtConfig) -> Result<RefreshClaims, AuthError> {
    let mut v = Validation::new(Algorithm::HS256);
    v.validate_exp = true;
    v.set_audience(&[cfg.audience.clone()]);
    v.set_issuer(&[cfg.issuer.clone()]);
    let data = decode::<RefreshClaims>(token, &DecodingKey::from_secret(&cfg.signing_key), &v)
        .map_err(|e| match e.kind() {
            ErrorKind::ExpiredSignature => AuthError::TokenExpired,
            _ => AuthError::TokenInvalid,
        })?;
    Ok(data.claims)
}

pub struct JwtHs256Codec {
    cfg: JwtConfig,
}

impl JwtHs256Codec {
    pub fn new(cfg: JwtConfig) -> Self {
        JwtHs256Codec { cfg }
    }

    #[inline]
    fn gen_jti() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    #[inline]
    fn parse_user_id(sub: &str) -> Result<UserId, AuthError> {
        let id = sub.parse::<UserId>().map_err(|_| AuthError::TokenInvalid)?;
        Ok(id)
    }
}

#[async_trait::async_trait]
impl TokenCodec for JwtHs256Codec {
    async fn issue_access_token(
        &self,
        user: UserId,
        jti: Option<String>,
    ) -> Result<(AccessToken, DateTime<Utc>), AuthError> {
        let jti = jti.unwrap_or_else(Self::gen_jti);
        let (token, exp_dt) = encode_access(user, jti, &self.cfg)?;
        Ok((AccessToken(token), exp_dt))
    }

    async fn issue_refresh_token(
        &self,
        user: UserId,
        jti: String,
    ) -> Result<(RefreshToken, DateTime<Utc>), AuthError> {
        let (token, exp_dt) = encode_refresh(user, jti, &self.cfg)?;
        Ok((RefreshToken(token), exp_dt))
    }

    async fn verify_access_token(
        &self,
        token: &AccessToken,
    ) -> Result<TokenVerifyResult, AuthError> {
        let claims = decode_access(&token.0, &self.cfg)?;
        let user_id = Self::parse_user_id(&claims.sub)?;
        Ok(TokenVerifyResult {
            user_id,
            jti: Some(claims.jti),
        })
    }

    async fn verify_refresh_token(
        &self,
        token: &RefreshToken,
    ) -> Result<TokenVerifyResult, AuthError> {
        let claims = decode_refresh(&token.0, &self.cfg)?;
        let user_id = Self::parse_user_id(&claims.sub)?;
        Ok(TokenVerifyResult {
            user_id,
            jti: Some(claims.jti),
        })
    }
}

pub struct RealAuthService {
    auth_repo: Arc<dyn AuthRepo>,
    user_repo: Arc<dyn UserRepo>,
    credential_hasher: Arc<dyn CredentialHasher>,
    token_codec: Arc<dyn TokenCodec>,
    session_store: Arc<dyn AuthSessionStore>,
    tx_manager: Arc<dyn TxManager>,
    min_username_len: usize,
    min_password_len: usize,
}

impl RealAuthService {
    pub fn new(
        auth_repo: Arc<dyn AuthRepo>,
        user_repo: Arc<dyn UserRepo>,
        credential_hasher: Arc<dyn CredentialHasher>,
        token_codec: Arc<dyn TokenCodec>,
        session_store: Arc<dyn AuthSessionStore>,
        tx_manager: Arc<dyn TxManager>,
    ) -> Self {
        Self {
            auth_repo,
            user_repo,
            credential_hasher,
            token_codec,
            session_store,
            tx_manager,
            min_username_len: 6,
            min_password_len: 6,
        }
    }

    fn validate_signup(&self, username: &str, password: &str) -> Result<(), AuthError> {
        if username.len() < self.min_username_len {
            return Err(AuthError::InternalError("username too short".to_string()));
        }
        if password.len() < self.min_password_len {
            return Err(AuthError::InternalError("password too short".to_string()));
        }
        Ok(())
    }

    #[inline]
    fn new_user_id() -> UserId {
        UserId(Uuid::new_v4())
    }

    #[inline]
    fn new_jti() -> String {
        Uuid::new_v4().to_string()
    }

    fn ttl_secs(until: DateTime<Utc>) -> u64 {
        let now = Utc::now();
        let secs = (until - now).num_seconds();
        if secs <= 0 {
            1
        } else {
            secs as u64
        }
    }
}

#[async_trait::async_trait]
impl AuthService for RealAuthService {
    async fn signup(&self, request: SignupInput) -> std::result::Result<UserId, AuthError> {
        let SignupInput { username, password } = request;

        self.validate_signup(&username, &password)?;

        if self.user_repo.username_exists(&username).await? {
            return Err(AuthError::UserExists);
        }

        let mut tx = self
            .tx_manager
            .begin()
            .await
            .map_err(|e| AuthError::Store(e.to_string()))?;
        let user_id = Self::new_user_id();

        self.user_repo
            .create_in_tx(tx.as_mut(), user_id, &username)
            .await?;

        let password_hash = self.credential_hasher.hash_password(&password).await?;
        self.auth_repo
            .create_credentials_in_tx(tx.as_mut(), user_id, &username, &password_hash)
            .await?;

        tx.commit()
            .await
            .map_err(|e| AuthError::Store(e.to_string()))?;

        Ok(user_id)
    }

    async fn login(&self, request: LoginInput) -> std::result::Result<LoginResult, AuthError> {
        let LoginInput { username, password } = request;

        let rec = self
            .auth_repo
            .get_by_username(&username)
            .await?
            .ok_or(AuthError::InvalidCredentials)?;

        if !rec.is_active {
            return Err(AuthError::InvalidCredentials);
        }

        let ok = self
            .credential_hasher
            .verify_password(&password, &rec.password_hash)
            .await?;
        if !ok {
            return Err(AuthError::InvalidCredentials);
        }

        let jti = Self::new_jti();

        let (access_token, access_exp) = self
            .token_codec
            .issue_access_token(rec.user_id, Some(jti.clone()))
            .await?;

        let (refresh_token, refresh_exp) = self
            .token_codec
            .issue_refresh_token(rec.user_id, jti.clone())
            .await?;

        let ttl_secs = Self::ttl_secs(refresh_exp);
        self.session_store
            .save_refresh_jti(rec.user_id, &jti, ttl_secs)
            .await?;

        Ok(LoginResult {
            user_id: rec.user_id,
            tokens: AuthTokens {
                access_token,
                refresh_token,
                access_token_expires_at: access_exp,
                refresh_token_expires_at: refresh_exp,
            },
        })
    }

    async fn verify_token(&self, token: &str) -> std::result::Result<UserId, AuthError> {
        let verify_result = self
            .token_codec
            .verify_access_token(&AccessToken(token.to_string()))
            .await?;

        if !self.user_repo.id_exists(verify_result.user_id).await? {
            return Err(AuthError::UserNotFound);
        }

        Ok(verify_result.user_id)
    }

    async fn refresh_token(
        &self,
        refresh_token: &str,
    ) -> std::result::Result<AuthTokens, AuthError> {
        let verify_result = self
            .token_codec
            .verify_refresh_token(&RefreshToken(refresh_token.to_string()))
            .await?;

        if !self.user_repo.id_exists(verify_result.user_id).await? {
            return Err(AuthError::UserNotFound);
        }

        let user_id = verify_result.user_id;
        let jti = verify_result.jti.ok_or(AuthError::TokenInvalid)?;

        // Rotation: check-and-consume
        match self
            .session_store
            .check_refresh_jti(user_id, &jti, true)
            .await?
        {
            Some(found_user_id) => if found_user_id == user_id { /* proceed */ },
            _ => return Err(AuthError::TokenInvalid),
        }

        // Issue new JTI + tokens
        let new_jti = Self::new_jti();

        let (access_token, access_exp) = self
            .token_codec
            .issue_access_token(user_id, Some(new_jti.clone()))
            .await?;
        let (refresh_token, refresh_exp) = self
            .token_codec
            .issue_refresh_token(user_id, new_jti.clone())
            .await?;

        let ttl_secs = Self::ttl_secs(refresh_exp);
        self.session_store
            .save_refresh_jti(user_id, &jti, ttl_secs)
            .await?;

        Ok(AuthTokens {
            access_token,
            refresh_token,
            access_token_expires_at: access_exp,
            refresh_token_expires_at: refresh_exp,
        })
    }
}