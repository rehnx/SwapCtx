'use strict';
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const hash = bytes=>crypto.createHash('sha256').update(bytes).digest('hex');
function inputs(root) {
  const files = ['Cargo.toml','Cargo.lock','rust-toolchain.toml'];
  function collect(relative) {
    for (const entry of fs.readdirSync(path.join(root,relative),{withFileTypes:true})) {
      const name = `${relative}/${entry.name}`;
      if (entry.isSymbolicLink()) throw new Error(`Source links are not supported: ${name}`);
      if (entry.isDirectory()) collect(name);
      else if (/\.(rs|sql|cjs)$/.test(name)) files.push(name);
    }
  }
  for (const dir of ['src','tests','integrations/codex']) collect(dir);
  files.sort();
  return files.map(file=>({path:file,sha256:hash(fs.readFileSync(path.join(root,file)))}));
}
function tools(root) {
  return ['build-windows.cjs','inspect-pe.cjs','release-inputs.cjs'].map(file=>({path:'scripts/'+file,sha256:hash(fs.readFileSync(path.join(root,'scripts',file)))}));
}
module.exports = {inputs,tools,hash};
