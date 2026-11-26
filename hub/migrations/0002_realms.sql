CREATE TABLE realms (
    id                 uuid PRIMARY KEY,
    owner_id           uuid NOT NULL REFERENCES accounts (id),
    name               text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 32),
    -- The settings of the new world (`RealmSettings`), passed to the agent at each start.
    settings           jsonb NOT NULL DEFAULT '{}',
    state              text NOT NULL DEFAULT 'stopped' CHECK (state IN
                           ('stopped', 'starting', 'running', 'stopping', 'failed', 'archived')),
    -- The slot of a world that has a container, or may still have one.
    slot               smallint UNIQUE CHECK (slot >= 0),
    -- The SHA-256 of the realm token of the current start.
    token_hash         bytea CHECK (length(token_hash) = 32),
    players            integer NOT NULL DEFAULT 0 CHECK (players >= 0),
    state_changed_at   timestamptz NOT NULL DEFAULT now(),
    -- Since when a running world has had no player.
    last_empty_at      timestamptz,
    last_played_at     timestamptz,
    -- Why a world that is stopping, or stopped, failed; NULL for a plain stop.
    failed_reason      text,
    created_at         timestamptz NOT NULL DEFAULT now(),
    -- When the agent archived the volume of an archived world; NULL while it has not.
    volume_archived_at timestamptz,
    CHECK (slot IS NOT NULL OR state NOT IN ('starting', 'running', 'stopping'))
);
CREATE INDEX realms_owner_idx ON realms (owner_id);
CREATE INDEX realms_active_idx ON realms (state) WHERE state IN ('starting', 'running', 'stopping');

CREATE TABLE realm_members (
    realm_id   uuid NOT NULL REFERENCES realms (id) ON DELETE CASCADE,
    account_id uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    role       text NOT NULL CHECK (role IN ('member', 'operator')),
    added_at   timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (realm_id, account_id)
);
CREATE INDEX realm_members_account_idx ON realm_members (account_id);
