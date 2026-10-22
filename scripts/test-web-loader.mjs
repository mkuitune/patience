import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';

const html = readFileSync(new URL('../web/index.html', import.meta.url), 'utf8');
const script = html.match(/<script type="module">([\s\S]*?)<\/script>/)[1]
  .replaceAll('import.meta.url', '"https://example.test/game/"')
  .replaceAll('await import(', 'await load(');

async function run({ gpu, renderer = '', failBackend, hash = '' } = {}) {
  const loaded = [];
  const started = [];
  let removed = false;
  let replaced = false;
  const messages = [];
  const canvas = { cloneNode: () => ({}), replaceWith: () => { replaced = true; } };
  const status = {
    replaceChildren() {},
    append: element => messages.push(element.textContent),
    remove: () => { removed = true; },
  };
  const context = {
    URL, URLSearchParams,
    navigator: { gpu, userAgent: '', maxTouchPoints: 0 },
    matchMedia: () => ({ matches: false }),
    location: { hash, search: renderer ? `?renderer=${renderer}` : '', pathname: '/game/' },
    document: {
      getElementById: id => id === 'game' ? canvas : status,
      createElement: () => ({}),
    },
    window: { addEventListener() {} },
    console: { warn() {}, error() {} },
    load: async path => {
      const backend = path.includes('/webgpu/') ? 'webgpu' : 'webgl2';
      loaded.push(backend);
      return {
        default: async ({ module_or_path }) => {
          assert.equal(module_or_path.pathname, `/game/engines/v1/${backend}/patience_app_bg.wasm`);
        },
        start: async () => {
          started.push(backend);
          if (backend === failBackend) throw new Error('GPU startup failed');
        },
      };
    },
  };
  await vm.runInNewContext(`(async () => {${script}})()`, context);
  return { loaded, started, removed, replaced, messages };
}

const adapter = { requestAdapter: async () => ({}) };
assert.deepEqual((await run()).loaded, ['webgl2']);
assert.deepEqual((await run({ gpu: adapter })).loaded, ['webgpu']);
assert.deepEqual((await run({ gpu: { requestAdapter: async () => null } })).loaded, ['webgl2']);
assert.deepEqual((await run({ gpu: { requestAdapter: async () => { throw Error(); } } })).loaded, ['webgl2']);
const fallback = await run({ gpu: adapter, failBackend: 'webgpu' });
assert.deepEqual(fallback.started, ['webgpu', 'webgl2']);
assert.equal(fallback.replaced, true);
assert.equal(fallback.removed, true);
assert.deepEqual((await run({ gpu: adapter, renderer: 'webgl2' })).loaded, ['webgl2']);
const forced = await run({ renderer: 'webgpu', failBackend: 'webgpu' });
assert.deepEqual(forced.loaded, ['webgpu']);
assert.equal(forced.removed, false);
assert.deepEqual((await run({ renderer: 'invalid' })).loaded, []);
assert.deepEqual((await run({ hash: '#v2:state' })).loaded, []);
assert.equal((await run({ failBackend: 'webgl2' })).removed, false);
console.log('Web loader: selection, forced backends, fallback, paths, and errors passed.');
