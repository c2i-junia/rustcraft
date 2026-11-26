-- `game`: a bearer token of the game client. `admin_web`: the cookie of the web admin.
ALTER TABLE sessions ADD COLUMN kind text NOT NULL DEFAULT 'game' CHECK (kind IN ('game', 'admin_web'));

-- The web admin reads the audit log of one account, newest first.
CREATE INDEX audit_log_account_idx ON audit_log (account_id, id);
