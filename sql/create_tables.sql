CREATE DATABASE IF NOT EXISTS counterpoint_db;

USE counterpoint_db;

CREATE TABLE IF NOT EXISTS user
(
    user_id    BINARY(16)   NOT NULL, # UUID
    username   VARCHAR(32)  NOT NULL, # only lower case letters, 0-9 and '_' is supported
    is_active  BOOLEAN      NOT NULL DEFAULT TRUE,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),

    CONSTRAINT pk_user PRIMARY KEY (user_id)
    ) ENGINE = InnoDB
    DEFAULT CHARSET = utf8mb4
    COLLATE = utf8mb4_0900_ai_ci;

CREATE TABLE IF NOT EXISTS conversation_kind
(
    kind_id TINYINT UNSIGNED NOT NULL,
    name    VARCHAR(32)      NOT NULL,

    CONSTRAINT pk_conversation_kind PRIMARY KEY (kind_id),
    CONSTRAINT uq_conversation_kind_name UNIQUE (name)
    ) ENGINE = InnoDB
    DEFAULT CHARSET = utf8mb4
    COLLATE = utf8mb4_0900_ai_ci;

INSERT INTO conversation_kind (kind_id, name)
VALUES (1, '1-1'),
       (2, 'group');

CREATE TABLE IF NOT EXISTS permission
(
    perm_id  SMALLINT UNSIGNED NOT NULL,
    perm_key VARCHAR(64)       NOT NULL, # e.g. message.send

    CONSTRAINT pk_permission PRIMARY KEY (perm_id),
    CONSTRAINT uq_permission_key UNIQUE (perm_key)
    ) ENGINE = InnoDB
    DEFAULT CHARSET = utf8mb4
    COLLATE = utf8mb4_0900_ai_ci;

INSERT INTO permission (perm_id, perm_key)
VALUES (1, 'message.send'),
       (2, 'member.invite');

CREATE TABLE IF NOT EXISTS conversation
(
    conversation_id BINARY(16)       NOT NULL, # UUID
    kind_id         TINYINT UNSIGNED NOT NULL,
    created_at      TIMESTAMP(6)     NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    last_msg_off    BIGINT UNSIGNED  NOT NULL DEFAULT 0,
    last_msg_at     TIMESTAMP(6)     NULL,

    INDEX ix_conv_last (last_msg_at DESC),

    CONSTRAINT pk_conversation PRIMARY KEY (conversation_id),
    CONSTRAINT fk_conversation_kind FOREIGN KEY (kind_id) REFERENCES conversation_kind (kind_id)
    ) ENGINE = InnoDB
    DEFAULT CHARSET = utf8mb4
    COLLATE = utf8mb4_0900_ai_ci;

CREATE TABLE IF NOT EXISTS auth_credential
(
    user_id       BINARY(16)   NOT NULL,
    username      VARCHAR(32)  NOT NULL, # only lower case letters, 0-9 and '_' is supported
    password_hash VARCHAR(255) NOT NULL,
    is_active     BOOLEAN      NOT NULL DEFAULT TRUE,
    created_at    TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),

    CONSTRAINT pk_auth_credential PRIMARY KEY (user_id),
    CONSTRAINT uq_auth_username UNIQUE (username),
    CONSTRAINT fk_auth_user FOREIGN KEY (user_id) REFERENCES user (user_id) ON DELETE CASCADE
    ) ENGINE = InnoDB
    DEFAULT CHARSET = utf8mb4
    COLLATE = utf8mb4_0900_ai_ci;

CREATE TABLE IF NOT EXISTS friendship
(
    user_min     BINARY(16)                  NOT NULL,
    user_max     BINARY(16)                  NOT NULL,
    status       ENUM ('pending','accepted') NOT NULL,
    requested_by BINARY(16)                  NOT NULL, # must be either user_min or user_max
    created_at   TIMESTAMP(6)                NOT NULL DEFAULT CURRENT_TIMESTAMP(6),

    # Helpful indexes for “my friends / my requests”
    INDEX ix_friendship_min_status (user_min, status),
    INDEX ix_friendship_max_status (user_max, status),
    INDEX ix_friendship_requested_status (requested_by, status),
    INDEX ix_friendship_min_status_since (user_min, status, created_at DESC),
    INDEX ix_friendship_max_status_since (user_max, status, created_at DESC),

    CONSTRAINT pk_friendship PRIMARY KEY (user_min, user_max),
    CONSTRAINT fk_friend_min FOREIGN KEY (user_min) REFERENCES user (user_id) ON DELETE CASCADE,
    CONSTRAINT fk_friend_max FOREIGN KEY (user_max) REFERENCES user (user_id) ON DELETE CASCADE,
    CONSTRAINT ck_friendship_requested_by CHECK (requested_by IN (user_min, user_max)),
    CONSTRAINT ck_friendship_order CHECK ( user_min < user_max )
) ENGINE = InnoDB
  DEFAULT CHARSET = utf8mb4
  COLLATE = utf8mb4_0900_ai_ci;

CREATE TABLE IF NOT EXISTS direct_pair
(
    user_min        BINARY(16) NOT NULL,
    user_max        BINARY(16) NOT NULL,
    conversation_id BINARY(16) NOT NULL,

    CONSTRAINT pk_direct_pair PRIMARY KEY (user_min, user_max),
    CONSTRAINT fk_directpair_conversation FOREIGN KEY (conversation_id) REFERENCES conversation (conversation_id) ON DELETE CASCADE,
    CONSTRAINT ck_direct_pair_order CHECK ( user_min < user_max )
    ) ENGINE = InnoDB
    DEFAULT CHARSET = utf8mb4
    COLLATE = utf8mb4_0900_ai_ci;

CREATE TABLE IF NOT EXISTS chat_group
(
    group_id        BINARY(16)  NOT NULL, # UUID
    owner_id        BINARY(16)  NOT NULL,
    group_name      VARCHAR(64) NOT NULL,
    description     TEXT,
    created_at      TIMESTAMP(6) DEFAULT CURRENT_TIMESTAMP(6),
    conversation_id BINARY(16)  NOT NULL,

    CONSTRAINT pk_chat_group PRIMARY KEY (group_id),
    CONSTRAINT uq_chat_group_conversation UNIQUE (conversation_id),
    CONSTRAINT fk_chatgroup_owner FOREIGN KEY (owner_id) REFERENCES user (user_id) ON DELETE CASCADE,
    CONSTRAINT fk_chatgroup_conversation FOREIGN KEY (conversation_id) REFERENCES conversation (conversation_id) ON DELETE CASCADE
    ) ENGINE = InnoDB
    DEFAULT CHARSET = utf8mb4
    COLLATE = utf8mb4_0900_ai_ci;

CREATE TABLE IF NOT EXISTS group_create_idem
(
    owner_id        BINARY(16)                              NOT NULL,
    idem_key        BINARY(16)                              NOT NULL, # client-provided UUID
    proposed_group  BINARY(16)                              NOT NULL,
    conversation_id BINARY(16)                              NULL,
    status          ENUM ('pending', 'succeeded', 'failed') NOT NULL DEFAULT 'pending',
    created_at      TIMESTAMP(6)                                     DEFAULT CURRENT_TIMESTAMP(6),

    CONSTRAINT pk_group_create_idem PRIMARY KEY (owner_id, idem_key)
    ) ENGINE = InnoDB
    DEFAULT CHARSET = utf8mb4
    COLLATE = utf8mb4_0900_ai_ci;

CREATE TABLE IF NOT EXISTS conversation_counter
(
    conversation_id BINARY(16)      NOT NULL,
    next_offset     BIGINT UNSIGNED NOT NULL,

    CONSTRAINT pk_conversation_counter PRIMARY KEY (conversation_id),
    CONSTRAINT ck_conversation_counter_positive CHECK ( next_offset > 0 ),
    CONSTRAINT fk_convcounter_conversation FOREIGN KEY (conversation_id) REFERENCES conversation (conversation_id) ON DELETE CASCADE
    ) ENGINE = InnoDB
    DEFAULT CHARSET = utf8mb4
    COLLATE = utf8mb4_0900_ai_ci;

# INSERT INTO conversation_counters (conversation_id, next_offset)
# VALUES (UUID_TO_BIN(?), 1)
# ON DUPLICATE KEY UPDATE next_offset = LAST_INSERT_ID(next_offset + 1);

CREATE TABLE IF NOT EXISTS conversation_member
(
    conversation_id BINARY(16)      NOT NULL,
    user_id         BINARY(16)      NOT NULL,
    joined_at       TIMESTAMP(6)    NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    last_read_off   BIGINT UNSIGNED NOT NULL DEFAULT 0,

    INDEX ix_member_user (user_id, conversation_id),

    CONSTRAINT pk_conversation_member PRIMARY KEY (conversation_id, user_id),
    CONSTRAINT fk_convmember_conversation FOREIGN KEY (conversation_id) REFERENCES conversation (conversation_id) ON DELETE CASCADE,
    CONSTRAINT fk_convmember_user FOREIGN KEY (user_id) REFERENCES user (user_id) ON DELETE CASCADE
    ) ENGINE = InnoDB
    DEFAULT CHARSET = utf8mb4
    COLLATE = utf8mb4_0900_ai_ci;

CREATE TABLE IF NOT EXISTS message
(
    message_id      BINARY(16)      NOT NULL, # UUID
    conversation_id BINARY(16)      NOT NULL,
    message_offset  BIGINT UNSIGNED NOT NULL,
    sender_id       BINARY(16)      NOT NULL,
    content         TEXT            NOT NULL,
    created_at      TIMESTAMP(6)    NOT NULL DEFAULT CURRENT_TIMESTAMP(6),

    INDEX ix_message_id (message_id),

    # Clustered by conversation then offset for fast pagination
    CONSTRAINT pk_message PRIMARY KEY (conversation_id, message_offset),
    # Global handle for cross-table references and links
    CONSTRAINT uq_message_id UNIQUE KEY (message_id),
    CONSTRAINT fk_message_conversation FOREIGN KEY (conversation_id) REFERENCES conversation (conversation_id) ON DELETE CASCADE,
    CONSTRAINT fk_message_sender FOREIGN KEY (sender_id) REFERENCES user (user_id) ON DELETE CASCADE
    ) ENGINE = InnoDB
    DEFAULT CHARSET = utf8mb4
    COLLATE = utf8mb4_0900_ai_ci;

CREATE TABLE IF NOT EXISTS conversation_role
(
    role_id         BIGINT AUTO_INCREMENT NOT NULL,
    conversation_id BINARY(16)            NOT NULL,
    name            VARCHAR(32)           NOT NULL,

    CONSTRAINT pk_conversation_role PRIMARY KEY (role_id),
    CONSTRAINT uq_conversation_role_name UNIQUE KEY (conversation_id, name),
    CONSTRAINT fk_convrole_conversation FOREIGN KEY (conversation_id) REFERENCES conversation (conversation_id) ON DELETE CASCADE
    ) ENGINE = InnoDB
    DEFAULT CHARSET = utf8mb4
    COLLATE = utf8mb4_0900_ai_ci;

CREATE TABLE IF NOT EXISTS conversation_role_perm
(
    role_id BIGINT                 NOT NULL,
    perm_id SMALLINT UNSIGNED      NOT NULL,
    effect  ENUM ('allow', 'deny') NOT NULL, # explicit deny beats allow

    CONSTRAINT pk_conversation_role_perm PRIMARY KEY (role_id, perm_id),
    CONSTRAINT fk_roleperm_role FOREIGN KEY (role_id) REFERENCES conversation_role (role_id) ON DELETE CASCADE,
    CONSTRAINT fk_roleperm_perm FOREIGN KEY (perm_id) REFERENCES permission (perm_id)
    ) ENGINE = InnoDB
    DEFAULT CHARSET = utf8mb4
    COLLATE = utf8mb4_0900_ai_ci;

CREATE TABLE IF NOT EXISTS conversation_member_role
(
    conversation_id BINARY(16) NOT NULL,
    user_id         BINARY(16) NOT NULL,
    role_id         BIGINT     NOT NULL,

    CONSTRAINT pk_conversation_member_role PRIMARY KEY (conversation_id, user_id),
    CONSTRAINT fk_memberrole_conversation FOREIGN KEY (conversation_id) REFERENCES conversation (conversation_id) ON DELETE CASCADE,
    CONSTRAINT fk_memberrole_user FOREIGN KEY (user_id) REFERENCES user (user_id) ON DELETE CASCADE,
    CONSTRAINT fk_memberrole_role FOREIGN KEY (role_id) REFERENCES conversation_role (role_id) ON DELETE CASCADE
    ) ENGINE = InnoDB
    DEFAULT CHARSET = utf8mb4
    COLLATE = utf8mb4_0900_ai_ci;

CREATE TABLE IF NOT EXISTS outbox
(
    event_id        BINARY(16)      NOT NULL,
    event_type      VARCHAR(64)     NOT NULL, # e.g. "chat.message.new" "friendship.new"
    partition_key   BINARY(16)     NULL,

    # snapshot at send-time; array of UserId
    receivers_json  JSON            NOT NULL, # e.g. ["ab12…","cd34…",...]
    payload_json    JSON            NOT NULL, # full event body (versioned)

    created_at      TIMESTAMP(6)    NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    delivered_at    TIMESTAMP(6)    NULL,
    attempt_count   INT             NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMP(6)    NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    last_error      VARCHAR(1024)   NULL,

    INDEX idx_outbox_ready (delivered_at, next_attempt_at, created_at),
    INDEX idx_outbox_type (event_type),

    CONSTRAINT pk_outbox PRIMARY KEY (event_id)
    ) ENGINE = InnoDB
    DEFAULT CHARSET = utf8mb4
    COLLATE = utf8mb4_0900_ai_ci;
