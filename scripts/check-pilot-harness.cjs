'use strict';
// These test the evaluator, not a real AI model. No provider requests occur.
const {test,after} = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const {SENTINEL,verifyWorkspace,turnEvidence,resultStatus,requireAuthenticated} = require('./check-pilot.cjs');
const root=fs.mkdtempSync(path.join(os.tmpdir(),'swapctx-pilot-harness-'));
const project=path.join(root,'project ü space');fs.mkdirSync(project);
const nonce='test-only-release-identifier';
const good=`'use strict';
exports.formatLabel=value=>{if(typeof value!=='string')throw new TypeError('string required');return value.trim().replace(/\\s+/g,' ').toLowerCase();};
exports.releaseTag=()=>${JSON.stringify(nonce)};
exports.words=value=>{const text=exports.formatLabel(value);return text?text.split(' '):[];};
exports.wordCount=value=>exports.words(value).length;
exports.hasWord=(value,word)=>exports.words(value).includes(exports.formatLabel(word));
exports.initials=value=>exports.words(value).map(word=>word[0]).join('');
exports.reverseWords=value=>exports.words(value).reverse().join(' ');
`;
function reset(source=good) {fs.writeFileSync(path.join(project,'lib.cjs'),source);fs.writeFileSync(path.join(project,'contract.txt'),SENTINEL);}
function fails(fn,code) {assert.throws(fn,e=>e.code===code);}
test('independent artifact checks accept all six stages of a correct implementation',()=>{
  reset();for(let stage=0;stage<6;stage++){const evidence=verifyWorkspace(project,nonce,stage);assert.match(evidence.implementationHash,/^[a-f0-9]{64}$/);}
});
test('stale release identifiers fail despite otherwise correct code',()=>{
  reset();fails(()=>verifyWorkspace(project,'superseding-release',5),'workspace_behavior_failed');
});
test('premature successful process exit cannot skip the independent assertions',()=>{
  reset('process.exit(0);');fails(()=>verifyWorkspace(project,nonce,0),'workspace_behavior_failed');
});
test('formatting regressions fail after later features were implemented',()=>{
  reset(good.replace(".replace(/\\s+/g,' ')",''));fails(()=>verifyWorkspace(project,nonce,5),'workspace_behavior_failed');
});
test('a missing feature cannot pass based on assistant claims',()=>{
  reset(good.replace('exports.initials=value=>exports.words(value).map(word=>word[0]).join(\'\');',''));fails(()=>verifyWorkspace(project,nonce,4),'workspace_behavior_failed');
});
test('saved unchanged-file requirement is checked byte-for-byte',()=>{
  reset();fs.appendFileSync(path.join(project,'contract.txt'),'changed');fails(()=>verifyWorkspace(project,nonce,0),'saved_requirement_violated');
});
test('unexpected files are reported rather than silently ignored',()=>{
  reset();fs.writeFileSync(path.join(project,'unexpected'),'extra');try {fails(()=>verifyWorkspace(project,nonce,0),'unexpected_workspace_paths');} finally {fs.unlinkSync(path.join(project,'unexpected'));}
});
test('the oracle child has no inherited API key and cannot write project files',()=>{
  reset(good+"\nrequire('node:fs').writeFileSync('forbidden','write');\n");fails(()=>verifyWorkspace(project,nonce,0),'workspace_behavior_failed');assert.equal(fs.existsSync(path.join(project,'forbidden')),false);
  reset(good+"\nif(process.env.OPENAI_API_KEY)throw new Error('unexpected inherited credential');\n");
  const previous=process.env.OPENAI_API_KEY;process.env.OPENAI_API_KEY='test-only-placeholder';try {verifyWorkspace(project,nonce,0);} finally {if(previous===undefined)delete process.env.OPENAI_API_KEY;else process.env.OPENAI_API_KEY=previous;}
});
function events(resumed=false,threadId='test-thread') {return [
  {event:'session_ready',threadId,resumed},{event:'context_selected',hash:'test-hash',bytes:2048},
  {event:'turn_completed',status:'completed',operation:'turn'},
  {event:'operation_metrics',success:true,contextMs:1,sessionMs:2,turnMs:3,totalMs:7}
];}
test('session evidence rejects thread changes, missing completion and absent metrics',()=>{
  assert.equal(turnEvidence(events()).threadId,'test-thread');assert.equal(turnEvidence(events(true),'test-thread').threadId,'test-thread');
  fails(()=>turnEvidence(events(true,'foreign'),'test-thread'),'session_continuity_failed');
  fails(()=>turnEvidence(events().filter(e=>e.event!=='turn_completed')),'incomplete_turn_evidence');
  fails(()=>turnEvidence(events().filter(e=>e.event!=='operation_metrics')),'operation_metrics_missing');
});
test('compaction requires its actual completion event',()=>{
  const input=events(true);input.find(e=>e.event==='turn_completed').operation='compact';
  fails(()=>turnEvidence(input,'test-thread',true),'compaction_not_observed');input.push({event:'compaction_completed'});turnEvidence(input,'test-thread',true);
});
test('blocked and partial failed evaluations are never classified as passed',()=>{
  assert.equal(resultStatus({code:'authentication_required'}),'blocked');assert.equal(resultStatus({code:'provider_authentication_unverified'}),'blocked');assert.equal(resultStatus({code:'workspace_behavior_failed'}),'failed');assert.equal(resultStatus(null),'passed');
  assert.equal(resultStatus({code:'codex_startup_permission_denied'}),'blocked');
});
test('no-auth providers cannot be credited as authenticated real-model evidence',()=>{
  fails(()=>requireAuthenticated({requiresOpenaiAuth:false,account:null}),'provider_authentication_unverified');
  fails(()=>requireAuthenticated({requiresOpenaiAuth:true,account:null}),'authentication_required');
  fails(()=>requireAuthenticated({requiresOpenaiAuth:true,account:{type:'unknown'}}),'provider_authentication_unverified');
  fails(()=>requireAuthenticated({requiresOpenaiAuth:true}),'codex_protocol_invalid');
  requireAuthenticated({requiresOpenaiAuth:true,account:{type:'apiKey'}});
});
after(()=>{assert.equal(path.dirname(root),os.tmpdir());fs.rmSync(root,{recursive:true,force:true});});
