'use strict';
// Independently inspect copied executable, complete source inventory and build tools.
const fs = require('node:fs');
const path = require('node:path');
const {inspect} = require('./inspect-pe.cjs');
const {inputs,tools,hash} = require('./release-inputs.cjs');
function validate(binary,receipt,root) {
  if (receipt.native_release_tests_run !== true) throw new Error('Receipt-backed release packaging requires native release tests; rebuild with --test');
  const expectedInputs = inputs(root);
  if (JSON.stringify(receipt.inputs) !== JSON.stringify(expectedInputs)) throw new Error('Incomplete or changed release source inventory');
  if (receipt.source_fingerprint !== hash(Buffer.from(JSON.stringify(expectedInputs)))) throw new Error('Source fingerprint does not match inventory');
  if (JSON.stringify(receipt.tools) !== JSON.stringify(tools(root))) throw new Error('Build tool inventory changed or missing; rebuild the release');
  const actual = inspect(fs.readFileSync(binary));
  if (actual.dynamic_crt_imports.length) throw new Error('Copied executable imports a dynamic C runtime');
  if (JSON.stringify(actual) !== JSON.stringify(receipt.pe)) throw new Error('Build receipt PE metadata does not match copied executable');
}
if (require.main === module) {
  if (process.argv.length !== 5) throw new Error('Usage: node validate-build-info.cjs BINARY RECEIPT ROOT');
  validate(process.argv[2],JSON.parse(fs.readFileSync(process.argv[3],'utf8')),path.resolve(process.argv[4]));
  console.log('Independent build receipt inspection passed');
}
module.exports = {validate};
