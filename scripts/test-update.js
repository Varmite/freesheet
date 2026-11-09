#!/usr/bin/env node
'use strict';
// Checks how FreeSheet decides that a new version is out (src/js/update.js).
const assert = require('assert');
const path = require('path');
const U = require(path.join(__dirname, '..', 'src', 'js', 'update.js'));

assert(U.isNewer('v1.10.0', '1.9.9'), 'numbers, not text');
assert(U.isNewer('2.0.0', '1.99.99'));
assert(U.isNewer('android-v1.3.0', '1.2.0'));
assert(!U.isNewer('v1.2.0', '1.2.0'));
assert(!U.isNewer('1.1.0', '1.2.0'));
assert(!U.isNewer('nonsense', '1.2.0'));

const rel = (tag, extra) => ({ tag_name: tag, html_url: `${U.RELEASES}tag/${tag}`, draft: false, prerelease: false, ...extra });

// Desktop: one release.
assert.deepStrictEqual(U.interpret(rel('v1.3.0'), '1.2.0', false), { status: 'available', version: '1.3.0', url: `${U.RELEASES}tag/v1.3.0` });
assert.deepStrictEqual(U.interpret(rel('v1.2.0'), '1.2.0', false), { status: 'current' });

// Phones: only the android-v tags count, and the newest one wins.
const list = [rel('v9.0.0'), rel('android-v1.2.5'), rel('android-v1.10.0'), rel('android-v2.0.0', { draft: true }), rel('android-v3.0.0', { prerelease: true })];
assert.strictEqual(U.interpret(list, '1.2.0', true).version, '1.10.0');

// A link that doesn't point at FreeSheet's releases is never used.
assert.strictEqual(U.interpret(rel('v1.3.0', { html_url: 'https://example.com/evil' }), '1.2.0', false).url, `${U.RELEASES}latest`);

// An answer without a release is an error, not "up to date".
assert.throws(() => U.interpret({ message: 'rate limited' }, '1.2.0', false));

// The size offered is the smallest installer, ignoring the checksum list.
assert.strictEqual(U.interpret(rel('v1.3.0', { assets: [{ name: 'SHA256SUMS.txt', size: 900 }, { name: 'a.exe', size: 251658240 }, { name: 'a.AppImage', size: 335544320 }] }), '1.2.0', false).sizeMB, 240);

console.log('Update checks passed.');
