'use strict';
// Evaluator self-tests only. No provider or simulated model success is claimed.
const {test,after}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),os=require('node:os'),path=require('node:path');
const {Collector,prompt,frozen,grade,summarize,compare}=require('./soak-support.cjs');
const {parse,save}=require('./check-soak.cjs');
const root=fs.mkdtempSync(path.join(os.tmpdir(),'swapctx-soak-harness-')),project=path.join(root,'project');fs.mkdirSync(project);
const counts=n=>({inputTokens:n,cachedInputTokens:0,outputTokens:1,reasoningOutputTokens:0,totalTokens:n+1});
function operation(round=0,overrides={}){return {round,threadId:'thread',configuredModel:'test-model',behaviorPassed:true,automaticCompactions:0,unmatchedCompactions:false,afterAutomaticCompaction:false,historyInjectionBytes:0,metrics:{contextMs:1,sessionMs:2,turnMs:3,totalMs:6,adapterCpuUserMs:1,adapterCpuSystemMs:.5,adapterRssBytes:100},usage:{total:counts((round+1)*10),last:counts(10)},...overrides};}
test('default run is bounded and invalid budgets or relative paths fail',()=>{
 const args=[path.join(root,'codex'),path.join(root,'swapctx'),path.join(root,'report')];assert.equal(parse([...args]).turns,24);
 for(const extra of [['--turns','0'],['--turns','97'],['--minutes','0'],['--turns','6','--turns','6'],['--first','unknown']])assert.throws(()=>parse([...args,...extra]));
 assert.throws(()=>parse(['relative',...args.slice(1)]));
});
test('both arms receive identical current requirements and explicit revisions',()=>{
 const first=prompt(0,'new-release'),revision=prompt(6,'revised-release');
 assert.ok(first.includes('new-release'));assert.ok(revision.includes('revised-release'));assert.ok(!revision.includes('new-release'));
 assert.ok(!prompt(7,'revised-release').includes('revised-release'));assert.ok(!prompt(0,'new-release').includes('/compact'));
});
test('collector ignores transcripts and rejects incomplete/rescoped sessions',()=>{
 const c=new Collector();for(let i=0;i<1000;i++)c.emit({event:'text',text:'password=PRIVATE'});
 assert.ok(!JSON.stringify(c).includes('PRIVATE'));assert.throws(()=>c.result());
 c.emit({event:'session_ready',threadId:'thread',resumed:false,configuredModel:'test-model'});c.emit({event:'turn_completed',status:'completed'});c.emit({event:'operation_metrics',success:true});
 assert.equal(c.result().threadId,'thread');assert.throws(()=>c.result('foreign'));
});
test('an invalid latest usage snapshot invalidates older telemetry',()=>{
 const c=new Collector();c.emit({event:'token_usage',last:counts(10),total:counts(10)});assert.ok(c.usage);
 c.emit({event:'warning',code:'invalid_token_usage'});assert.equal(c.usage,null);
});
test('cumulative native counters are not added or reconstructed from last requests',()=>{
 const summary=summarize([operation(0),operation(1)]);assert.equal(summary.reportedTokens.inputTokens,20);assert.equal(summary.reportedTokens.outputTokens,1);assert.equal(summary.adapterCpuMs,3);
});
test('missing and reset counters remain unknown rather than suggesting savings',()=>{
 assert.equal(summarize([operation(0,{usage:null})]).reportedTokens,null);
 const reset=summarize([operation(2),operation(0)]);assert.equal(reset.tokenCounterResetObserved,true);assert.equal(reset.reportedTokens,null);
 const absent=summarize([operation(0,{metrics:{}})]);assert.equal(absent.adapterCpuMs,null);assert.equal(absent.maxObservedAdapterRssBytes,null);
});
test('passing every coding turn without automatic compaction remains incomplete',()=>{
 const arms={managed:[operation(0),operation(1)],baseline:[operation(0),operation(1)]};const result=compare(arms,2);
 assert.equal(result.status,'incomplete');assert.equal(result.automaticRecoveryObserved,false);assert.equal(result.behaviorPassDifference,0);
});
test('automatic recovery needs successful behavior during and following compaction',()=>{
 const arms={managed:[operation(0,{automaticCompactions:1}),operation(1,{afterAutomaticCompaction:true})],baseline:[operation(0),operation(1)]};
 assert.equal(compare(arms,2).status,'passed');assert.equal(compare(arms,2).causalImprovementEstablished,false);
 arms.managed[1].behaviorPassed=false;assert.equal(compare(arms,2).status,'failed');assert.equal(compare(arms,2).automaticRecoveryObserved,false);
});
test('partial runs, changed models and rerouting do not support comparison claims',()=>{
 const arms={managed:[operation()],baseline:[operation(0,{configuredModel:'different'})]};assert.equal(compare(arms,1).behaviorPassDifference,null);
 arms.baseline[0].configuredModel='test-model';arms.baseline[0].modelRerouted=true;assert.equal(compare(arms,1).comparisonComplete,false);
 assert.equal(compare({managed:[operation()],baseline:[]},1).comparisonComplete,false);
});
test('checkpoint publication preserves existing and externally changed reports',()=>{
 const file=path.join(root,'report.json');save(file,{runId:'owned',status:'running'},true);assert.throws(()=>save(file,{runId:'other'},true));
 save(file,{runId:'owned',status:'incomplete'});assert.equal(JSON.parse(fs.readFileSync(file,'utf8')).status,'incomplete');
 assert.throws(()=>save(file,{runId:'foreign'}));assert.equal(JSON.parse(fs.readFileSync(file,'utf8')).runId,'owned');
});
test('extended oracle verifies every API and protects historical fixtures',()=>{
 const files=frozen('obsolete-release');for(const [name,text] of Object.entries(files))fs.writeFileSync(path.join(project,name),text);
 const source=`exports.formatLabel=v=>{if(typeof v!=='string')throw new TypeError();return v.trim().replace(/\\s+/g,' ').toLowerCase()};
 exports.releaseTag=()=> 'current-release';exports.words=v=>{const t=exports.formatLabel(v);return t?t.split(' '):[]};
 exports.wordCount=v=>exports.words(v).length;exports.hasWord=(v,w)=>exports.words(v).includes(exports.formatLabel(w));exports.initials=v=>exports.words(v).map(w=>w[0]).join('');exports.reverseWords=v=>exports.words(v).reverse().join(' ');
 exports.uniqueWords=v=>[...new Set(exports.words(v))];exports.longestWord=v=>exports.words(v).reduce((a,w)=>w.length>a.length?w:a,'');exports.slugify=v=>exports.words(v).join('-');exports.wordLengths=v=>exports.words(v).map(w=>w.length);exports.wordFrequency=v=>[...exports.words(v).reduce((m,w)=>m.set(w,(m.get(w)||0)+1),new Map())];exports.summarize=v=>({label:exports.formatLabel(v),count:exports.wordCount(v),initials:exports.initials(v)});`;
 fs.writeFileSync(path.join(project,'lib.cjs'),source);grade(project,'current-release',23,files);
 assert.throws(()=>grade(project,'superseding-release',23,files),e=>e.code==='workspace_behavior_failed');
 fs.appendFileSync(path.join(project,'ARCHIVE.md'),'changed');assert.throws(()=>grade(project,'current-release',23,files),e=>e.code==='saved_requirement_violated');
});
after(()=>{assert.equal(path.dirname(root),os.tmpdir());fs.rmSync(root,{recursive:true,force:true});});
