import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';

const reportRoot = path.resolve(process.argv[2] ?? 'reports/reconciliation-20261010-phase2');
const outputRoot = path.resolve(process.argv[3] ?? reportRoot);
const readJson = async name => JSON.parse(await fs.readFile(path.join(reportRoot, name), 'utf8'));

function parseCsv(text) {
  const rows = [];
  let row = [], field = '', quoted = false;
  for (let i = 0; i < text.length; i += 1) {
    const ch = text[i];
    if (quoted) {
      if (ch === '"' && text[i + 1] === '"') { field += '"'; i += 1; }
      else if (ch === '"') quoted = false;
      else field += ch;
    } else if (ch === '"') quoted = true;
    else if (ch === ',') { row.push(field); field = ''; }
    else if (ch === '\n') { row.push(field.replace(/\r$/, '')); rows.push(row); row = []; field = ''; }
    else field += ch;
  }
  if (field.length || row.length) { row.push(field); rows.push(row); }
  const headers = rows.shift();
  return rows.filter(item => item.length === headers.length).map(item => Object.fromEntries(headers.map((header, i) => [header, item[i]])));
}

const phase2 = await readJson('INVENTARIO_FASE_2_ANTES.json');
const model = await readJson('INVENTARIO_FASE_2_MODELO_BASELINE.json');
const managerFileRoot = path.resolve(process.argv[4] ?? phase2.root);
const exceptions = parseCsv(await fs.readFile(path.join(reportRoot, 'EXCECOES_65_ANALISADAS.csv'), 'utf8'))
  .filter(row => row.decisao === 'REVISÃO_PENDENTE');
const managerByPath = new Map(phase2.files.map(file => [file.relative_path, file]));
const modelByPath = new Map(model.files.map(file => [file.relative_path, file]));
const tgi = file => new Set((file.dbpf?.resources ?? []).map(resource => `${resource.type_id}|${resource.group}|${resource.instance}`));
const resourceMap = new Map();
for (const file of model.files.filter(item => item.status === 'active_package')) {
  for (const key of tgi(file)) resourceMap.set(key, [...(resourceMap.get(key) ?? []), file]);
}
const hashCache = new Map();
const fileBuffer = async (root, relative) => {
  const full = path.join(root, relative);
  if (!hashCache.has(full)) hashCache.set(full, await fs.readFile(full));
  return hashCache.get(full);
};
const rawResourceHashes = async (root, file) => {
  const buffer = await fileBuffer(root, file.relative_path);
  return new Map((file.dbpf?.resources ?? []).map(resource => {
    const start = Number(resource.offset);
    const end = start + Number(resource.file_size);
    const bytes = start >= 0 && end <= buffer.length ? buffer.subarray(start, end) : null;
    return [`${resource.type_id}|${resource.group}|${resource.instance}`, bytes ? crypto.createHash('sha256').update(bytes).digest('hex') : null];
  }));
};
const evidenceTokens = file => new Set([
  file.name,
  file.relative_path,
  ...(file.dbpf?.payload_evidence ?? []),
].join(' ').toLowerCase().match(/[a-z0-9][a-z0-9_.-]{3,}/g) ?? []);
const pathTokens = file => new Set(file.relative_path.toLowerCase().split(/[\\/ _#,!&()\[\]-]+/).filter(token => token.length >= 3));
const overlap = (left, right) => [...left].filter(item => right.has(item));
const typeNames = file => new Set((file.dbpf?.resources ?? []).map(resource => resource.type_name ?? resource.type_id));

const results = [];
for (const row of exceptions) {
  const manager = managerByPath.get(row.caminho_manager);
  if (!manager) {
    results.push({ ...row, technical_decision: 'REVISÃO_PENDENTE', reason: 'Registro não encontrado no inventário de fase 2.' });
    continue;
  }
  const managerTgi = tgi(manager);
  const candidateSet = new Set();
  for (const key of managerTgi) for (const candidate of resourceMap.get(key) ?? []) candidateSet.add(candidate.relative_path);
  const managerHashes = await rawResourceHashes(managerFileRoot, manager);
  const managerEvidence = evidenceTokens(manager);
  const managerPath = pathTokens(manager);
  const candidates = [];
  for (const relative of candidateSet) {
    const candidate = modelByPath.get(relative);
    if (!candidate) continue;
    const candidateTgi = tgi(candidate);
    const sharedTgi = overlap(managerTgi, candidateTgi);
    const candidateHashes = await rawResourceHashes(model.root, candidate);
    const rawMatches = sharedTgi.filter(key => managerHashes.get(key) && managerHashes.get(key) === candidateHashes.get(key));
    const evidence = overlap(managerEvidence, evidenceTokens(candidate));
    const family = overlap(managerPath, pathTokens(candidate));
    const types = overlap(typeNames(manager), typeNames(candidate));
    const union = new Set([...managerTgi, ...candidateTgi]).size;
    const jaccard = union ? sharedTgi.length / union : 0;
    const score = rawMatches.length * 8 + sharedTgi.length * 2 + evidence.length * 0.2 + family.length * 0.5 + (jaccard * 10);
    candidates.push({
      path: relative,
      sha256: candidate.sha256,
      size: candidate.size,
      shared_tgi: sharedTgi.length,
      manager_tgi: managerTgi.size,
      candidate_tgi: candidateTgi.size,
      jaccard: Number(jaccard.toFixed(4)),
      raw_resource_hash_matches: rawMatches.length,
      shared_resource_types: types,
      evidence_overlap: evidence.slice(0, 40),
      family_overlap: family,
      score: Number(score.toFixed(3)),
    });
  }
  candidates.sort((a, b) => b.score - a.score || b.raw_resource_hash_matches - a.raw_resource_hash_matches);
  results.push({
    ...row,
    manager_size: manager.size,
    manager_resource_count: manager.dbpf?.resource_count ?? 0,
    manager_resource_types: manager.dbpf?.resource_types ?? [],
    manager_assemblies: (manager.dbpf?.payload_evidence ?? []).filter(token => /assembly|\.dll|\.exe|namespace/i.test(token)),
    candidates: candidates.slice(0, 8),
  });
}

await fs.mkdir(outputRoot, { recursive: true });
await fs.writeFile(path.join(outputRoot, 'FASE_3_ANALISE_47.json'), JSON.stringify({
  generated_at_utc: new Date().toISOString(),
  scope: 'Directed investigation of the 47 phase 2 REVIEW_PENDING records.',
  manager_root: managerFileRoot,
  model_root: model.root,
  records: results,
}, null, 2));
const csvEscape = value => `"${String(value ?? '').replaceAll('"', '""')}"`;
const summaryLines = ['caminho_manager,technical_decision,top_candidate,shared_tgi,raw_resource_hash_matches,score'];
for (const result of results) {
  const top = result.candidates?.[0];
  summaryLines.push([
    result.caminho_manager,
    result.technical_decision ?? 'UNDECIDED',
    top?.path ?? '',
    top?.shared_tgi ?? '',
    top?.raw_resource_hash_matches ?? '',
    top?.score ?? '',
  ].map(csvEscape).join(','));
}
await fs.writeFile(path.join(outputRoot, 'FASE_3_ANALISE_47_RESUMO.csv'), `${summaryLines.join('\n')}\n`);
console.log(JSON.stringify({ records: results.length, output: outputRoot }, null, 2));
