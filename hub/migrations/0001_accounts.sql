CREATE EXTENSION IF NOT EXISTS citext;

CREATE TABLE accounts (
    id            uuid PRIMARY KEY,
    username      citext NOT NULL UNIQUE,
    display_name  text NOT NULL,
    status        text NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'suspended', 'deleted')),
    status_reason text,
    realm_quota   integer NOT NULL DEFAULT 0 CHECK (realm_quota >= 0),
    created_at    timestamptz NOT NULL DEFAULT now(),
    deleted_at    timestamptz
);

CREATE TABLE account_roles (
    account_id uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    role       text NOT NULL CHECK (role IN ('admin', 'host')),
    granted_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (account_id, role)
);

CREATE TABLE account_identities (
    account_id uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    provider   text NOT NULL,
    subject    text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (provider, subject)
);
CREATE INDEX account_identities_account_idx ON account_identities (account_id);

CREATE TABLE password_credentials (
    account_id    uuid PRIMARY KEY REFERENCES accounts (id) ON DELETE CASCADE,
    password_hash text NOT NULL,
    recovery_hash text NOT NULL,
    updated_at    timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE sessions (
    id           uuid PRIMARY KEY,
    account_id   uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    token_hash   bytea NOT NULL UNIQUE,
    device_name  text,
    created_at   timestamptz NOT NULL DEFAULT now(),
    last_used_at timestamptz NOT NULL DEFAULT now(),
    expires_at   timestamptz NOT NULL
);
CREATE INDEX sessions_account_idx ON sessions (account_id);

CREATE TABLE certificates (
    id         uuid PRIMARY KEY,
    account_id uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    device_key bytea NOT NULL,
    issued_at  timestamptz NOT NULL,
    expires_at timestamptz NOT NULL,
    revoked_at timestamptz
);
CREATE INDEX certificates_account_idx ON certificates (account_id, issued_at);

CREATE TABLE signups (
    id         bigserial PRIMARY KEY,
    ip_key     bytea NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX signups_ip_idx ON signups (ip_key, created_at);
CREATE INDEX signups_created_idx ON signups (created_at);

CREATE TABLE audit_log (
    id         bigserial PRIMARY KEY,
    at         timestamptz NOT NULL DEFAULT now(),
    actor      text NOT NULL,
    action     text NOT NULL,
    account_id uuid,
    details    jsonb NOT NULL DEFAULT '{}'
);
