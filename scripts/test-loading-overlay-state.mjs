import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, "..");
const read = (relative) => fs.readFileSync(path.join(root, relative), "utf8");

const app = read("src/app.js");
const converter = read("src/sims3pack-converter.js");

// Every polling loop is owned by a unique token.  A completion, cancellation,
// or failure from an older run therefore cannot write into a newer run.
assert.match(app, /const operationMonitorTokens = new Map\(\)/);
assert.match(app, /beginOperationMonitor\("unified_analysis"\)/);
assert.match(app, /invoke\("analyze_unified"/);
assert.match(app, /async function analyzeDuplicates\(\)[\s\S]*runUnifiedAnalysis/);
assert.match(app, /async function analyzeConflicts\(\)[\s\S]*runUnifiedAnalysis/);
assert.doesNotMatch(app, /runAutomaticAnalyses/);
assert.match(app, /while \(operationMonitorIsCurrent\(kind, token\) && operationBusy\(kind\)\)/);
assert.match(app, /await settleOperationStatus\("unified_analysis", operationToken\)/);
assert.match(app, /endOperationMonitor\("unified_analysis", operationToken\)/);
assert.match(app, /\["unified_analysis", "scan", "duplicates", "conflicts"\]/);

// The local lifecycle, rather than a stale backend snapshot, controls
// visibility.  This is what closes the overlay for complete, cancelled and
// failed commands without a timeout.
assert.match(app, /const visible = operationBusy\(kind\);/);
assert.match(app, /if \(message\.includes\("__S3CC_OPERATION_CANCELLED__"\)\)/);

// Continue-in-background is still a real state transition, while terminal
// converter events are identified so delayed events cannot reopen the overlay.
assert.match(app, /loadingOverlayDismissedKey = current\.operationKey/);
assert.match(app, /const canContinue = current\.continueable && !status\.cancelRequested/);
assert.match(app, /const completedExternalOperationIds = new Set\(\)/);
assert.match(app, /completedExternalOperationIds\.has\(operationId\)/);
assert.match(app, /detail\.status\.running === false/);
assert.match(app, /operationId !== currentId/);
assert.match(converter, /state\.operationId = `sims3pack_inspect:/);
assert.match(converter, /state\.operationId = `sims3pack_convert:/);
assert.match(converter, /while \(state\.busy && state\.operationId === operationId\)/);
assert.match(converter, /monitorOperationProgress\("sims3pack_inspect", state\.operationId\)/);
assert.match(converter, /monitorOperationProgress\("sims3pack_convert", state\.operationId\)/);

// Loading copy must describe Manager work in all supported languages, not
// imply that the game world itself is being loaded.
for (const forbidden of [
  "Preparing your neighborhood",
  "Preparando a vizinhança",
  "Preparando el vecindario",
  "world loads",
  "carregar o mundo",
  "cargar el mundo",
]) {
  assert.doesNotMatch(app, new RegExp(forbidden.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")));
}
assert.match(app, /Working through your content/);
assert.match(app, /Processando seus conteúdos/);
assert.match(app, /Procesando tus contenidos/);
assert.match(app, /Reading the files selected in the Manager/);
assert.match(app, /Lendo os arquivos selecionados no Manager/);
assert.match(app, /Leyendo los archivos seleccionados en el Manager/);

console.log("loading overlay lifecycle and localized copy checks passed");
