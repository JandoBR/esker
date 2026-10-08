CREATE TABLE files (
    id           TEXT PRIMARY KEY,
    item_id      TEXT NOT NULL REFERENCES items (id) ON DELETE CASCADE,
    file_name    TEXT NOT NULL,
    content_type TEXT,
    size         INTEGER,
    created_at   TEXT NOT NULL
);

CREATE INDEX files_item_id ON files (item_id);

-- Move existing single-file items into the new table. The old columns stay in
-- place but are no longer read.
INSERT INTO files (id, item_id, file_name, content_type, size, created_at)
SELECT id, id, COALESCE(file_name, 'file'), content_type, size, created_at
FROM items
WHERE kind = 'file' AND file_path IS NOT NULL;
