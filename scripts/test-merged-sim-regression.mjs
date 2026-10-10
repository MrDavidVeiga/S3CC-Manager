import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

const scanner = fs.readFileSync(path.resolve('src-tauri/src/scanner.rs'), 'utf8');
const planner = fs.readFileSync(path.resolve('src-tauri/src/planner.rs'), 'utf8');
const app = fs.readFileSync(path.resolve('src/app.js'), 'utf8');

assert.match(scanner, /TYPE_SIME_LOCAL: u32 = 0x025E_D6F4/);
assert.match(scanner, /TYPE_SIMO_LOCAL: u32 = 0x0341_ACC9/);
assert.match(scanner, /fn merged_sim_export_evidence/);
assert.match(scanner, /fn merged_sim_export_classification/);
assert.match(scanner, /MergedSimExport/);
assert.match(scanner, /embedded payload/);
assert.match(planner, /ready_sim_export_merged_root/);
assert.match(planner, /generic merged-package review/);
assert.match(app, /sim_export_merged/);

const TYPE_SIME = 0x025ed6f4;
const TYPE_SIMO = 0x0341acc9;
const TYPE_CASP = 0x034aeecb;
const TYPE_GEOM = 0x015a1849;

function readKeys(file) {
  const buffer = fs.readFileSync(file);
  const u32 = offset => buffer.readUInt32LE(offset);
  const count = u32(36);
  let offset = u32(64);
  const indexType = u32(offset); offset += 4;
  const common = new Array(8).fill(0);
  for (let field = 0; field < 8; field += 1) {
    if (indexType & (1 << field)) { common[field] = u32(offset); offset += 4; }
  }
  const records = [];
  for (let i = 0; i < count; i += 1) {
    const values = [...common];
    for (let field = 0; field < 8; field += 1) {
      if (!(indexType & (1 << field))) { values[field] = u32(offset); offset += 4; }
    }
    const [type, group, hi, lo] = values;
    if ([TYPE_SIME, TYPE_SIMO].includes(type) || [TYPE_CASP, TYPE_GEOM].includes(type)) {
      records.push({ type, group, instance: `${hi.toString(16).padStart(8, '0')}${lo.toString(16).padStart(8, '0')}` });
    }
  }
  return records;
}

const explicitPaths = process.argv.slice(2);
const backupRoot = process.env.S3CC_PHASE3_BACKUP_ROOT;
const paths = explicitPaths.length >= 2
  ? explicitPaths.slice(0, 2)
  : backupRoot
    ? [path.join(backupRoot, 'cas2.package'), path.join(backupRoot, 'SavannahRosenberg.package')]
    : [];

if (paths.length < 2 || paths.some(file => !fs.existsSync(file))) {
  console.log(JSON.stringify({
    ok: true,
    skipped: 'cas2.package and SavannahRosenberg.package are local regression fixtures; pass their paths or S3CC_PHASE3_BACKUP_ROOT to run binary verification',
  }, null, 2));
  process.exit(0);
}

const results = paths.map(file => {
  const records = readKeys(file);
  const sim = records.filter(record => record.type === TYPE_SIME || record.type === TYPE_SIMO);
  const casp = records.filter(record => record.type === TYPE_CASP);
  const geom = records.filter(record => record.type === TYPE_GEOM);
  assert.ok(sim.length > 0, `${file} must contain SIME/SIMO`);
  assert.ok(casp.length > 0, `${file} must contain CASP payload`);
  assert.ok(geom.length > 0, `${file} must contain GEOM payload`);
  assert.ok(sim.every(record => record.instance !== '0000000000000000'), `${file} has invalid Sim identifier`);
  return {
    file,
    sime: sim.filter(record => record.type === TYPE_SIME).length,
    simo: sim.filter(record => record.type === TYPE_SIMO).length,
    casp: casp.length,
    geom: geom.length,
    identifiers: sim.slice(0, 12).map(record => ({
      type: record.type === TYPE_SIME ? 'SIME' : 'SIMO',
      group: `0x${record.group.toString(16).padStart(8, '0')}`,
      instance: `0x${record.instance}`,
    })),
  };
});

console.log(JSON.stringify({ ok: true, fixtures: results }, null, 2));
