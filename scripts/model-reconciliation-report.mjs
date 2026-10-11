import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';

const [modelRoot, corpusRoot, modelScanPath, corpusScanPath, outputDir] = process.argv.slice(2);
if (![modelRoot, corpusRoot, modelScanPath, corpusScanPath, outputDir].every(Boolean)) {
  throw new Error('usage: node model-reconciliation-report.mjs MODEL_ROOT CORPUS_ROOT MODEL_SCAN.json CORPUS_SCAN.json OUTPUT_DIR');
}

const slash = value => value.replaceAll('/', '\\').split(path.sep).join('\\');
const lower = value => value.toLowerCase();
const isPackage = name => /\.package(?:\.disabled)?$/i.test(name);

async function walk(root, current = root, result = []) {
  for (const entry of await fs.readdir(current, { withFileTypes: true })) {
    const full = path.join(current, entry.name);
    if (entry.isDirectory()) await walk(root, full, result);
    else if (entry.isFile() && isPackage(entry.name)) result.push(full);
  }
  return result;
}

async function sha256(file) {
  return crypto.createHash('sha256').update(await fs.readFile(file)).digest('hex').toUpperCase();
}

async function inventory(root) {
  const files = await walk(root);
  const entries = [];
  for (const file of files) {
    const relative = slash(path.relative(root, file));
    entries.push({ relative, sha256: await sha256(file) });
  }
  return entries.sort((a, b) => lower(a.relative).localeCompare(lower(b.relative)));
}

async function loadScan(file) {
  return JSON.parse((await fs.readFile(file, 'utf8')).replace(/^\uFEFF/, ''));
}

const modelScan = await loadScan(modelScanPath);
const corpusScan = await loadScan(corpusScanPath);
const [modelFiles, corpusFiles] = await Promise.all([inventory(modelRoot), inventory(corpusRoot)]);
const modelItems = new Map(modelScan.items.map(item => [lower(slash(item.relativePath)), item]));
const modelByHash = new Map();
for (const file of modelFiles) {
  if (!modelByHash.has(file.sha256)) modelByHash.set(file.sha256, []);
  modelByHash.get(file.sha256).push(file);
}
const modelDirectoryByKey = new Map(modelFiles.map(file => {
  const parent = path.posix.dirname(file.relative.replaceAll('\\', '/'));
  const original = parent === '.' ? '' : slash(parent);
  return [lower(original), original];
}));
const modelDirectories = new Set(modelDirectoryByKey.keys());
const corpusShaByPath = new Map(corpusFiles.map(file => [lower(file.relative), file.sha256]));

function destinationDirectory(item) {
  return slash(item.destinationPath || '');
}

function rowFor(item) {
  const current = slash(item.relativePath);
  const itemSha = corpusShaByPath.get(lower(current)) || '';
  const currentDirectory = lower(slash(path.posix.dirname(current.replaceAll('\\', '/')) === '.'
    ? '' : path.posix.dirname(current.replaceAll('\\', '/'))));
  const name = path.posix.basename(current.replaceAll('\\', '/'));
  const hashMatches = modelByHash.get(itemSha) || [];
  const directories = [...new Set(hashMatches.map(file => lower(slash(path.posix.dirname(file.relative.replaceAll('\\', '/')) === '.' ? '' : path.posix.dirname(file.relative.replaceAll('\\', '/'))))))];
  const uniqueHashDirectory = directories.length === 1 ? directories[0] : null;
  const classifierDirectory = lower(destinationDirectory(item));
  const canonicalDirectoryKey = uniqueHashDirectory ?? (modelDirectories.has(classifierDirectory) ? classifierDirectory : null);
  const canonicalDirectory = canonicalDirectoryKey === null ? null : modelDirectoryByKey.get(canonicalDirectoryKey);
  const canonicalPath = canonicalDirectory === null ? '' : slash(path.posix.join(canonicalDirectory, name));
  let result = 'REVISAR';
  let method = 'sem_evidencia_modelo_segura';
  if (directories.length > 1) {
    result = 'REVISAR';
    method = 'SHA-256_ambigua_em_destinos_modelo';
  } else if (uniqueHashDirectory !== null) {
    method = 'SHA-256_exato_modelo';
    result = currentDirectory === uniqueHashDirectory ? 'CORRETO' : 'MOVER';
  } else if (canonicalDirectory !== null) {
    method = 'DBPF_regra_geral_com_destino_existente_no_modelo';
    result = currentDirectory === canonicalDirectory ? 'CORRETO' : 'MOVER';
  } else if (item.status === 'invalid') {
    result = 'BLOQUEADO';
    method = 'DBPF_invalido';
  }
  const evidence = [
    `resourceTypes=${(item.resourceTypes || []).join('|')}`,
    `instances=${(item.instances || []).join('|')}`,
    `detectedFrom=${(item.detectedFrom || []).join('|')}`,
    `modelMatches=${hashMatches.map(file => file.relative).join('|')}`,
    item.classificationReason || '',
  ].filter(Boolean).join(' ; ');
  return {
    file: name,
    current_path: current,
    category: [item.category, item.subCategory].filter(Boolean).join('\\'),
    correct_model_path: canonicalPath,
    method,
    evidence,
    confidence: directories.length > 1 ? 'low' : (uniqueHashDirectory !== null ? 'high' : (item.classificationConfidence || 'low')),
    result,
    sha256: itemSha,
  };
}

const rows = corpusScan.items.map(rowFor).sort((a, b) => lower(a.current_path).localeCompare(lower(b.current_path)));
const counts = Object.fromEntries(['CORRETO', 'MOVER', 'REVISAR', 'BLOQUEADO'].map(value => [value, rows.filter(row => row.result === value).length]));
const duplicateGroups = [...new Map(corpusFiles.map(file => [file.sha256, corpusFiles.filter(other => other.sha256 === file.sha256)])).values()]
  .filter(group => group.length > 1)
  .map(group => group.map(file => file.relative));
const report = {
  schema: 's3cc-model-reconciliation-v1',
  generatedAt: new Date().toISOString(),
  modelRoot: 'OFFICIAL_MODEL_PACKAGES_READ_ONLY',
  corpusRoot: 'TEST_CORPUS_READ_ONLY',
  modelPackages: modelFiles.length,
  corpusPackages: corpusFiles.length,
  modelDirectories: modelDirectories.size,
  counts,
  duplicateGroups,
  generalization: {
    exactModelShaMatches: rows.filter(row => row.method === 'SHA-256_exato_modelo').length,
    dbpfRulesWithModelDirectory: rows.filter(row => row.method === 'DBPF_regra_geral_com_destino_existente_no_modelo').length,
    ambiguousModelMatches: rows.filter(row => row.method === 'SHA-256_ambigua_em_destinos_modelo').length,
    unresolvedWithoutSafeDestination: rows.filter(row => row.result === 'REVISAR' || row.result === 'BLOQUEADO').length,
  },
  rows,
};
await fs.mkdir(outputDir, { recursive: true });
await fs.writeFile(path.join(outputDir, 'MODEL_RECONCILIATION_REAL.json'), `${JSON.stringify(report, null, 2)}\n`, 'utf8');
const columns = ['file', 'current_path', 'category', 'correct_model_path', 'method', 'evidence', 'confidence', 'result', 'sha256'];
const csvEscape = value => `"${String(value ?? '').replaceAll('"', '""')}"`;
await fs.writeFile(path.join(outputDir, 'MODEL_RECONCILIATION_REAL.csv'), `${columns.join(',')}\n${rows.map(row => columns.map(column => csvEscape(row[column])).join(',')).join('\n')}\n`, 'utf8');
const markdown = [
  '# Reconciliação real modelo × acervo', '',
  `- Model packages: ${modelFiles.length}`, `- Corpus packages: ${corpusFiles.length}`,
  `- CORRETO: ${counts.CORRETO}`, `- MOVER: ${counts.MOVER}`, `- REVISAR: ${counts.REVISAR}`, `- BLOQUEADO: ${counts.BLOQUEADO}`,
  '', 'A linha detalhada e a evidência DBPF estão no CSV/JSON. Caminhos só são CORRETO quando o diretório relativo completo coincide com o destino canônico.', '',
].join('\n');
await fs.writeFile(path.join(outputDir, 'MODEL_RECONCILIATION_REAL.md'), markdown, 'utf8');
console.log(JSON.stringify({ ok: true, counts, duplicate_groups: duplicateGroups.length }, null, 2));
