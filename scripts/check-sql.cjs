// Supplemental schema checks only. Native Rust/CLI tests are the primary gate.
// Node.js 24 is optional and is not a SwapCtx runtime dependency.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { DatabaseSync } = require('node:sqlite');
const root = path.resolve(__dirname, '..');
const source = fs.readFileSync(path.join(root, 'src/storage.rs'), 'utf8');
const decode = raw => JSON.parse(raw.replace(/\r/g, '\\r').replace(/\n/g, '\\n'));
const base = decode(source.match(/tx\.execute_batch\(\s*("(?:[^"\\]|\\.)*")/)[1]);
const migration = fs.readFileSync(path.join(root, 'src/schema_v2.sql'), 'utf8');
const provenance = fs.readFileSync(path.join(root, 'src/schema_v3.sql'), 'utf8');
const original = fs.readFileSync(path.join(root, 'tests/fixtures/v0.sql'), 'utf8');
const version = Number(source.match(/SCHEMA_VERSION:\s*i64\s*=\s*(\d+)/)[1]);
const app = Number(source.match(/APPLICATION_ID:\s*i64\s*=\s*(0x[0-9a-fA-F]+)/)[1]);
const db = new DatabaseSync(':memory:');
db.exec('PRAGMA foreign_keys=ON');
db.exec(original);
db.exec("INSERT INTO projects(id,path) VALUES(7,'legacy'); INSERT INTO memories(id,project_id,kind,content) VALUES(42,7,'requirement','keep')");
let checks = 0;
function check(name, fn) { fn(); checks++; console.log(`PASS ${name}`); }
function upgrade(conn) {
  conn.exec('BEGIN IMMEDIATE');
  try {
    conn.exec(base);
    if (conn.prepare('PRAGMA user_version').get().user_version < 2) conn.exec(migration);
    if (conn.prepare('PRAGMA user_version').get().user_version < 3) conn.exec(provenance);
    conn.exec(`PRAGMA application_id=${app}; PRAGMA user_version=${version}; COMMIT`);
  } catch (e) { conn.exec('ROLLBACK'); throw e; }
}
check('additive migration preserves original IDs and content', () => {
  upgrade(db);
  assert.equal(db.prepare('SELECT content FROM memories WHERE id=42').get().content, 'keep');
  assert.equal(db.prepare('PRAGMA user_version').get().user_version, version);
});
check('repeated initialization is idempotent', () => {
  upgrade(db);
  assert.equal(db.prepare("SELECT count(*) AS n FROM sqlite_schema WHERE type='table'").get().n, 7);
});
check('invalid policies and orphaned policy references are rejected', () => {
  db.exec("INSERT INTO rules(id,project_id,pattern,reason) VALUES(9,7,'src/**','review')");
  assert.throws(() => db.exec("INSERT INTO rule_policies VALUES(9,'allow')"));
  assert.throws(() => db.exec("INSERT INTO rule_policies VALUES(99,'deny')"));
  db.exec("INSERT INTO rule_policies VALUES(9,'ask')");
});
check('invalid lifecycle state and nonpositive expiry are rejected', () => {
  assert.throws(() => db.exec("INSERT INTO memory_metadata(memory_id,state) VALUES(42,'deleted')"));
  assert.throws(() => db.exec('INSERT INTO memory_metadata(memory_id,expires_at) VALUES(42,0)'));
  db.exec("INSERT INTO memory_metadata(memory_id,memory_key) VALUES(42,'auth')");
});
check('supersession requires a real replacement and consistent state', () => {
  assert.throws(() => db.exec("UPDATE memory_metadata SET state='superseded' WHERE memory_id=42"));
  assert.throws(() => db.exec("UPDATE memory_metadata SET state='superseded',superseded_by=999 WHERE memory_id=42"));
  db.exec("INSERT INTO memories(id,project_id,kind,content) VALUES(43,7,'requirement','replacement')");
  db.exec("UPDATE memory_metadata SET state='superseded',superseded_by=43 WHERE memory_id=42");
});
check('key bounds and blank keys are enforced', () => {
  assert.throws(() => db.exec("UPDATE memory_metadata SET memory_key='' WHERE memory_id=42"));
  assert.throws(() => db.prepare('UPDATE memory_metadata SET memory_key=? WHERE memory_id=42').run('x'.repeat(129)));
});
check('actual migration failure rolls back DDL and preserves the original store', () => {
  const failed = new DatabaseSync(':memory:');
  failed.exec(original);
  failed.exec("INSERT INTO projects(path) VALUES('original'); CREATE INDEX rule_policies ON projects(path)");
  assert.throws(() => upgrade(failed));
  assert.equal(failed.prepare('PRAGMA user_version').get().user_version, 0);
  assert.equal(failed.prepare("SELECT count(*) AS n FROM sqlite_schema WHERE type='table'").get().n, 3);
  assert.equal(failed.prepare('SELECT path FROM projects').get().path, 'original');
  failed.close();
});
check('integrity and foreign keys remain valid after lifecycle changes', () => {
  assert.equal(db.prepare('PRAGMA integrity_check').get().integrity_check, 'ok');
  assert.equal(db.prepare('PRAGMA foreign_key_check').all().length, 0);
});
check('legacy origins are explicitly unknown and stable across initialization', () => {
  const origin = db.prepare('SELECT * FROM memory_origins WHERE memory_id=42').get();
  assert.equal(origin.source, 'legacy-unknown');
  assert.equal(origin.actor, 'unknown');
  assert.match(origin.origin_id, /^[0-9a-f]{32}$/);
  upgrade(db);
  assert.equal(db.prepare('SELECT origin_id FROM memory_origins WHERE memory_id=42').get().origin_id, origin.origin_id);
});
check('history and origin relationships reject orphaned records', () => {
  assert.throws(() => db.exec("INSERT INTO memory_origins VALUES(999,'abc','source','actor',1)"));
  assert.throws(() => db.exec("INSERT INTO memory_events(memory_id,action,detail,recorded_at) VALUES(999,'add','bad',1)"));
  assert.throws(() => db.exec("INSERT INTO memory_events(memory_id,action,detail,recorded_at) VALUES(42,'fabricated','bad',1)"));
});
console.log(`SQLite engine: ${db.prepare('SELECT sqlite_version() AS version').get().version}`);
db.close();
console.log(`${checks} supplemental schema checks passed; run scripts/verify.ps1 for native verification.`);
