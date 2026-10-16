'use strict';
// Explicit opt-in: five model operations through the user's configured Codex
// provider/auth. Only generated probe knowledge is used; no real project memory.
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');
const {spawnSync} = require('node:child_process');
const {AppServer,ManagedSession,SUPPORTED} = require('../integrations/codex/session.cjs');
const codex = path.resolve(process.argv[2]);
const exe = path.resolve(process.argv[3] || 'target/release/swapctx.exe');
const root = fs.mkdtempSync(path.join(os.tmpdir(),'swapctx-model-probe-'));
const project = path.join(root,'project'),dataDir = path.join(root,'data');fs.mkdirSync(project);
const report = {protocol:'swapctx.model-evaluation',version:1,syntheticDataOnly:true,codexVersion:SUPPORTED,checks:[]};
const prompt = 'Return only the release identifier from the current SwapCtx project memory. If absent return UNKNOWN. Do not use tools.';
function cli(args) {const r=spawnSync(exe,['--data-dir',dataDir,...args],{cwd:project,encoding:'utf8',timeout:15000});assert.equal(r.status,0,'Synthetic memory operation failed');}
async function baseline() {
  const api = new AppServer(codex,project);
  try {
    await api.initialize();const account = await api.call('account/read',{refreshToken:false});
    if (account.requiresOpenaiAuth && !account.account) {const e=new Error('Codex authentication required');e.code='authentication_required';throw e;}
    const thread = await api.call('thread/start',{cwd:project,ephemeral:true,sandbox:'read-only',approvalPolicy:'never'});
    let text = '',toolSeen = false;
    const waiting = api.waitTurn(thread.thread.id,m=>{
      if (m.method === 'item/agentMessage/delta') text += m.params.delta;
      if (m.method === 'item/started' && ['commandExecution','fileChange','mcpToolCall','dynamicToolCall'].includes(m.params.item?.type)) toolSeen = true;
    },60000);waiting.promise.catch(()=>{});
    try {
      await api.call('turn/start',{threadId:thread.thread.id,input:[{type:'text',text:prompt}]});
      const result = await waiting.promise;
      assert.equal(result.status,'completed','Baseline turn failed');assert.equal(text.trim(),'UNKNOWN','No-context control failed');assert.equal(toolSeen,false,'Unexpected tool activity');
      report.checks.push('no-context control returns UNKNOWN');
    } finally {waiting.cancel();}
  } finally {await api.close();}
}
(async()=>{
  const version=spawnSync(codex,['--version'],{encoding:'utf8',timeout:5000});assert.equal(version.stdout.trim(),'codex-cli '+SUPPORTED,'Unsupported Codex version');
  await baseline(); // Generate the unpredictable answers only AFTER the control.
  cli(['init']);
  let text = '',toolSeen = false;
  const managed = new ManagedSession({swapctx:exe,dataDir,project},{codex,sandbox:'read-only'},path.join(root,'state.json'),e=>{if(e.event==='text')text+=e.text;if(e.event==='tool_started')toolSeen=true;});
  async function checkedTurn(value,label) {
    text = '';toolSeen=false;await managed.perform(prompt);assert.equal(text.trim(),value,label+' did not use current selected knowledge');assert.equal(toolSeen,false,'Unexpected tool activity');report.checks.push(label);
  }
  const a=crypto.randomBytes(16).toString('hex');cli(['memory','add','The project release identifier is '+a,'--kind','requirement']);await checkedTurn(a,'startup nonce recalled');
  const b=crypto.randomBytes(16).toString('hex');cli(['memory','supersede','1','The project release identifier is '+b]);await checkedTurn(b,'resumption uses superseding nonce');
  await managed.perform('',{compact:true});report.checks.push('manual compaction completed');
  const c=crypto.randomBytes(16).toString('hex');cli(['memory','supersede','2','The project release identifier is '+c]);await checkedTurn(c,'post-compaction continuation uses new nonce');
  report.ok=true;report.contextUseObserved=true;report.guaranteedAdherence=false;
  console.log(JSON.stringify(report,null,2));
})().catch(e=>{
  report.ok=false;report.contextUseObserved=false;report.error={code:e.code || 'model_evaluation_failed'};
  console.log(JSON.stringify(report,null,2));process.exitCode=e.code==='authentication_required'?2:1;
}).finally(()=>{assert.equal(path.dirname(root),os.tmpdir());fs.rmSync(root,{recursive:true,force:true});});
