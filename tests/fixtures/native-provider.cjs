'use strict';
// Synthetic native UI only. No provider SDK, authentication, network or model.
const fs=require('node:fs'),path=require('node:path'),{spawnSync}=require('node:child_process');
const args=process.argv.slice(2),provider=process.env.SWAPCTX_FAKE_PROVIDER;
const reportFile=process.env.SWAPCTX_FAKE_REPORT;
const report=fs.existsSync(reportFile)?JSON.parse(fs.readFileSync(reportFile,'utf8')):{probes:[],callbacks:[]};
const save=()=>fs.writeFileSync(reportFile,JSON.stringify(report));
if(args.length===1&&['--version','--help'].includes(args[0])) {
  report.probes.push(args[0]);save();
  console.log(args[0]==='--version'
    ?process.env.SWAPCTX_FAKE_UNSUPPORTED?'unsupported fixture':provider==='codex'?'codex-cli 9.0.0':'2.1.296 (Claude Code)'
    :provider==='codex'?'--config --dangerously-bypass-hook-trust':'--plugin-dir');
  process.exit(0);
}
report.ui_started=true;report.arguments=args;save();
let executable,hookArgs,directory;
if(provider==='claude'&&args.includes('--plugin-dir')) {
  directory=args[args.indexOf('--plugin-dir')+1];
  const definition=JSON.parse(fs.readFileSync(path.join(directory,'hooks/hooks.json'),'utf8')).hooks.SessionStart[0].hooks[0];
  executable=definition.command;hookArgs=definition.args;
  report.script_directory=path.dirname(hookArgs[0]);
} else if(provider==='codex'&&args.includes('-c')) {
  const definition=args[args.indexOf('-c')+1];
  const command=JSON.parse(/command_windows=("(?:\\.|[^"\\])*")/.exec(definition)[1]);
  executable=/^"([^"]+)" /.exec(command)[1];
  const encoded=command.split(' ').at(-1);
  hookArgs=['-NoProfile','-NonInteractive','-EncodedCommand',encoded];
  const decoded=Buffer.from(encoded,'base64').toString('utf16le');
  const values=[...decoded.matchAll(/'((?:[^']|'')*)'/g)].map(m=>m[1].replace(/''/g,"'"));
  directory=path.dirname(values.at(-1));report.script_directory=path.dirname(values[1]);
}
report.hook_directory=directory;save();
if(process.env.SWAPCTX_FAKE_MODE==='interrupt') process.kill(process.pid,'SIGTERM');
if(directory&&process.env.SWAPCTX_FAKE_MODE!=='no-hooks') {
  const sources=['startup','resume','compact','clear','fork'];
  for(const source of sources) {
    const event={hook_event_name:'SessionStart',source,cwd:process.env.SWAPCTX_FAKE_EVENT_CWD||process.cwd(),session_id:'synthetic-do-not-retain'};
    const result=spawnSync(executable,hookArgs,{input:process.env.SWAPCTX_FAKE_MODE==='bad-hook'?'{invalid':JSON.stringify(event),encoding:'utf8',timeout:12000});
    let output;try{output=JSON.parse(result.stdout);}catch{output={};}
    const text=output.hookSpecificOutput?.additionalContext;
    let packet;try{packet=JSON.parse(text.slice(text.indexOf('\n')+1));}catch{packet=null;}
    const owner=packet?fs.statSync(packet.project.root):null,local=fs.statSync(process.cwd());
    report.callbacks.push({source,exit:result.status,context:!!text,project_matches:!!owner&&owner.dev===local.dev&&owner.ino===local.ino,
      expected_requirement:!!packet?.records.some(r=>r.content===(process.env.SWAPCTX_FAKE_MODE==='refresh'&&source!=='startup'?'Keep updated compatibility':'Keep the public interface stable')),
      protocol_valid:packet?.protocol==='swapctx.context'&&packet?.version===1,
      mandatory_complete:packet?.coverage.mandatory_context_complete===true,advisory:!!output.systemMessage});save();
    if(source==='startup'&&process.env.SWAPCTX_FAKE_MODE==='refresh') {
      const changed=spawnSync(process.env.SWAPCTX_FAKE_CLI,['--data-dir',process.env.SWAPCTX_FAKE_DATA,'memory','supersede','1','Keep updated compatibility'],{encoding:'utf8'});
      if(changed.status!==0)process.exit(42);
    }
  }
}
console.log('synthetic-native-interface');
process.exit(Number(process.env.SWAPCTX_FAKE_EXIT||0));
