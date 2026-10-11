// Supplemental check of SQL taken directly from the Rust source. This does NOT
// compile Rust or exercise the CLI. Requires Node.js with node:sqlite (Node 24).
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { DatabaseSync } = require('node:sqlite');
const root = path.resolve(__dirname, '..');
const source = fs.readFileSync(path.join(root, 'src/storage.rs'), 'utf8');
const main = fs.readFileSync(path.join(root, 'src/main.rs'), 'utf8');
function decode(raw) {
  return JSON.parse(raw.replace(/\r/g, '\\r').replace(/\n/g, '\\n'));
}
function sqlStrings(text) {
  return [...text.matchAll(/"(?:[^"\\]|\\.)*"/g)]
    .filter(m => /^"(?:SELECT|INSERT|DELETE)/.test(m[0])).map(m => decode(m[0]));
}
const schema = decode(source.match(/tx\.execute_batch\(\s*("(?:[^"\\]|\\.)*")/)[1]);
const statements = sqlStrings(main);
const findSQL = prefix => {
  const sql = statements.find(s => s.startsWith(prefix));
  assert.ok(sql, `Source SQL missing: ${prefix}`);
  return sql;
};
const search = findSQL('SELECT id, kind, content, created_at FROM memories\n');
const recent = statements.find(s => s.includes('FROM memories WHERE project_id = ?1 ORDER BY id DESC LIMIT ?2'));
assert.ok(recent);
let checks = 0;
function check(name, fn) { fn(); checks++; console.log(`PASS ${name}`); }
function fresh() {
  const db = new DatabaseSync(':memory:');
  db.exec('PRAGMA foreign_keys=ON');
  db.exec(schema);
  return db;
}
const db = fresh();
check('fresh schema and idempotent initialization', () => {
  db.exec(schema);
  assert.equal(db.prepare("SELECT count(*) AS n FROM sqlite_schema WHERE type='table'").get().n, 3);
});
check('schema identity unique-index validation SQL', () => {
  const unique = sqlStrings(source).find(s => s.includes("pragma_index_list('projects')"));
  assert.ok(unique);
  assert.equal(Object.values(db.prepare(unique).get())[0], 1);
  const wrong = new DatabaseSync(':memory:');
  wrong.exec('CREATE TABLE projects(id INTEGER PRIMARY KEY,path TEXT NOT NULL)');
  assert.equal(Object.values(wrong.prepare(unique).get())[0], 0);
  wrong.exec('CREATE UNIQUE INDEX partial_identity ON projects(path) WHERE id > 0');
  assert.equal(Object.values(wrong.prepare(unique).get())[0], 0);
  wrong.close();
});
check('Windows ASCII project lookup uses an index', () => {
  const sql = sqlStrings(source).find(s => s === 'SELECT id FROM projects WHERE path = ?1 COLLATE NOCASE');
  assert.ok(sql);
  const plan = db.prepare(`EXPLAIN QUERY PLAN ${sql}`).all({'?1': 'example'});
  assert.ok(plan.some(r => r.detail.includes('projects_path_nocase_idx')));
});
check('Windows ambiguous project identity detection SQL', () => {
  const sql = sqlStrings(source).find(s => s.includes('GROUP BY path COLLATE NOCASE HAVING count(*) > 1'));
  assert.ok(sql);
  const identities = fresh();
  identities.exec("INSERT INTO projects(path) VALUES ('C:/Project'),('c:/project')");
  assert.equal(Object.values(identities.prepare(sql).get())[0], 1);
  identities.close();
});
db.exec("INSERT INTO projects(id,path) VALUES (1,'first'),(2,'second')");
const add = db.prepare(findSQL('INSERT INTO memories(project_id, kind, content)'));
add.run({'?1': 1, '?2': 'requirement', '?3': 'Preserve 世界 100%_done\\path'});
add.run({'?1': 1, '?2': 'note', '?3': 'latest'});
add.run({'?1': 2, '?2': 'note', '?3': 'secret other project'});
function query(term, project = 1) {
  const escaped = term.replace(/\\/g, '\\\\').replace(/%/g, '\\%').replace(/_/g, '\\_');
  return db.prepare(search).all({'?1': project, '?2': `%${escaped}%`, '?3': 10});
}
check('literal percent, underscore and backslash search', () => {
  assert.equal(query('%_done\\path').length, 1);
  assert.match(query('%_done\\path')[0].content, /世界/);
});
check('search kinds and ASCII case behavior', () => assert.equal(query('REQUIREMENT').length, 1));
check('project isolation in search', () => assert.equal(query('secret').length, 0));
check('SQL injection remains literal input', () => assert.equal(query("' OR 1=1 --").length, 0));
check('recent ordering and limit', () => {
  const rows = db.prepare(recent).all({'?1': 1, '?2': 1});
  assert.equal(rows.length, 1);
  assert.equal(rows[0].content, 'latest');
});
check('recent query uses project index without temporary sorting', () => {
  const plan = db.prepare(`EXPLAIN QUERY PLAN ${recent}`).all({'?1': 1, '?2': 10});
  assert.ok(plan.some(r => r.detail.includes('memories_project_idx')));
  assert.ok(plan.every(r => !r.detail.includes('TEMP B-TREE')));
});
check('foreign keys reject orphan records', () => assert.throws(() => add.run({'?1': 99, '?2': 'note', '?3': 'orphan'})));
const legacy = new DatabaseSync(':memory:');
legacy.exec(fs.readFileSync(path.join(root, 'tests/fixtures/v0.sql'), 'utf8'));
legacy.exec("INSERT INTO projects(id,path) VALUES(7,'legacy'); INSERT INTO memories(id,project_id,kind,content) VALUES(42,7,'requirement','keep')");
check('legacy migration preserves IDs/content and existing indexes', () => {
  legacy.exec('BEGIN IMMEDIATE');
  legacy.exec(schema);
  legacy.exec(`PRAGMA application_id=${0x53574358}; PRAGMA user_version=1; COMMIT`);
  assert.equal(legacy.prepare('SELECT content FROM memories WHERE id=42').get().content, 'keep');
  assert.equal(legacy.prepare('PRAGMA user_version').get().user_version, 1);
  assert.equal(legacy.prepare("SELECT count(*) AS n FROM sqlite_schema WHERE name='memories_project_idx'").get().n, 1);
});
check('DDL and version changes roll back together on failure', () => {
  const rollback = new DatabaseSync(':memory:');
  rollback.exec('BEGIN IMMEDIATE');
  rollback.exec(schema);
  rollback.exec('PRAGMA user_version=1');
  assert.throws(() => rollback.exec('INSERT INTO missing VALUES(1)'));
  rollback.exec('ROLLBACK');
  assert.equal(rollback.prepare("SELECT count(*) AS n FROM sqlite_schema WHERE type='table'").get().n, 0);
  assert.equal(rollback.prepare('PRAGMA user_version').get().user_version, 0);
  rollback.close();
});
check('project-scoped removal cannot delete another project rule', () => {
  db.exec("INSERT INTO rules(id,project_id,pattern,reason) VALUES(1,1,'src/**','review')");
  const remove = db.prepare(findSQL('DELETE FROM rules WHERE id = ?1 AND project_id = ?2'));
  assert.equal(remove.run({'?1': 1, '?2': 2}).changes, 0);
  assert.equal(remove.run({'?1': 1, '?2': 1}).changes, 1);
});
check('database integrity after changes', () => {
  assert.equal(db.prepare('PRAGMA integrity_check').get().integrity_check, 'ok');
  assert.equal(legacy.prepare('PRAGMA integrity_check').get().integrity_check, 'ok');
  assert.equal(db.prepare('PRAGMA foreign_key_check').all().length, 0);
});
const diskDir = fs.mkdtempSync(path.join(os.tmpdir(), 'swapctx-sql-'));
const diskPath = path.join(diskDir, 'isolated.db');
let writer;
let reader;
let contender;
try {
  writer = new DatabaseSync(diskPath);
  writer.exec(schema);
  writer.exec("INSERT INTO projects(id,path) VALUES(1,'disk-project')");
  writer.prepare(findSQL('INSERT INTO memories(project_id, kind, content)'))
    .run({'?1': 1, '?2': 'requirement', '?3': 'persistent 世界'});
  writer.close(); writer = undefined;
  reader = new DatabaseSync(diskPath, { readOnly: true });
  check('file-backed records survive close and read-only reopen', () => {
    assert.equal(reader.prepare(recent).all({'?1': 1, '?2': 10})[0].content, 'persistent 世界');
  });
  check('read-only connection rejects data writes', () => {
    assert.throws(() => reader.prepare(findSQL('INSERT INTO memories(project_id, kind, content)'))
      .run({'?1': 1, '?2': 'note', '?3': 'should fail'}), /readonly/i);
  });
  reader.close(); reader = undefined;
  writer = new DatabaseSync(diskPath);
  contender = new DatabaseSync(diskPath);
  contender.exec('PRAGMA busy_timeout=25');
  check('SQLite contention fails without partial writes and succeeds after rollback', () => {
    writer.exec('BEGIN IMMEDIATE');
    const insert = contender.prepare(findSQL('INSERT INTO memories(project_id, kind, content)'));
    assert.throws(() => insert.run({'?1': 1, '?2': 'note', '?3': 'locked'}), /locked/i);
    writer.exec('ROLLBACK');
    assert.equal(writer.prepare('SELECT count(*) AS n FROM memories').get().n, 1);
    insert.run({'?1': 1, '?2': 'note', '?3': 'after rollback'});
    assert.equal(writer.prepare('SELECT count(*) AS n FROM memories').get().n, 2);
  });
} finally {
  if (reader) reader.close();
  if (contender) contender.close();
  if (writer) writer.close();
  // Delete only the exact fixture file and its now-empty directory; no recursive cleanup.
  fs.unlinkSync(diskPath);
  fs.rmdirSync(diskDir);
}
console.log(`SQLite engine: ${db.prepare('SELECT sqlite_version() AS version').get().version}`);
db.close(); legacy.close();
console.log(`${checks} supplemental SQL checks passed. Rust/CLI verification remains separate.`);
