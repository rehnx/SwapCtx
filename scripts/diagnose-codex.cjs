'use strict';
// No model turns, credential reads/copies by this script, or configuration edits.
// Codex itself reads its configured authentication and may initialize local caches.
const fs = require('node:fs');
const path = require('node:path');
const https = require('node:https');
const {spawnSync} = require('node:child_process');
const {AppServer,Failure,identity,SUPPORTED} = require('../integrations/codex/session.cjs');

function parse(argv) {
  const codex=argv.shift();if(!codex||!path.isAbsolute(codex))throw new Failure('absolute_codex_path_required');
  const options={codex,network:false};
  while(argv.length) {
    const flag=argv.shift();
    if(flag==='--network'&&!options.network)options.network=true;
    else if(['--codex-home','--output'].includes(flag)) {
      const key=flag==='--codex-home'?'home':'output',value=argv.shift();
      if(options[key]||!value||!path.isAbsolute(value))throw new Failure('invalid_arguments');options[key]=value;
    } else throw new Failure('invalid_arguments');
  }
  if(options.output&&fs.existsSync(options.output))throw new Failure('report_destination_exists');
  return options;
}
function probeNetwork(url) {
  return new Promise(resolve=>{
    let complete=false;
    const finish=value=>{if(!complete){complete=true;clearTimeout(timer);resolve({host:new URL(url).hostname,...value});}};
    // A total deadline also bounds DNS/connect stalls. No auth headers or payload.
    const request=https.request(url,{method:'HEAD'},response=>{response.resume();finish({reachable:true,status:response.statusCode});});
    const timer=setTimeout(()=>request.destroy(Object.assign(new Error('timeout'),{code:'ETIMEDOUT'})),5000);
    request.on('error',error=>finish({reachable:false,errorCode:['EACCES','EPERM','ENOTFOUND','EAI_AGAIN','ETIMEDOUT','ECONNREFUSED','ECONNRESET','ENETUNREACH','EHOSTUNREACH','CERT_HAS_EXPIRED','UNABLE_TO_VERIFY_LEAF_SIGNATURE'].includes(error.code)?error.code:'network_error'}));
    request.end();
  });
}
async function main(argv=process.argv.slice(2)) {
  const options=parse(argv),env={...process.env};if(options.home)env.CODEX_HOME=options.home;
  const report={protocol:'swapctx.codex-diagnostic',version:1,date:new Date().toISOString(),testedCodexVersion:SUPPORTED,modelTurnsRequested:0,credentialContentsReadByScript:false,credentialCopiesCreated:false,codexHomeOverride:Boolean(options.home)};
  const version=spawnSync(options.codex,['--version'],{env,encoding:'utf8',timeout:5000,maxBuffer:8192,windowsHide:true});
  const match=/^codex-cli ([0-9]+\.[0-9]+\.[0-9]+)\s*$/.exec(version.stdout||'');
  report.detectedCodexVersion=version.status===0&&match?match[1]:null;
  const login=spawnSync(options.codex,['login','status'],{env,encoding:'utf8',timeout:5000,maxBuffer:8192,windowsHide:true});
  report.loginStatus={exit:login.status,loggedIn:!login.error&&login.status===0,reportsNotLoggedIn:/Not logged in/i.test((login.stdout||'')+(login.stderr||''))};
  const api=new AppServer(options.codex,process.cwd(),env);
  report.appServerInitialized=false;
  try {
    const init=await api.initialize();report.appServerInitialized=true;
    const expected=options.home||(env.USERPROFILE?path.join(env.USERPROFILE,'.codex'):null);
    // Compare identities only; avoid saving account details or personal home paths.
    if(expected&&fs.existsSync(expected))report.nativeHomeDiffersFromExpected=identity(init.codexHome)!==identity(expected);
    const account=await api.call('account/read',{refreshToken:false});
    report.account={present:Boolean(account.account),requiresOpenaiAuth:account.requiresOpenaiAuth===true};
  } catch(error) {
    report.error={code:error instanceof Failure?error.code:'diagnostic_local_failure'};
  } finally {await api.close();}
  if(options.network)report.network=await Promise.all(['https://chatgpt.com','https://api.openai.com'].map(probeNetwork));
  report.note='Login status is not inference access; network probes do not verify model entitlement. No model turn was requested.';
  const blocked=Boolean(report.error)||!report.loginStatus.loggedIn||report.account?.present!==true||Boolean(report.network?.some(item=>!item.reachable));
  report.status=blocked?'blocked':'preflight_passed';
  const json=JSON.stringify(report,null,2)+'\n';if(options.output)fs.writeFileSync(options.output,json,{flag:'wx',mode:0o600});
  process.stdout.write(json);return blocked?2:0;
}
module.exports={parse,probeNetwork,main};
if(require.main===module)main().then(code=>{process.exitCode=code;}).catch(error=>{
  console.error(JSON.stringify({protocol:'swapctx.codex-diagnostic',version:1,status:'failed',error:{code:error instanceof Failure?error.code:'diagnostic_local_failure'}}));process.exitCode=1;
});
