import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";

const app = readFileSync(new URL("../src/app.js", import.meta.url), "utf8");
const start = app.indexOf("function buildAuditSnapshot() {");
const end = app.indexOf("function buildAuditMarkdown(snapshot) {", start);
assert(start >= 0 && end > start, "audit snapshot builder must be present");

const referenceEvidence = {
  officialOverlapKind: "mixed_official_and_new",
  officialConfidence: "evidence",
  sharedTgiCount: 1,
  identicalResourceCount: 0,
  changedResourceCount: 1,
  unmatchedResourceCount: 2,
  evidence: ["Recursos oficiais: 1 TGI em comum, 1 conteúdo modificado."],
};

const context = vm.createContext({
  Map,
  Date,
  eligibleForPlan: () => true,
  analysisReportStatus: kind => ({ status: kind === "manager" ? "completed" : "not_run", error: "" }),
  state: {
    folder: "C:\\Fixture\\Mods - Copia",
    language: "pt",
    stats: { packages: 1, classified: 1 },
    items: [{
      name: "replacement.package",
      path: "C:\\Fixture\\Mods - Copia\\Packages\\replacement.package",
      relativePath: "Packages\\replacement.package",
      status: "classified",
      classificationConfidence: "high",
      resourceTypes: ["0x0166038C", "IMG"],
      catalogResourceCount: 0,
      resourceCount: 3,
      detectedFrom: ["OfficialReplacementEvidence"],
      candidateDestinations: [],
      fileSize: 42,
      category: "Substituições",
      destinationPath: "Substituições",
      classificationReason: "Changed official TGI",
      referenceEvidence,
      warnings: [],
    }],
    plan: null,
    organizationReview: null,
    conflictsAnalysis: null,
    duplicatesAnalysis: null,
    persistentConflictRecords: [],
    manualOperations: [],
    restorePlan: null,
    quarantinePlan: null,
    cacheInfo: null,
    workspaceStore: null,
    healthReport: null,
    snapshots: [],
    snapshotDiff: null,
    dependenciesAnalysis: null,
    operationHistory: [],
    inboxFolder: null,
    inboxScan: null,
    inboxPlan: null,
    analysisStatus: { manager: "completed", duplicates: "not_run", conflicts: "not_run" },
    error: "",
    duplicatesError: "",
    conflictsError: "",
  },
});

vm.runInContext(app.slice(start, end), context);
const snapshot = JSON.parse(vm.runInContext("JSON.stringify(buildAuditSnapshot())", context));
const item = snapshot.organizer.packages[0];
assert.deepEqual(item.referenceEvidence, referenceEvidence);
assert.equal(item.referenceEvidence.changedResourceCount, 1);
assert.equal(item.referenceEvidence.officialOverlapKind, "mixed_official_and_new");

const eligibilityStart = app.indexOf("function eligibleForPlan(item) {");
const eligibilityEnd = app.indexOf("function statusLabel(status) {", eligibilityStart);
assert(eligibilityStart >= 0 && eligibilityEnd > eligibilityStart, "plan eligibility must be present");
const eligibilityContext = vm.createContext({});
vm.runInContext(`${app.slice(eligibilityStart, eligibilityEnd)}; globalThis.eligible = eligibleForPlan;`, eligibilityContext);
assert.equal(
  vm.runInContext("eligible({ path: 'C:/Fixture/Packages/#Duplicados/a.package.duplicated', status: 'quarantined_duplicate', destinationPath: '#Duplicados' })", eligibilityContext),
  false,
  "inactive duplicate quarantine markers must never enter the ordinary organization plan",
);
assert.equal(
  vm.runInContext("eligible({ path: 'C:/Fixture/Packages/a.package', status: 'unknown' })", eligibilityContext),
  true,
  "unknown active packages retain the explicit #Sem Categoria review path",
);

console.log("Organizer audit evidence and quarantine selection safety: PASS");
