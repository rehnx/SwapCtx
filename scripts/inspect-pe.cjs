'use strict';
// Read-only PE inspection; never loads or executes the binary.
const fs = require('node:fs');
function inspect(bytes) {
  const need = (offset,count) => {
    if (!Number.isSafeInteger(offset) || offset < 0 || offset+count > bytes.length) throw new Error('Truncated or invalid PE offset');
    return offset;
  };
  const u16 = offset => bytes.readUInt16LE(need(offset,2));
  const u32 = offset => bytes.readUInt32LE(need(offset,4));
  if (u16(0) !== 0x5a4d) throw new Error('Not a PE executable');
  const pe = u32(0x3c);
  if (u32(pe) !== 0x4550 || u16(pe+4) !== 0x8664) throw new Error('Expected x64 PE executable');
  const optional = pe+24, size = u16(pe+20), sections = u16(pe+6);
  need(optional,size);
  if (u16(optional) !== 0x20b || size < 240 || sections > 96) throw new Error('Unsupported PE optional header');
  const sectionTable = optional+size;
  need(sectionTable,sections*40);
  function rva(value,count = 1) {
    for (let index=0;index<sections;index++) {
      const s = sectionTable+index*40, start = u32(s+12), rawSize = u32(s+16);
      if (value >= start && value-start+count <= rawSize) return need(u32(s+20)+value-start,count);
    }
    throw new Error('Unmapped PE RVA');
  }
  function name(value) {
    const offset = rva(value), end = bytes.indexOf(0,offset);
    if (end < offset || end-offset > 256) throw new Error('Invalid import name');
    const result = bytes.toString('ascii',offset,end);
    if (!/^[A-Za-z0-9_.-]+\.dll$/i.test(result)) throw new Error('Invalid DLL import');
    return result.toUpperCase();
  }
  function imports(directory,width,nameOffset,delay = false) {
    const va = u32(optional+112+directory*8), length = u32(optional+116+directory*8);
    if (!va && !length) return [];
    if (!va || length < width || length > 1024*1024) throw new Error('Invalid import directory');
    const start = rva(va,length), result = [];
    let terminated = false;
    for (let offset=start;offset+width<=start+length;offset+=width) {
      if (bytes.subarray(offset,offset+width).every(byte=>byte===0)) {terminated=true;break;}
      if (delay && u32(offset) !== 1) throw new Error('Unsupported delay import addressing');
      result.push(name(u32(offset+nameOffset)));
    }
    if (!terminated) throw new Error('Unterminated import directory');
    return [...new Set(result)].sort();
  }
  const direct = imports(1,20,12), delayed = imports(13,32,4,true);
  return {machine:'x86_64',imports:direct,delay_imports:delayed,
    dynamic_crt_imports:[...new Set([...direct,...delayed])].filter(name=>/^(VCRUNTIME|MSVCP|MSVCR|UCRTBASE|API-MS-WIN-CRT-)/.test(name)).sort()};
}
if (require.main === module) console.log(JSON.stringify(inspect(fs.readFileSync(process.argv[2])),null,2));
module.exports = {inspect};
