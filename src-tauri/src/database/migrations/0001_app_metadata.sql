-- Baseline schema.
--
-- Key/value facts about this database file itself (not user data).
-- Domain tables are added by later migrations as each roadmap phase lands.
CREATE TABLE app_metadata (
    key   TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
) STRICT;

INSERT INTO app_metadata (key, value)
VALUES ('created_at', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
