ALTER TABLE items ADD COLUMN expires_at TEXT;

CREATE INDEX items_expires_at ON items (expires_at);
