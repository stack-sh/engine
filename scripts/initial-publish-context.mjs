import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

export function validateInitialPublish(metadata, runs, expectedSha, packageName) {
  assert.match(expectedSha, /^[a-f0-9]{40}$/);
  const versions = { 'stack-formatter': '0.1.0', 'stack-engine': '0.7.0' };
  assert.ok(Object.hasOwn(versions, packageName), 'Unsupported initial crate');
  const selected = metadata.packages.filter(crate => crate.name === packageName);
  assert.equal(selected.length, 1, 'Expected one selected package');
  const crate = selected[0];
  assert.equal(crate.version, versions[packageName], 'Only the initial version may use this workflow');
  assert.ok(crate.dependencies.every(dependency => !dependency.source?.startsWith('git+')), 'Git dependencies cannot be published');
  assert.deepEqual(crate.publish, ['crates-io']);
  assert.equal(crate.license, 'Apache-2.0');
  assert.equal(crate.rust_version, '1.85');
  assert.equal(runs.length, 1, 'The exact main commit needs a CI run');
  assert.equal(runs[0].headSha, expectedSha);
  assert.equal(runs[0].status, 'completed');
  assert.equal(runs[0].conclusion, 'success');
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const metadata = JSON.parse(await readFile(process.argv[2], 'utf8'));
  const runs = JSON.parse(await readFile(process.argv[3], 'utf8'));
  validateInitialPublish(metadata, runs, process.env.EXPECTED_SHA, process.env.PACKAGE_NAME);
  console.log('Initial package identity and exact-commit CI verified.');
}
