CREATE TABLE users (
    sub          TEXT PRIMARY KEY,
    username     TEXT,
    display_name TEXT,
    email        TEXT,
    is_admin     INTEGER NOT NULL DEFAULT 0,
    created_at   TEXT NOT NULL,
    last_login_at TEXT NOT NULL
);

CREATE TABLE sessions (
    token_hash TEXT PRIMARY KEY,
    user_sub   TEXT NOT NULL REFERENCES users (sub) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL
);

CREATE INDEX sessions_user_sub ON sessions (user_sub);

CREATE TABLE login_states (
    state         TEXT PRIMARY KEY,
    nonce         TEXT NOT NULL,
    pkce_verifier TEXT NOT NULL,
    created_at    TEXT NOT NULL
);

CREATE TABLE items (
    id           TEXT PRIMARY KEY,
    kind         TEXT NOT NULL CHECK (kind IN ('text', 'link', 'file')),
    title        TEXT NOT NULL,
    description  TEXT,
    body         TEXT,
    url          TEXT,
    file_name    TEXT,
    file_path    TEXT,
    content_type TEXT,
    size         INTEGER,
    created_by   TEXT REFERENCES users (sub) ON DELETE SET NULL,
    created_at   TEXT NOT NULL
);

CREATE INDEX items_created_at ON items (created_at DESC);
