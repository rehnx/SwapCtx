-- Additive migration: original memory/rule rows, IDs and timestamps are untouched.
-- Missing metadata means active/unkeyed/no expiry for memory, deny for a rule.
CREATE TABLE rule_policies (
    rule_id INTEGER PRIMARY KEY REFERENCES rules(id) ON DELETE CASCADE,
    policy TEXT NOT NULL CHECK (policy IN ('deny', 'ask', 'audit'))
);
CREATE TABLE memory_metadata (
    memory_id INTEGER PRIMARY KEY REFERENCES memories(id) ON DELETE CASCADE,
    state TEXT NOT NULL DEFAULT 'active' CHECK (state IN ('active', 'archived', 'superseded')),
    memory_key TEXT CHECK (memory_key IS NULL OR (length(memory_key) BETWEEN 1 AND 128 AND trim(memory_key) != '')),
    expires_at INTEGER CHECK (expires_at IS NULL OR expires_at > 0),
    reviewed_at INTEGER CHECK (reviewed_at IS NULL OR reviewed_at > 0),
    superseded_by INTEGER REFERENCES memories(id),
    CHECK ((state = 'superseded' AND superseded_by IS NOT NULL) OR (state != 'superseded' AND superseded_by IS NULL))
);
CREATE INDEX memory_key_state_idx ON memory_metadata(memory_key, state);
