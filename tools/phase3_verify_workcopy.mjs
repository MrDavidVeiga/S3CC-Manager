import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

const args = process.argv.slice(2);
const value = flag => {
  const index = args.indexOf(flag);
  if (index < 0 || !args[index + 1]) throw new Error(`missing ${flag}`);
  return args[index + 1];
};

const managerRoot = path.resolve(value('--manager'));
const plan = JSON.parse(fs.readFileSync(value('--plan'), 'utf8'));
const decisions = JSON.parse(fs.readFileSync(value('--decisions'), 'utf8'));
const out = value('--out');

const sha256 = file => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
const normalized = relative => relative.split('/').join(path.sep);
const exists = relative => fs.existsSync(path.join(managerRoot, normalized(relative)));

const moved = plan.operations.map(operation => {
  const destination = path.join(managerRoot, normalized(operation.after));
  const source = path.join(managerRoot, normalized(operation.before));
  const present = fs.existsSync(destination);
  return {
    before: operation.before,
    after: operation.after,
    expected_sha256: operation.sha256,
    destination_exists: present,
    source_absent: !fs.existsSync(source),
    sha256: present ? sha256(destination) : null,
    verified: present && !fs.existsSync(source) && sha256(destination) === operation.sha256,
  };
});

const pending = decisions.records
  .filter(record => record.decision === 'REVISÃO_PENDENTE')
  .map(record => ({
    path: record.caminho_manager,
    remains_in_work_copy: exists(record.caminho_manager),
    decision: record.decision,
  }));

const packageFiles = [];
for (const entry of fs.readdirSync(managerRoot, { withFileTypes: true })) {
  const stack = [path.join(managerRoot, entry.name)];
  while (stack.length) {
    const current = stack.pop();
    const stat = fs.statSync(current);
    if (stat.isDirectory()) {
      for (const child of fs.readdirSync(current, { withFileTypes: true })) stack.push(path.join(current, child.name));
    } else if (current.toLowerCase().endsWith('.package')) {
      packageFiles.push(current);
    }
  }
}

const result = {
  generated_at_utc: new Date().toISOString(),
  manager_root: managerRoot,
  package_count: packageFiles.length,
  moved_count: moved.length,
  moved_verified: moved.filter(item => item.verified).length,
  pending_count: pending.length,
  pending,
  all_moved_verified: moved.every(item => item.verified),
  all_pending_preserved: pending.every(item => item.remains_in_work_copy),
  moved,
};

if (!result.all_moved_verified || !result.all_pending_preserved) {
  throw new Error('work-copy verification failed');
}
fs.mkdirSync(out, { recursive: true });
fs.writeFileSync(path.join(out, 'FASE_3_VERIFICACAO_COPIA_TRABALHO.json'), `${JSON.stringify(result, null, 2)}\n`);
console.log(JSON.stringify({
  ok: true,
  package_count: result.package_count,
  moved_verified: result.moved_verified,
  pending_preserved: result.pending.filter(item => item.remains_in_work_copy).length,
}, null, 2));
