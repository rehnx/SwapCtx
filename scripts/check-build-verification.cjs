'use strict';
const {test}=require('node:test');
const assert=require('node:assert/strict');
const {requireIdentical}=require('./build-windows.cjs');
const {selectVersion}=require('./windows-toolchain.cjs');
const first='a'.repeat(64),second='b'.repeat(64);
test('strict verification refuses pre/post changes even if final independent builds match',()=> {
  assert.throws(()=>requireIdentical([first,second,second,second]),/no candidate published/);
  assert.throws(()=>requireIdentical([first,first,first,second]),/no candidate published/);
  requireIdentical([first,first,first,first]);
});
test('strict verification refuses missing or malformed observations',()=> {
  for(const values of [[],[first],[first,''],[first,first.toUpperCase()],[first,null]]) assert.throws(()=>requireIdentical(values));
});
test('native selection uses numeric installed versions and refuses unavailable/path inputs',()=> {
  assert.equal(selectVersion(['14.9.123','14.50.100','14.44.35207','14.51.2/escape']),'14.50.100');
  assert.equal(selectVersion(['10.0.26100.0','10.0.19041.0'],'10.0.19041.0'),'10.0.19041.0');
  for(const requested of ['../14.50.100','14.51.100','']) {
    if(requested) assert.throws(()=>selectVersion(['14.50.100'],requested));
  }
  assert.throws(()=>selectVersion(['garbage','../../tool']));
});
