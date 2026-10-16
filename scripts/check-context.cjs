// Independent JSON parser and real CLI/adapter contract tests; Node 24+.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const {spawnSync} = require('node:child_process');
const {DatabaseSync} = require('node:sqlite');
const exe = path.resolve(process.argv[2] || 'target/debug/swapctx.exe');
const adapter = path.resolve(__dirname, '../integrations/codex/session-context.cjs');
const schema = JSON.parse(fs.readFileSync(path.resolve(__dirname,'../docs/schema/context-v1.schema.json'),'utf8'));
// Test validator for the specific keywords used in this checked-in schema.
// It is not shipped as a general purpose JSON Schema implementation.
function validate(value, rule=schema) {
  if(rule.$ref) return validate(value,schema.$defs[rule.$ref.split('/').pop()]);
  if('const' in rule) assert.deepEqual(value,rule.const);
  if(rule.enum) assert.ok(rule.enum.includes(value));
  if(rule.type) {const types=Array.isArray(rule.type)?rule.type:[rule.type];assert.ok(types.some(t=>t==='null'?value===null:t==='array'?Array.isArray(value):t==='integer'?Number.isSafeInteger(value):t==='object'?value!==null&&typeof value==='object'&&!Array.isArray(value):typeof value===t));}
  if(typeof value==='number') {if(rule.minimum!==undefined)assert.ok(value>=rule.minimum);if(rule.maximum!==undefined)assert.ok(value<=rule.maximum);}
  if(typeof value==='string') {if(rule.minLength!==undefined)assert.ok(value.length>=rule.minLength);if(rule.pattern)assert.match(value,new RegExp(rule.pattern));}
  if(Array.isArray(value)) {if(rule.maxItems!==undefined)assert.ok(value.length<=rule.maxItems);if(rule.items)for(const item of value)validate(item,rule.items);}
  if(rule.required)for(const key of rule.required)assert.ok(Object.hasOwn(value,key),'missing '+key);
  if(rule.properties)for(const [key,child]of Object.entries(rule.properties))if(Object.hasOwn(value,key))validate(value[key],child);
  if(rule.oneOf)assert.equal(rule.oneOf.filter(child=>{try{validate(value,child);return true;}catch{return false;}}).length,1);
}
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'swapctx-context-'));
let count = 0;
function check(name, fn) {fn(); count++; console.log('PASS ' + name);}
function fixture(name) {
  const project = path.join(tmp, name), data = path.join(tmp, name + '-data');
  fs.mkdirSync(project);
  function cli(args, cwd = project) {
    return spawnSync(exe, ['--data-dir',data,...args], {cwd, encoding:'utf8',timeout:15000});
  }
  function ok(args) {const r=cli(args); assert.equal(r.status,0,r.stderr);return r.stdout;}
  ok(['init']);
  function add(text, args=[]) {ok(['memory','add',text,...args]); const db=new DatabaseSync(path.join(data,'swapctx.db'));const id=db.prepare('SELECT max(id) id FROM memories').get().id;db.close();return id;}
  function sql(fn) {const db=new DatabaseSync(path.join(data,'swapctx.db'));try {fn(db);} finally {db.close();}}
  function packet(args=[], cwd=project) {const r=cli(['context',...args],cwd);assert.equal(r.status,0,r.stdout+r.stderr);const p=JSON.parse(r.stdout);validate(p);assert.equal(p.budget.bytes_used,Buffer.byteLength(r.stdout.trimEnd()));assert.ok(Buffer.byteLength(r.stdout)<=p.budget.max_bytes);assert.equal(p.protocol,'swapctx.context');assert.equal(p.version,1);assert.equal(p.ok,true);return p;}
  function hook(input, args=[]) {const r=spawnSync(process.execPath,[adapter,'--swapctx',exe,'--data-dir',data,'--project',project,...args],{input: typeof input==='string'||Buffer.isBuffer(input)?input:JSON.stringify(input),encoding:'utf8',timeout:10000});assert.equal(r.status,0,r.stderr);assert.equal(r.stderr,'');return JSON.parse(r.stdout);}
  return {project,data,cli,ok,add,sql,packet,hook};
}
try {
  check('operational errors are JSON and do not create a missing store',()=>{
    const data=path.join(tmp,'missing');const r=spawnSync(exe,['--data-dir',data,'context'],{cwd:tmp,encoding:'utf8'});
    assert.equal(r.status,1);validate(JSON.parse(r.stdout));assert.equal(JSON.parse(r.stdout).error.code,'store_unavailable');assert.equal(r.stderr,'');assert.equal(fs.existsSync(data),false);
  });
  const f=fixture('project ü space');
  check('unsupported versions and sensitive queries return generic JSON errors',()=>{
    for(const args of [['--protocol-version','2'],['--query','password=NeverPrintThisSecret123']]) {
      const r=f.cli(['context',...args]);assert.equal(r.status,1);assert.equal(JSON.parse(r.stdout).ok,false);assert.ok(!r.stdout.includes('NeverPrint'));assert.equal(r.stderr,'');
    }
  });
  const requirement=f.add('Keep SQLite offline', ['--kind','requirement','--source','spec.md','--actor','maintainer']);
  f.add('alpha beta older');f.add('alpha newest');
  check('requirements survive unrelated queries and rank before term matches',()=>{
    let p=f.packet(['--query','alpha beta']);assert.deepEqual(p.records.map(r=>r.content),['Keep SQLite offline','alpha beta older','alpha newest']);assert.equal(p.records[0].origin.source,'spec.md');assert.equal(p.coverage.mandatory_context_complete,true);
    p=f.packet(['--query','unrelated']);assert.equal(p.records.length,1);assert.equal(p.records[0].id,String(requirement));
  });
  check('literal wildcards, quotes and terminal controls round-trip safely',()=>{
    const text='literal %_ " quote ü\u0085\u202e';f.add(text);const r=f.cli(['context','--query','%_']);const p=JSON.parse(r.stdout);assert.equal(p.records[1].content,text);assert.ok(!r.stdout.includes('\u0085'));assert.ok(!r.stdout.includes('\u202e'));assert.equal(f.packet(['--query',"' OR 1=1"]).records.length,1);
  });
  const stale=f.add('old knowledge');const unknown=f.add('unknown origin');
  f.add('conflicting A',['--key','architecture']);f.add('conflicting B',['--key','architecture']);
  const invalid=f.add('invalid text');f.ok(['memory','invalidate',String(invalid),'--reason','wrong']);
  f.add('password=HiddenValue123456',['--allow-sensitive']);
  f.sql(db=>{db.prepare('UPDATE memory_metadata SET expires_at=1 WHERE memory_id=?').run(stale);db.prepare("UPDATE memory_origins SET actor='unknown' WHERE memory_id=?").run(unknown);});
  check('stale, competing, invalid, unknown and sensitive records are withheld',()=>{
    const p=f.packet();assert.equal(p.omitted.stale,1);assert.equal(p.omitted.conflicted,2);assert.equal(p.omitted.unknown_provenance,1);assert.equal(p.omitted.sensitive,1);assert.equal(p.omitted.inactive_or_invalid,1);assert.ok(!JSON.stringify(p).includes('HiddenValue'));
    const review=f.packet(['--include-stale','--include-conflicts','--allow-unknown-provenance']);assert.equal(review.coverage.review_required_records,4);assert.equal(review.coverage.mandatory_context_complete,false);
  });
  check('byte budgets retain whole records and expose incomplete requirement coverage',()=>{
    const b=fixture('budget');b.add('ü'.repeat(3000),['--kind','requirement']);const p=b.packet(['--max-bytes','2048']);assert.equal(p.records.length,0);assert.equal(p.coverage.mandatory_context_complete,false);assert.equal(p.omitted.budget,1);
  });
  check('record limit exposes missing mandatory requirements',()=>{
    const b=fixture('limit');b.add('one',['--kind','requirement']);b.add('two',['--kind','requirement']);const p=b.packet(['--limit','1']);assert.equal(p.coverage.requirements_returned,1);assert.equal(p.coverage.mandatory_context_complete,false);assert.equal(p.omitted.record_limit,1);
  });
  check('read-only packets leave database bytes unchanged and discover nested roots',()=>{
    const before=fs.readFileSync(path.join(f.data,'swapctx.db'));const nested=path.join(f.project,'nested');fs.mkdirSync(nested);assert.equal(f.packet([],nested).records[0].id,String(requirement));assert.deepEqual(fs.readFileSync(path.join(f.data,'swapctx.db')),before);
  });
  check('missing provenance fails closed instead of silently omitting facts',()=>{
    const b=fixture('broken');const id=b.add('fact');b.sql(db=>db.prepare('DELETE FROM memory_origins WHERE memory_id=?').run(id));const r=b.cli(['context']);assert.equal(r.status,1);assert.equal(JSON.parse(r.stdout).error.code,'retrieval_failed');
  });
  check('boundaries are included with truthful enforcement metadata',()=>{
    f.ok(['protect','add','--path','src/**','--reason','review source','--policy','ask']);const p=f.packet();assert.equal(p.boundary_rules[0].policy,'ask');assert.equal(p.boundary_rules[0].enforced,false);assert.equal(p.limitations.enforcement,false);
  });
  check('adapter accepts all supported lifecycle inputs and excludes sensitive data',()=>{
    for(const source of ['startup','resume','clear','compact']) {const h=f.hook({hook_event_name:'SessionStart',source,cwd:f.project});assert.equal(h.hookSpecificOutput.hookEventName,'SessionStart');assert.ok(h.hookSpecificOutput.additionalContext.includes('Keep SQLite offline'));assert.ok(!JSON.stringify(h).includes('HiddenValue'));}
  });
  check('adapter stops on malformed, oversized, wrong-event and outside-project input',()=>{
    for(const input of ['{',Buffer.from([0xff]),'x'.repeat(32769),{hook_event_name:'PreToolUse',source:'startup',cwd:f.project},{hook_event_name:'SessionStart',source:'startup',cwd:tmp}]) assert.equal(f.hook(input).continue,false);
  });
  check('adapter stops when a mandatory requirement is stale',()=>{
    f.sql(db=>db.prepare('UPDATE memory_metadata SET expires_at=1 WHERE memory_id=?').run(requirement));assert.equal(f.hook({hook_event_name:'SessionStart',source:'startup',cwd:f.project}).continue,false);
  });
  check('launcher rejects empty, invalid UTF-8 and oversized prompts before child start',()=>{
    const b=fixture('launcher-input');
    for(const input of ['',Buffer.from([0xff]),'x'.repeat(16385)]) {
      const r=spawnSync(process.execPath,[path.resolve(__dirname,'../integrations/codex/exec-context.cjs'),'--swapctx',exe,'--codex',process.execPath,'--data-dir',b.data,'--project',b.project],{input,encoding:'utf8',timeout:5000});
      // If invoked, Node would reject Codex's exec options with a different error/code.
      assert.equal(r.status,1);assert.equal(r.stdout,'');assert.match(r.stderr,/SwapCtx launch refused/);
    }
  });
  check('sensitive provenance labels withhold the whole record',()=>{
    const b=fixture('secret-origin');b.add('ordinary knowledge',['--actor','password=PrivateActor193847','--allow-sensitive']);const p=b.packet();assert.equal(p.records.length,0);assert.equal(p.omitted.sensitive,1);
  });
  check('registered child projects cannot inherit the parent bridge scope',()=>{
    const b=fixture('parent');const child=path.join(b.project,'child');fs.mkdirSync(child);assert.equal(b.cli(['init'],child).status,0);assert.equal(b.packet([],child).records.length,0);assert.equal(b.hook({hook_event_name:'SessionStart',source:'startup',cwd:child}).continue,false);
  });
  check('legacy stores require explicit migration and are left untouched',()=>{
    const project=path.join(tmp,'legacy-project'),data=path.join(tmp,'legacy-data');fs.mkdirSync(project);fs.mkdirSync(data);const file=path.join(data,'swapctx.db');const db=new DatabaseSync(file);db.exec(fs.readFileSync(path.resolve(__dirname,'../tests/fixtures/v0.sql'),'utf8'));db.prepare('INSERT INTO projects(path) VALUES(?)').run(fs.realpathSync.native(project));db.close();const before=fs.readFileSync(file);const r=spawnSync(exe,['--data-dir',data,'context'],{cwd:project,encoding:'utf8'});assert.equal(r.status,1);assert.equal(JSON.parse(r.stdout).error.code,'migration_required');assert.deepEqual(fs.readFileSync(file),before);
  });
  check('64-bit SQLite identities survive JSON parsing without precision loss',()=>{
    const b=fixture('wide-id');b.add('wide identity');b.sql(db=>{db.exec('PRAGMA foreign_keys=OFF; BEGIN; UPDATE memories SET id=9007199254740993; UPDATE memory_metadata SET memory_id=9007199254740993; UPDATE memory_origins SET memory_id=9007199254740993; UPDATE memory_events SET memory_id=9007199254740993; COMMIT');});assert.equal(b.packet().records[0].id,'9007199254740993');
  });
  check('5001 candidates report capped scanning, never complete delivery',()=>{
    const b=fixture('scan');b.sql(db=>{db.exec('BEGIN');const m=db.prepare("INSERT INTO memories(project_id,kind,content) VALUES(1,'note',?)");const o=db.prepare("INSERT INTO memory_origins VALUES(?,?,'manual','user',1)");const e=db.prepare("INSERT INTO memory_events(memory_id,action,detail,recorded_at) VALUES(?,'add','fixture',1)");for(let i=0;i<5001;i++){const id=Number(m.run('fact '+i).lastInsertRowid);o.run(id,id.toString(16).padStart(32,'0'));e.run(id);}db.exec('COMMIT');});const p=b.packet();assert.equal(p.scan.candidates_examined,5000);assert.equal(p.scan.truncated,true);assert.equal(p.coverage.mandatory_context_complete,false);
  });
  console.log(`${count} context/adapter checks passed`);
} finally {
  // Only remove the exact directory returned by mkdtemp, under the chosen temp root.
  assert.equal(path.dirname(tmp),os.tmpdir());fs.rmSync(tmp,{recursive:true,force:true});
}
