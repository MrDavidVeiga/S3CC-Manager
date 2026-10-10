import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

const scannerPath = path.resolve('src-tauri/src/scanner.rs');
const scanner = fs.readFileSync(scannerPath, 'utf8');
const root = path.resolve('reports/reconciliation-20261010-phase3');

assert.match(scanner, /phase3_directed_family_classification/);
assert.match(scanner, /EPsFixesPath/);
assert.match(scanner, /CmarFamily/);
assert.match(scanner, /LazyDuchessFamily/);
assert.match(scanner, /BoringBonesFamily/);
assert.match(scanner, /path_independent_directed_classification/);
assert.match(scanner, /InternalNMAPFixFamily/);
assert.match(scanner, /InternalNMAPBodyFamily/);
assert.match(scanner, /TYPE_LAYO_LOCAL/);
assert.match(scanner, /CASLayoutIdentifier/);
assert.match(scanner, /Scripts.*Jogabilidade.*NRaas/);
assert.match(scanner, /nraas_family_destination/);

if (!fs.existsSync(root)) {
  console.log(JSON.stringify({
    ok: true,
    skipped: 'private phase 3 corpus is not part of the public checkout',
  }, null, 2));
  process.exit(0);
}

const json = name => JSON.parse(fs.readFileSync(path.join(root, name), 'utf8'));
const analysis = json('FASE_3_ANALISE_47.json');
const decisions = json('FASE_3_DECISOES_INDIVIDUAIS.json');
const plan = json('FASE_3_PLANO_MOVIMENTACOES.json');
const applied = json('FASE_3_MOVIMENTACOES_APLICADAS.json');
const emptyFolders = json('FASE_3_PASTAS_VAZIAS_REMOVIDAS.json');

assert.equal(analysis.records.length, 47);
assert.equal(decisions.counts.total, 47);
assert.equal(decisions.counts.resolved, 42);
assert.equal(decisions.counts.pending, 5);
assert.equal(decisions.counts.moves, 39);
assert.equal(plan.operations.length, 39);
assert.equal(applied.counts.moved_verified, 39);
assert.equal(emptyFolders.folders.length, 15);
assert.equal(new Set(decisions.records.map(record => record.caminho_manager)).size, 47);
assert.equal(new Set(decisions.records.filter(record => record.decision === 'REVISÃO_PENDENTE').map(record => record.caminho_manager)).size, 5);
assert.equal(applied.operations.length, 39);
assert.ok(applied.operations.every(operation => operation.status === 'moved_verified'));

console.log(JSON.stringify({
  ok: true,
  analyzed: analysis.records.length,
  resolved: decisions.counts.resolved,
  pending: decisions.counts.pending,
  moves_verified: applied.counts.moved_verified,
  empty_folders_removed: emptyFolders.folders.length,
}, null, 2));
