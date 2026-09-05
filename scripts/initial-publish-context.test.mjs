import assert from 'node:assert/strict';
import test from 'node:test';
import { validateInitialPublish } from './initial-publish-context.mjs';

const sha = 'a'.repeat(40);
const runs = [{ headSha: sha, status: 'completed', conclusion: 'success' }];
const packages = [
  { name: 'stack-formatter', version: '0.1.0' },
  { name: 'stack-engine', version: '0.7.0' },
].map(crate => ({ ...crate, publish: ['crates-io'], license: 'Apache-2.0', rust_version: '1.85', dependencies: [] }));

test('accepts the two exact initial crates independently', () => {
  for (const crate of packages) validateInitialPublish({ packages }, runs, sha, crate.name);
});
test('rejects missing, stale, incomplete, and failed CI', () => {
  for (const invalid of [[], [...runs, ...runs], [{ ...runs[0], headSha: 'b'.repeat(40) }], [{ ...runs[0], status: 'in_progress' }], [{ ...runs[0], conclusion: 'failure' }]]) {
    assert.throws(() => validateInitialPublish({ packages }, invalid, sha, 'stack-engine'));
  }
});
test('rejects changed crate, version, registry, license, MSRV, and Git dependency', () => {
  for (const crate of packages) {
    for (const change of [{ version: '9.0.0' }, { publish: null }, { license: 'MIT' }, { rust_version: '1.86' }, { dependencies: [{ source: 'git+https://example.com/repo' }] }]) {
      assert.throws(() => validateInitialPublish({ packages: [{ ...crate, ...change }] }, runs, sha, crate.name));
    }
    assert.throws(() => validateInitialPublish({ packages: [] }, runs, sha, crate.name));
  }
  assert.throws(() => validateInitialPublish({ packages }, runs, sha, 'stack-engine-wasm'));
});
test('rejects non-immutable and malformed dispatch identities', () => {
  for (const invalid of ['main', 'a'.repeat(39), 'A'.repeat(40), `${sha}\n`, undefined]) {
    assert.throws(() => validateInitialPublish({ packages }, runs, invalid, 'stack-engine'));
  }
});
