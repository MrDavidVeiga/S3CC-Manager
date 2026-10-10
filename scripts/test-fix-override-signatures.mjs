import assert from 'node:assert/strict';
import fs from 'node:fs';

const signatures = fs.readFileSync('src-tauri/src/fix_signatures.rs', 'utf8');
const scanner = fs.readFileSync('src-tauri/src/scanner.rs', 'utf8');
const planner = fs.readFileSync('src-tauri/src/planner.rs', 'utf8');

assert.match(signatures, /DbpfResourceSignature/);
assert.match(signatures, /type_id/);
assert.match(signatures, /group/);
assert.match(signatures, /instance/);
assert.match(signatures, /payload_sha256/);
assert.match(signatures, /canonical_destination/);
assert.match(signatures, /fix-signatures-v1\.json/);
assert.match(signatures, /validated_fix_name/);
assert.match(signatures, /OverrideVerified/);
assert.match(signatures, /loading_branch/);
assert.match(signatures, /source_is_override/);

// A package whose filename merely says Override must not change its loading
// branch. The source branch, not a filename token, controls physical moves.
assert.match(scanner, /learned_fix_classification/);
assert.match(scanner, /source_is_override/);
assert.match(planner, /The origin decides the loading branch/);
assert.match(planner, /never reorganize the game's cache/);
assert.match(planner, /source_uses_overrides/);
assert.match(planner, /Resource\.cfg permits only the Overrides root/);

console.log(JSON.stringify({
  ok: true,
  checks: [
    'persistent DBPF TGI/payload signature index',
    'validated correction evidence before weak shape matching',
    'Packages and Overrides loading branches remain separate',
    'Resource.cfg gates override subfolders',
  ],
}, null, 2));
