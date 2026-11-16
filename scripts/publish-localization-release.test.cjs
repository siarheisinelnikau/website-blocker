const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const publish = require('./publish-localization-release.cjs');

async function fixture(run) {
  const original = process.cwd();
  const root = path.resolve('.test-output/release-tests');
  fs.mkdirSync(root, { recursive: true });
  const directory = fs.mkdtempSync(path.join(root, 'publish-'));
  const sha = 'a'.repeat(40), assets = [];
  fs.mkdirSync(path.join(directory, '.release-stage'));
  for (const [name, data] of [['WebsiteBlocker-v0.6.0.exe', 'checked executable fixture'], ['SHA256SUMS.txt', 'checked checksum fixture'], ['checksums.txt', 'updater checksum fixture']]) {
    const bytes = Buffer.from(data);
    fs.writeFileSync(path.join(directory, '.release-stage', name), bytes);
    assets.push({ name, size: bytes.length, digest: `sha256:${crypto.createHash('sha256').update(bytes).digest('hex')}` });
  }
  fs.writeFileSync(path.join(directory, '.release-stage/manifest.json'), JSON.stringify({
    product: 'WebsiteBlocker', version: '0.6.0', tag: 'v0.6.0', commit: sha, assets,
    notesText: 'WebsiteBlocker 0.6.0 refreshes the desktop and browser focus interfaces. Settings and website lists are preserved. This test uses artificial release assets only.',
  }));
  const state = { calls: [], release: null, published: false, wrongTag: false, badUpload: false };
  const missing = () => { throw Object.assign(new Error('Not found'), { status: 404 }); };
  const github = { rest: {
    git: { async getRef() { if (state.wrongTag) return { data: { object: { type: 'commit', sha: 'b'.repeat(40) } } }; return missing(); } },
    repos: {
      async getReleaseByTag() { if (!state.release) return missing(); return { data: state.release }; },
      async createRelease(input) { state.calls.push('create'); state.release = { ...input, id: 1, assets: [] }; return { data: state.release }; },
      async uploadReleaseAsset(input) {
        state.calls.push('upload');
        const asset = { name: input.name, size: input.data.length, digest: `sha256:${crypto.createHash('sha256').update(input.data).digest('hex')}` };
        if (state.badUpload) asset.digest = 'sha256:wrong';
        state.release.assets.push(asset); return { data: asset };
      },
      async getRelease() { return { data: state.release }; },
      async updateRelease(input) { state.calls.push('publish'); state.published = true; Object.assign(state.release, input); return { data: state.release }; },
    },
  } };
  const context = { eventName: 'push', ref: 'refs/heads/main', sha, repo: { owner: 'fixture', repo: 'fixture' }, payload: { head_commit: { message: 'Release WebsiteBlocker v0.6.0' } } };
  process.chdir(directory);
  try { await run({ github, context, core: { notice() {} } }, state); }
  finally {
    process.chdir(original);
    if (!directory.startsWith(root + path.sep)) throw new Error('Unexpected fixture path');
    fs.rmSync(directory, { recursive: true, force: true });
  }
}

test('release publishes only after every staged asset is uploaded and verified', () => fixture(async (args, state) => {
  await publish(args);
  assert.deepEqual(state.calls, ['create', 'upload', 'upload', 'upload', 'publish']);
  assert.equal(state.release.draft, false);
  assert.equal(state.release.make_latest, 'true');
}));
test('release rejects a changed asset before any remote mutation', () => fixture(async (args, state) => {
  fs.appendFileSync('.release-stage/WebsiteBlocker-v0.6.0.exe', 'changed');
  await assert.rejects(publish(args), /Staged asset changed/);
  assert.deepEqual(state.calls, []);
}));
test('release refuses an existing tag for a different source commit', () => fixture(async (args, state) => {
  state.wrongTag = true;
  await assert.rejects(publish(args), /existing version tag/);
  assert.deepEqual(state.calls, []);
}));
test('release stays a draft when GitHub reports an incorrect upload digest', () => fixture(async (args, state) => {
  state.badUpload = true;
  await assert.rejects(publish(args), /asset verification failed/);
  assert.equal(state.published, false);
  assert.equal(state.release.draft, true);
}));
test('ordinary source commits cannot publish a release', () => fixture(async (args, state) => {
  args.context.payload.head_commit.message = 'Update application styling';
  await assert.rejects(publish(args), /explicit release commit/);
  assert.deepEqual(state.calls, []);
}));
