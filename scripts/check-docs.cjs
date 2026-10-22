'use strict';
// Local documentation checks. Never discovers or launches an AI provider.
const fs=require('node:fs');
const os=require('node:os');
const path=require('node:path');
const assert=require('node:assert/strict');
const {spawnSync}=require('node:child_process');
const root=path.resolve(__dirname,'..');
const binary=path.resolve(process.argv[2]||path.join(root,'target/release/swapctx.exe'));
const files=['docs/INTELLIGENCE_WORKFLOWS.md','docs/UNIFIED_INTEGRATION.md','AGENTS.md','docs/MEMORY_QUALITY.md','README.md','CONTRIBUTING.md','SECURITY.md','SUPPORT.md','CHANGELOG.md',
  'docs/USAGE.md','docs/RELEASE_PREPARATION.md','docs/PHASE11_RELEASE_READINESS.md',
  'docs/WINDOWS_RELEASE.md','.github/ISSUE_TEMPLATE/bug_report.md',
  '.github/ISSUE_TEMPLATE/feature_request.md','.github/pull_request_template.md'];
const read=file=>fs.readFileSync(path.join(root,file),'utf8');
function fences(text,file) {
  const blocks=[];let current;
  for(const line of text.split(/\r?\n/)) {
    const match=/^```([^`]*)$/.exec(line);
    if(match) {
      if(current) {assert.equal(match[1],'',file+': invalid closing fence');blocks.push(current);current=undefined;}
      else current={language:match[1],text:''};
    } else if(current) current.text+=line+'\n';
  }
  assert.equal(current,undefined,file+': unclosed fence');return blocks;
}
function anchors(text) {
  const seen=new Map(),result=new Set();
  for(const line of text.split(/\r?\n/)) {
    const heading=/^#{1,6}\s+(.+)$/.exec(line);if(!heading) continue;
    const base=heading[1].toLowerCase().replace(/[^\p{L}\p{N}_\-\s]/gu,'').replace(/\s/g,'-');
    const n=seen.get(base)||0;seen.set(base,n+1);result.add(base+(n?'-'+n:''));
  }
  return result;
}
let links=0;const blocks=[];
for(const file of files) {
  const text=read(file);blocks.push(...fences(text,file).map(block=>({...block,file})));
  // Includes both the inner image and outer target in badge links.
  for(const match of text.matchAll(/\]\(([^\s)]+)(?:\s+"[^"]*")?\)/g)) {
    const target=match[1];if(/^https?:\/\//.test(target)) continue;
    const [relative,fragment]=target.split('#');
    const resolved=relative?path.resolve(path.dirname(path.join(root,file)),decodeURIComponent(relative)):path.join(root,file);
    assert.ok(!path.relative(root,resolved).startsWith('..'),file+': link outside repository');
    assert.ok(fs.statSync(resolved).isFile(),file+': missing link '+target);
    let directory=root;
    for(const part of path.relative(root,resolved).split(path.sep)) {
      assert.ok(fs.readdirSync(directory).includes(part),file+': link casing differs from repository '+target);
      directory=path.join(directory,part);
    }
    if(fragment) assert.ok(anchors(fs.readFileSync(resolved,'utf8')).has(decodeURIComponent(fragment)),file+': missing anchor '+target);
    links++;
  }
}
const readme=read('README.md');
const version=/^version = "([^"]+)"/m.exec(read('Cargo.toml'))[1];
assert.ok(readme.includes('swapctx-'+version+'-windows-x64.zip'),'README package version differs from Cargo');
assert.ok(read('LICENSE').includes('Apache License'));
assert.match(read('Cargo.toml'),/license = "Apache-2.0"/);
assert.ok(fs.existsSync(path.join(root,'.github/workflows/ci.yml')));
for(const text of ['SwapCtx — choose your coding agent','  1  Codex','  2  Claude Code','  q  Quit (or press Enter)']) {
  assert.ok(readme.includes(text));assert.ok(read('src/provider.rs').includes(text));
}
assert.ok(read('src/native.rs').includes('Expose selected SwapCtx project memory to this provider? [y/N]: '));

// Validate the intentionally small flowchart grammar used here, not arbitrary Mermaid.
const diagrams=blocks.filter(b=>b.language==='mermaid');assert.equal(diagrams.length,1);
const lines=diagrams[0].text.trim().split(/\r?\n/);assert.equal(lines.shift(),'flowchart TD');
const ids=new Set(),refs=[];
function node(value) {
  const match=/^(\w+)(?:\[([A-Za-z0-9 ()-]+)\])?$/.exec(value);
  assert.ok(match,'Unsupported Mermaid node syntax: '+value);
  if(match[2]) ids.add(match[1]);refs.push(match[1]);
}
for(const line of lines) {
  const match=/^\s*(\w+(?:\[[^\]]+\])?) (?:-->|-\. [A-Za-z -]+ \.->) (\w+(?:\[[^\]]+\])?)$/.exec(line);
  assert.ok(match,'Unsupported Mermaid edge syntax: '+line);node(match[1]);node(match[2]);
}
for(const id of refs) assert.ok(ids.has(id),'Undefined Mermaid node '+id);

let syntax='not_run_non_windows',commands=[];
if(process.platform==='win32') {
  const ps=path.join(process.env.SystemRoot,'System32/WindowsPowerShell/v1.0/powershell.exe');
  const script=String.raw`
[Console]::InputEncoding=[Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
$blocks=[Console]::In.ReadToEnd() | ConvertFrom-Json
$commands=[Collections.Generic.List[object]]::new()
foreach($block in $blocks) {
  $tokens=$null; $errors=$null
  $ast=[Management.Automation.Language.Parser]::ParseInput($block.text,[ref]$tokens,[ref]$errors)
  if($errors.Count -gt 0) { throw ('PowerShell syntax error in '+$block.file+': '+$errors[0].Message) }
  if($block.file -eq 'README.md') {
    foreach($command in $ast.FindAll({param($n) $n -is [Management.Automation.Language.CommandAst]},$true)) {
      if($command.GetCommandName() -ne 'swapctx') { continue }
      $argv=@($command.CommandElements | Select-Object -Skip 1 | ForEach-Object {
        if($_ -is [Management.Automation.Language.StringConstantExpressionAst]) { $_.Value }
        else { $_.Extent.Text }
      })
      $commands.Add([pscustomobject]@{args=$argv})
    }
  }
}
ConvertTo-Json -InputObject @($commands.ToArray()) -Depth 5 -Compress
`;
  const result=spawnSync(ps,['-NoProfile','-NonInteractive','-EncodedCommand',Buffer.from(script,'utf16le').toString('base64')],{
    input:JSON.stringify(blocks.filter(b=>b.language==='powershell')),encoding:'utf8',timeout:15000,maxBuffer:131072,
  });
  assert.equal(result.status,0,result.stderr);commands=JSON.parse(result.stdout.replace(/^\uFEFF/,''));syntax='passed';
}

// Inline commands use the same parser help path. Never execute provider commands.
for(const match of readme.matchAll(/`swapctx ([^`]+)`/g)) {
  assert.ok(/^[a-z -]+$/.test(match[1]),'Inline CLI example needs an explicit safe parser fixture');
  commands.push({args:match[1].split(' ')});
}
const temp=fs.mkdtempSync(path.join(os.tmpdir(),'swapctx-docs-'));
let coreRuns=0,providerHelp=0;const seen=new Set();
try {
  for(const {args} of commands) {
    const key=JSON.stringify(args);if(seen.has(key)) continue;seen.add(key);
    const provider=['native','launch','codex'].includes(args[0]);
    // A forwarded --help after `--` would belong to the provider, not Clap.
    // Refuse such examples rather than risk starting any external executable.
    if(provider) assert.ok(!args.includes('--'),'Provider forwarding examples require a fake-executable fixture');
    if(!provider) assert.ok(['init','memory','context','status','doctor','privacy','capabilities','--version','--help'].includes(args[0]),'Unreviewed command in docs check');
    const result=spawnSync(binary,['--data-dir',path.join(temp,'data'),...args,...(provider?['--help']:[])],{
      cwd:temp,encoding:'utf8',timeout:10000,maxBuffer:131072,
    });
    assert.equal(result.status,0,'README command failed: '+key+'\n'+result.stderr);
    if(provider) providerHelp++;else coreRuns++;
    if(args[0]==='context') {
      const packet=JSON.parse(result.stdout);
      assert.equal(packet.protocol,'swapctx.context');assert.equal(packet.version,1);
      assert.equal(packet.coverage.mandatory_context_complete,true);
      assert.ok(packet.records.some(r=>r.content==='Preserve public interfaces'));
    }
  }
} finally {fs.rmSync(temp,{recursive:true,force:true});}
console.log(JSON.stringify({check:'documentation',ok:true,files:files.length,local_links:links,
  fenced_blocks:blocks.length,powershell_syntax:syntax,readme_core_commands_executed:coreRuns,
  provider_examples_help_only:providerHelp,mermaid:'restricted flowchart syntax and references checked; GitHub rendering unverified',
  external_links:'not fetched',real_provider_launches:0},null,2));
