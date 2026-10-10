import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';

const args = Object.fromEntries(process.argv.slice(2).reduce((pairs, value, index, all) => {
  if (value.startsWith('--')) pairs.push([value.slice(2), all[index + 1] ?? true]);
  return pairs;
}, []));
if (!args.manager || !args.plan || !args.out || !args.backup) {
  throw new Error('usage: node phase3_apply.mjs --manager PATH --plan PATH --out PATH --backup PATH');
}
const managerRoot = path.resolve(args.manager);
const planRoot = path.resolve(args.plan);
const out = path.resolve(args.out);
const backupRoot = path.resolve(args.backup);
const plan = JSON.parse(await fs.readFile(path.join(planRoot, 'FASE_3_PLANO_MOVIMENTACOES.json'), 'utf8'));
const operations = plan.operations;
const sha = async file => crypto.createHash('sha256').update(await fs.readFile(file)).digest('hex');
const absolute = relative => path.join(managerRoot, relative.split('/').join(path.sep));
const backupAbsolute = relative => path.join(backupRoot, relative.split('/').join(path.sep));

await fs.mkdir(out, { recursive: true });
const seenDestinations = new Set();
for (const item of operations) {
  if (seenDestinations.has(item.after)) throw new Error(`duplicate planned destination: ${item.after}`);
  seenDestinations.add(item.after);
  const source = absolute(item.before);
  const destination = absolute(item.after);
  const original = backupAbsolute(item.before);
  const sourceHash = await sha(source);
  const backupHash = await sha(original);
  if (sourceHash !== item.sha256) throw new Error(`work source changed: ${item.before}`);
  if (backupHash !== item.sha256) throw new Error(`backup mismatch: ${item.before}`);
  try { await fs.access(destination); throw new Error(`destination already exists: ${item.after}`); } catch (error) {
    if (error.message.startsWith('destination already exists')) throw error;
  }
}

const applied = [];
for (const item of operations) {
  const source = absolute(item.before);
  const destination = absolute(item.after);
  await fs.mkdir(path.dirname(destination), { recursive: true });
  await fs.rename(source, destination);
  const movedHash = await sha(destination);
  if (movedHash !== item.sha256) throw new Error(`destination hash mismatch: ${item.after}`);
  applied.push({ ...item, status: 'moved_verified' });
}

const protectedFolder = relative => /(^|[\\/])(?:Fixes|Overrides)(?:[\\/]|$)|EPs Fixes/i.test(relative);
const emptyFolders = [];
async function collect(dir) {
  for (const entry of await fs.readdir(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) { await collect(full); emptyFolders.push(full); }
  }
}
await collect(managerRoot);
const removedFolders = [];
for (const folder of emptyFolders.sort((a, b) => b.length - a.length)) {
  const relative = path.relative(managerRoot, folder).replaceAll(path.sep, '/');
  if (protectedFolder(relative)) continue;
  if ((await fs.readdir(folder)).length === 0) {
    await fs.rmdir(folder);
    removedFolders.push(relative);
  }
}

await fs.writeFile(path.join(out, 'FASE_3_MOVIMENTACOES_APLICADAS.json'), JSON.stringify({
  generated_at_utc: new Date().toISOString(),
  manager_root: managerRoot,
  backup_root: backupRoot,
  operations: applied,
  counts: { moved_verified: applied.length, removed_empty_folders: removedFolders.length },
}, null, 2));
await fs.writeFile(path.join(out, 'FASE_3_PASTAS_VAZIAS_REMOVIDAS.json'), JSON.stringify({
  folders: removedFolders,
  protected_roots_preserved: ['Fixes', 'Overrides', 'EPs Fixes'],
}, null, 2));
console.log(JSON.stringify({ mode: 'APPLIED_TO_WORK_COPY', moved_verified: applied.length, removed_empty_folders: removedFolders.length }, null, 2));
