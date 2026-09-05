import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';

for (const name of ['stack-formatter', 'stack-engine']) {
  for (const file of ['LICENSE', 'THIRD_PARTY_LICENSES.md']) {
    const source = await readFile(new URL(`../${file}`, import.meta.url));
    const target = new URL(`../crates/${name}/${file}`, import.meta.url);
    if (process.argv.includes('--check')) assert.deepEqual(await readFile(target), source, `${name}/${file} drift`);
    else await writeFile(target, source);
  }
}

const fixture = await readFile(new URL('../tests/fixtures/provider-pack-input.json', import.meta.url));
const target = new URL('../crates/stack-engine/tests/fixtures/provider-pack-input.json', import.meta.url);
if (process.argv.includes('--check')) assert.deepEqual(await readFile(target), fixture, 'Cargo provider fixture drift');
else await writeFile(target, fixture);
