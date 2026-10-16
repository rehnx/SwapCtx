-- Existing content, IDs and timestamps are preserved. Legacy origins are unknown.
ALTER TABLE memory_metadata ADD COLUMN validity TEXT NOT NULL DEFAULT 'valid' CHECK(validity IN ('valid','invalid'));
ALTER TABLE memory_metadata ADD COLUMN invalid_reason TEXT;
CREATE TABLE memory_origins (
 memory_id INTEGER PRIMARY KEY REFERENCES memories(id) ON DELETE CASCADE,
 origin_id TEXT NOT NULL UNIQUE,
 source TEXT NOT NULL,
 actor TEXT NOT NULL,
 recorded_at INTEGER NOT NULL CHECK(recorded_at > 0)
);
CREATE TABLE memory_events (
 id INTEGER PRIMARY KEY,
 memory_id INTEGER NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
 action TEXT NOT NULL CHECK(action IN ('legacy','add','review','archive','supersede','invalidate','import')),
 detail TEXT NOT NULL,
 recorded_at INTEGER NOT NULL CHECK(recorded_at > 0)
);
CREATE INDEX memory_events_record_idx ON memory_events(memory_id,id);
INSERT INTO memory_origins(memory_id,origin_id,source,actor,recorded_at)
 SELECT id,lower(hex(randomblob(16))),'legacy-unknown','unknown',unixepoch() FROM memories;
INSERT INTO memory_events(memory_id,action,detail,recorded_at)
 SELECT id,'legacy','Origin and prior action history unavailable; registered during schema migration',unixepoch() FROM memories;
