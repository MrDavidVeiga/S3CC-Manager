import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';

const inventoryPath = path.resolve(process.argv[2] ?? 'reports/reconciliation-20261010-phase2/INVENTARIO_FASE_2_ANTES.json');
const root = path.resolve(process.argv[3] ?? '.');
const inventory = JSON.parse(await fs.readFile(inventoryPath, 'utf8'));
const filterPath = process.argv[4] ? path.resolve(process.argv[4]) : null;
const filter = filterPath ? new Set(JSON.parse(await fs.readFile(filterPath, 'utf8')).records.map(item => item.caminho_manager)) : null;
let verified = 0;
for (const item of inventory.files.filter(item => !filter || filter.has(item.relative_path))) {
  const file = path.join(root, item.relative_path.split('/').join(path.sep));
  const hash = crypto.createHash('sha256').update(await fs.readFile(file)).digest('hex');
  if (hash !== item.sha256) throw new Error(`backup mismatch: ${item.relative_path}`);
  verified += 1;
}
const expected = inventory.files.filter(item => !filter || filter.has(item.relative_path)).length;
console.log(JSON.stringify({ verified, expected, unchanged: verified === expected }, null, 2));
