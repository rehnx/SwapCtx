'use strict';
// Offline generator/checker. Cargo metadata uses only the locked local cache.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const zlib = require('node:zlib');
const {spawnSync} = require('node:child_process');
const root = path.resolve(__dirname,'..');
const target = 'x86_64-pc-windows-msvc';
const digest = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
function crateFiles(archive) {
  const tar = zlib.gunzipSync(archive,{maxOutputLength:128*1024*1024}), files = new Map();
  let longName;
  for (let pos = 0; pos + 512 <= tar.length;) {
    const header = tar.subarray(pos,pos+512);
    if (header.every(byte => byte === 0)) break;
    const field = (offset,length) => header.subarray(offset,offset+length).toString('utf8').replace(/\0.*$/s,'');
    const sizeField = field(124,12).trim();
    if (!/^[0-7]+$/.test(sizeField)) throw new Error('Unsupported crate tar size');
    const size = parseInt(sizeField,8), name = field(0,100), prefix = field(345,155);
    const file = longName || (prefix ? `${prefix}/${name}` : name);
    longName = undefined;
    const kind = field(156,1);
    pos += 512;
    if (pos+size > tar.length) throw new Error('Truncated crate archive');
    if (kind === '' || kind === '0') {
      if (files.has(file)) throw new Error('Duplicate crate entry');
      files.set(file,tar.subarray(pos,pos+size));
    } else if (kind === 'L') longName = tar.subarray(pos,pos+size).toString('utf8').replace(/\0.*$/s,'');
    else if (kind !== '5') throw new Error(`Unsupported crate entry type ${kind}`);
    pos += Math.ceil(size/512)*512;
  }
  return files;
}
function generate(metadata) {
  const lock = fs.readFileSync(path.join(root,'Cargo.lock'),'utf8');
  const locked = new Map(lock.split('[[package]]').slice(1).map(block => {
    const value = key => block.match(new RegExp(`^${key} = "([^"]+)"`,'m'))?.[1];
    return [`${value('name')}@${value('version')}`,value('checksum')];
  }));
  const reachable = new Set(metadata.resolve.nodes.map(node => node.id));
  const packages = metadata.packages.filter(pkg => pkg.source && reachable.has(pkg.id))
    .sort((a,b) => a.name < b.name ? -1 : a.name > b.name ? 1 : a.version.localeCompare(b.version,'en'));
  let text = 'SwapCtx third-party notices for x86_64-pc-windows-msvc\n\n'
    + 'Generated offline from Cargo.lock-verified cached crate archives.\n'
    + 'Includes build dependencies and all alternative root license texts, without\n'
    + 'selecting a license on the recipient\'s behalf. This is a source notice\n'
    + 'inventory, not legal clearance or a vulnerability audit. Rust/MSVC runtime\n'
    + 'components and tools installed separately are outside this crate inventory.\n';
  const inventory = {format_version:1,target,lock_sha256:digest(fs.readFileSync(path.join(root,'Cargo.lock'))),packages:[]};
  for (const pkg of packages) {
    if (!pkg.source.startsWith('registry+') || !pkg.license) throw new Error(`Unreviewed dependency ${pkg.name}`);
    const dir = path.dirname(pkg.manifest_path);
    const archive = fs.readFileSync(path.resolve(dir,'../../../cache',path.basename(path.dirname(dir)),`${pkg.name}-${pkg.version}.crate`));
    const checksum = locked.get(`${pkg.name}@${pkg.version}`);
    if (!checksum || digest(archive) !== checksum) throw new Error(`Crate archive checksum mismatch: ${pkg.name}`);
    const prefix = `${pkg.name}-${pkg.version}/`, entries = crateFiles(archive);
    const files = [...entries.keys()].filter(file => file.startsWith(prefix)).map(file => file.slice(prefix.length))
      .filter(name => /^(licen[sc]e|copying|notice|unlicense)([.-].*)?$/i.test(name) && !name.includes('/'));
    // Additional Unicode data notice is embedded below the crate root.
    if (pkg.name === 'regex-syntax') files.push('src/unicode_tables/LICENSE-UNICODE');
    if (pkg.license_file && !files.includes(pkg.license_file)) files.push(pkg.license_file);
    files.sort();
    if (!files.length) throw new Error(`No license text for ${pkg.name}`);
    const entry = {name:pkg.name,version:pkg.version,license:pkg.license,source:pkg.source,crate_sha256:checksum,files:[]};
    text += `\n${'='.repeat(72)}\n${pkg.name} ${pkg.version}\nDeclared license: ${pkg.license}\n`;
    function content(relative) {
      const bytes = entries.get(prefix+relative);
      if (!bytes) throw new Error(`Missing crate notice: ${pkg.name}/${relative}`);
      return bytes;
    }
    for (const file of files) {
      const bytes = content(file);
      entry.files.push({path:file,sha256:digest(bytes)});
      text += `\n--- ${file} ---\n${bytes.toString('utf8').replace(/\r\n/g,'\n').trimEnd()}\n`;
    }
    if (pkg.name === 'libsqlite3-sys') {
      const relative = 'sqlite3/sqlite3.c', bytes = content(relative);
      const source = bytes.toString('utf8').replace(/\r\n/g,'\n');
      const notice = source.match(/\/\*\n\*\* 2001[^]*?\*\//);
      if (!notice || !notice[0].includes('disclaims copyright')) throw new Error('Bundled SQLite notice changed; review required');
      entry.files.push({path:relative,sha256:digest(bytes),excerpt:'first public-domain notice'});
      text += `\n--- Bundled SQLite: first public-domain notice from ${relative} ---\n${notice[0]}\n`;
    }
    inventory.packages.push(entry);
  }
  inventory.notices_sha256 = digest(Buffer.from(text));
  return {text, inventory:JSON.stringify(inventory,null,2)+'\n'};
}
if (require.main === module) {
  const mode = process.argv[2];
  if (!['--write','--check'].includes(mode)) throw new Error('Usage: node scripts/third-party.cjs --write|--check');
  const result = spawnSync('cargo',['metadata','--offline','--locked','--format-version','1','--filter-platform',target],{cwd:root,encoding:'utf8',maxBuffer:8*1024*1024});
  if (result.error || result.status !== 0) throw new Error(`Offline cargo metadata failed: ${result.error || result.stderr}`);
  const generated = generate(JSON.parse(result.stdout));
  for (const [file,bytes] of [['THIRD_PARTY_NOTICES.txt',generated.text],['docs/third-party-inventory.json',generated.inventory]]) {
    const dest = path.join(root,file);
    if (mode === '--write') fs.writeFileSync(dest,bytes);
    else if (!fs.readFileSync(dest).equals(Buffer.from(bytes))) throw new Error(`${file} is stale; regenerate and review`);
  }
  console.log(`Third-party notices ${mode === '--write' ? 'generated' : 'verified'} for ${JSON.parse(generated.inventory).packages.length} locked Windows packages`);
}
module.exports = {generate};
