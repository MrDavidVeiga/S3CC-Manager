import "@fortawesome/fontawesome-free/css/all.min.css";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

const I18N = {
  en: {
    moreActions: "More Actions",
    selectExactDuplicatesMain: "Select Exact Duplicates",
    viewQuarantine: "View Quarantine",
    selectForReview: "Select for Review",
    reviewSelected: "Review Selected",
    previousConflict: "Previous",
    nextConflict: "Next",
    moreConflictActions: "More review and quarantine actions",
    openOrganizedFolder: "Open Organized Folder",
    analysisNeedsRefresh: "Files moved; analyze again to refresh duplicates and conflicts.",
    remainingLegacyFiles: "files still in original folders",
    remainingLegacyExamples: "Show remaining files (never deleted automatically)",
    disabledPackages: "Disabled",
    noEligibleConflictDecisions: "No eligible conflict decisions to save in this selection.",
    organizeElapsed: "Elapsed",
    selectVisibleExactGroups: "Select Filtered Exact Groups",
    exactOnlySelectionHint: "Clear other selections before using an exact-duplicate batch.",
    markSelectedIntentional: "Mark Selected Intentional",
    markSelectedReviewed: "Mark Selected Reviewed (Session)",
    ignoreSelectedSession: "Ignore Selected (Session)",
    clearSelectedMarks: "Clear Selected Marks",
    reviewedSession: "Reviewed This Session",
    batchMarkedReviewed: "Reviewed for this session",
    batchMarkedIgnored: "Ignored for this session",
    batchMarkedIntentional: "Marked intentional persistently",
    batchMarksCleared: "Selected markings cleared",
    organizer: "Organizer",
    duplicates: "Duplicates",
    conflicts: "Conflicts",
    catalog: "Catalog",
    converter: "Converter",
    maintenance: "Maintenance",
    converterIntro: "Convert Sims3Pack files to .package safely, separately or combined.",
    restore: "Restore",
    language: "Language:",
    languageSelector: "Language",
    languageEnglish: "English",
    languagePortuguese: "Português",
    languageSpanish: "Español",
    theme: "Theme",
    themeSystem: "System",
    themeVeigaLight: "Veiga Light",
    themeVeigaDark: "Veiga Dark",
    themeMeanGirls: "Mean Girls",
    themeEaApp: "EA App",
    themeWinuiLight: "WinUI Light",
    themeWinuiDark: "WinUI Dark",
    themeSystemFootnote: "System theme and accent are read from the operating system.",
    chooseModsFolder: "Choose Mods Folder",
    noFolder: "No folder selected.",
    scanCcs: "Scan CCs",
    clearList: "Clear List",
    scanning: "Scanning packages…",
    progressStarting: "Preparing analysis…",
    progressScanning: "Scanning packages…",
    progressFingerprinting: "Fingerprinting packages…",
    progressIndexing: "Indexing resources…",
    progressComparing: "Comparing candidates…",
    progressCancelling: "Cancelling…",
    analyzeBefore: "Analyze before organizing",
    safetyMessage: "Packages are classified from their actual resources. Review the plan before any move.",
    readOnlyStage: "Analyze first, preview every move, then organize.",
    packages: "Packages",
    classified: "Classified",
    simExportMerged: "Merged Sim export",
    simExportRoot: "Keep whole package at Packages root",
    simExportAlreadyRoot: "Already at Packages root",
    simExportCollision: "Root collision — no overwrite",
    simExportOverride: "Kept in Overrides",
    mixed: "Mixed",
    needsReview: "Needs Review",
    invalid: "Invalid",
    unknown: "Unknown",
    chooseToBegin: "Choose a Mods folder to begin.",
    search: "Search by name or Instance ID",
    filter: "Filter",
    allStatuses: "All statuses",
    selectPackage: "Select a package to see its classification.",
    detectedFrom: "Detected from",
    category: "Category",
    subCategory: "Subcategory",
    gender: "Gender",
    age: "Age",
    species: "Species",
    usageCategories: "Catalog categories",
    suggestedDestination: "Suggested destination",
    possibleDestinations: "Possible destinations",
    resources: "Resources",
    originalPath: "Current path",
    warnings: "Review notes",
    noDestination: "No automatic destination",
    noResults: "No packages match the current filters.",
    scanComplete: "Scan complete",
    scanFailed: "Scan failed",
    previewPlan: "Preview Plan",
    planning: "Building plan…",
    planFailed: "Plan failed",
    selectAllVisible: "Select All",
    selectNoneVisible: "Select None",
    selected: "Selected",
    resourceCfgUpdateLabel: "Extend Resource.cfg for organized subfolders (backup required)",
  resourceCfgNewRules: "rules to add",
  resourceCfgNoUpdate: "No Resource.cfg changes in this plan.",
  resourceCfgAlreadyCovered: "All selected paths are already covered.",
  resourceCfgConfirm: "Resource.cfg will be backed up and updated:",
  organizationPlan: "Organization Plan",
    simulationOnly: "Preflight preview. Nothing moves until you choose Organize Selected.",
    readyToMove: "Ready",
    classifiedReady: "Classified CCs ready",
    exactDuplicatesReady: "Exact duplicates to disable",
    alreadyOrganized: "Already organized",
    reviewPreserved: "Preserved for review",
    unresolvedCollisions: "Unresolved collisions",
    legacyFoldersToRemove: "Legacy folders to remove",
    legacyFoldersRetained: "Legacy folders retained",
    loadingTitle: "Working through your content",
    loadingPhaseIdentification: "Identifying files",
    loadingPhaseAnalysis: "Analyzing packages",
    loadingPhaseDuplicates: "Verifying duplicates",
    loadingPhasePlan: "Preparing the organization plan",
    loadingPhaseOrganization: "Organizing packages",
    loadingPhaseIntegrity: "Checking integrity",
    loadingPhaseCleanup: "Cleaning legacy folders",
    loadingPhaseRestore: "Restoring files",
    loadingPhaseTools: "Working in the background",
    continueInBackground: "Continue in Background",
    loadingPhrase1: "Reading the files selected in the Manager.",
    loadingPhrase2: "Comparing details before preparing the next step.",
    loadingPhrase3: "Checking the collection for a safe, consistent result.",
    loadingPhrase4: "Recording the work for review or restoration.",
    collisions: "Collisions",
    planCollisionsSkipped: "Collisions to Review",
    organizationResult: "Organization Result",
    organizedPackages: "packages organized",
    duplicatesPending: "identical duplicates pending review",
    collisionsPending: "collisions pending review",
    emptyOldFoldersRemoved: "empty folders removed",
    oldFoldersStillOccupied: "source folders still contain files (preserved)",
    reviewDuplicates: "Review Duplicates",
    reviewCollisions: "Review Collisions",
    collisionReviewTitle: "Skipped Collisions",
    collisionReviewIntro: "These packages were left untouched because different files resolve to the same destination.",
    collisionReviewSafety: "No collided file was moved or overwritten.",
    duplicatesSkipped: "Duplicates to Review",
    duplicateSkipped: "Identical duplicate · review",
    moveToNotCategorized: "Keep in place",
    duplicateToReview: "Duplicate · keep in place",
    collisionToReview: "Collision · keep in place",
    blocked: "Blocked",
    foldersToCreate: "Folders",
    manifestPreview: "Restore manifest preview",
    reviewBeforeMove: "Review every change before organizing.",
    close: "Close",
    cancel: "Cancel",
    cancelAnalysis: "Cancel",
    cancelling: "Cancelling…",
    cancelled: "Analysis cancelled",
    technicalDetails: "Show Technical Details",
    hideTechnicalDetails: "Hide Technical Details",
    previewLoading: "Loading thumbnail…",
    previewUnavailable: "No thumbnail available.",
    previewOpenLocation: "Click the thumbnail to open the package location.",
    technicalDetailsLoading: "Reading package resources…",
    technicalDetailsError: "Could not load technical details",
    showingResourceLimit: "Showing the first 500 resources",
    fileSha256: "File SHA-256",
    dbpfVersion: "DBPF version",
    compression: "Compression",
    diskSize: "Disk size",
    memorySize: "Memory size",
    payloadHash: "Payload SHA-256",
    resourceTechnicalDetails: "Resource details",
    meshInformation: "Mesh Information",
    meshHighestPolycount: "Highest visible polycount",
    meshHighestVertices: "Vertices at highest polycount",
    meshLods: "LOD polycounts",
    meshResources: "Mesh resources",
    meshTriangles: "Triangles",
    meshVertices: "Vertices",
    meshGroups: "Groups",
    meshShadow: "Shadow",
    meshUnknownLod: "Unknown LOD",
    meshNoData: "No supported GEOM/MLOD/MODL mesh data found.",
    meshWarnings: "Mesh analysis notes",
    cacheReused: "Cache reused",
    cacheUpdated: "Cache updated",
    restoreHistory: "Restore History",
    restoreHistoryHint: "Manifests previously created for the selected Mods folder.",
    removeRestoreManifest: "Remove Record",
    confirmRemoveQuarantineHistoryTitle: "Remove completed quarantine record?",
    confirmRemoveQuarantineHistoryMessage: "Only the completed quarantine journal will be deleted. No .package files or folders will be removed. Active quarantines and incomplete recoveries are protected.",
    historyRemovalUnavailable: "Restore or recover this Quarantine before removing its history.",
    historyRecordRemoved: "History record removed",
    confirmRemoveRestoreTitle: "Remove restore record?",
    confirmRemoveRestoreMessage: "This permanently removes the selected restore manifest, NOT any .package files. Without this manifest, you cannot undo that organization through Restore.",
    restoreRecordRemoved: "Restore record removed",
    removingRestoreRecord: "Removing restore record…",
    restoreSelectedHint: "Manifest selected. Use Preview Restore to review changes. No files were moved.",
    confirmQuarantineRestoreTitle: "Restore files from Quarantine?",
    confirmQuarantineRestoreMessage: "Files in this quarantine will be moved back to their original paths. Confirm to proceed; no files will be overwritten.",
    refreshHistory: "Refresh",
    noRestoreHistory: "No restore manifests were found for this Mods folder.",
    invalidManifest: "Invalid manifest",
    manifestFiles: "files",
    differentRoot: "Different Mods root",
    cacheTitle: "Analysis Cache",
    cacheNoData: "No cache information.",
    openCacheFolder: "Open Cache",
    clearCache: "Clear Cache",
    fixIndexTitle: "Fix signature index",
    fixIndexNoData: "No model index loaded.",
    rebuildFixIndex: "Rebuild from model",
    fixIndexChooseModel: "Choose the manual model root",
    fixIndexRebuilt: "Fix signature index rebuilt.",
    fixIndexBackupOk: "backup OK",
    fixIndexBackupPending: "backup will be created on rebuild",
    confirmClearCacheTitle: "Clear analysis cache?",
    confirmClearCacheMessage: "Only cached fingerprints will be deleted. Your packages and restore manifests will not be changed.",
    performanceDiagnostics: "Performance",
    scanTime: "Scan",
    totalTime: "Total",
    hashingTime: "Hashing",
    dbpfReadTime: "DBPF read",
    resourceDecodeTime: "Resource decode",
    comparisonTime: "Comparison",
    cacheEntries: "Cache entries",
    cacheSize: "Cache size",
    openPackageLocation: "Open Package Location",
    classificationReason: "Classification evidence",
    classificationConfidence: "Classification confidence",
    creator: "Creator",
    modName: "Mod name",
    gameplayCategory: "Gameplay category",
    confidenceHigh: "High",
    confidenceMedium: "Medium",
    confidenceLow: "Low",
    confidenceManual: "Manual",
    keptUncategorized: "Kept in place",
    openManifestFolder: "Open Manifest Folder",
    manifestRoot: "Manifest root",
    selectedRoot: "Selected Mods root",
    rootMatch: "Roots match",
    rootMismatch: "Different Mods root — Restore blocked",
    intentionalOverride: "Intentional Override",
    markIntentional: "Mark Intentional",
    ignoreSession: "Ignore This Session",
    clearMark: "Clear Mark",
    ignoredSession: "Ignored This Session",
    selectForQuarantine: "Select for quarantine preview",
    previewQuarantine: "Preview Quarantine",
    quarantinePreviewOnly: "Quarantine preflight — review the files and destination before moving.",
    quarantineRoot: "Quarantine destination",
    quarantineReady: "Ready for quarantine",
    executeQuarantine: "Move to Quarantine",
    confirmQuarantineTitle: "Move selected duplicates to Quarantine?",
    confirmQuarantineMessage: "Selected files will be moved outside Packages after a fresh SHA-256 preflight. No file is deleted or overwritten, and failures are rolled back.",
    quarantineComplete: "Quarantine completed",
    restoreQuarantine: "Restore Quarantine",
    quarantineRestored: "Quarantine restored",
    recoverQuarantine: "Recover interrupted Quarantine",
    quarantineRecovered: "Quarantine recovery completed",
    dependencySkippedLarge: "Large resource payloads skipped for safety",
    dependencyTruncated: "Result limit reached; findings are incomplete",
    manualReview: "Manual Review",
    manualDestination: "Approved destination",
    saveManualReview: "Save Review",
    clearManualReview: "Clear Review",
    manualReviewHint: "Use a folder such as CAS\\Sliders or Gameplay\\Other. The decision is stored by SHA-256 and never changes the package.",
    manualReviewSaved: "Manual review saved",
    suggestedDependencyGroup: "Suggested Keep Together Group",
    reviewSuggestedGroup: "Review Group",
    clearSelection: "Clear Selection",
    navigationFailed: "Could not open location",
    savedIntentionalOverride: "Saved Intentional Override",
    auditReport: "Audit Report",
    auditReportHint: "Export classifications, duplicate findings, conflicts and review decisions.",
    auditReportHintOrganizer: "Export package classifications and planned destinations.",
    auditReportHintDuplicates: "Export duplicate groups and related CC variants.",
    auditReportHintConflicts: "Export shared resources, potential conflicts and review decisions.",
    exportAuditReport: "Export Report",
    exportingAuditReport: "Exporting report…",
    auditReportSaved: "Audit report saved",
    auditReportFailed: "Audit export failed",
    openReportFolder: "Open Reports",
    reportNotAnalyzed: "Not analyzed",
    analysisQueued: "Waiting for the previous analysis…",
    analysisStatusLabel: "Analysis status",
    analysisStatus_not_run: "Not executed",
    analysisStatus_running: "In progress",
    analysisStatus_queued: "Queued",
    analysisStatus_completed: "Completed",
    analysisStatus_cancelled: "Cancelled",
    analysisStatus_failed: "Failed",
    analysisStatus_cleared: "Cleared from list",
    analysisErrorLabel: "Analysis error",
    reportGeneratedAt: "Generated at",
    reportDecision: "Review decision",
    confirmRemoveEmptyTitle: "Remove empty folder?",
    confirmRemoveEmptyMessage: "Only this empty folder will be removed. No package file will be deleted.",
    removeEmptyFolderAction: "Remove Empty Folder",
    structure: "Structure",
    structureIntro: "Create folders, move files or folders, and rename folders inside the selected Mods root.",
    structureUp: "Up",
    createFolder: "Create Folder",
    moveSelected: "Move Selected",
    renameFolder: "Rename Folder",
    currentFolder: "Current folder",
    structureChooseRoot: "Choose a Mods folder to manage its structure.",
    selectStructureItem: "Select a file or folder.",
    targetFolder: "Target folder",
    newFolderPath: "New folder path",
    newFolderPathHint: "You can create nested folders, for example: Creator\\Hair\\Female.",
    newFolderName: "New folder name",
    createFolderTitle: "Create folder",
    renameFolderTitle: "Rename folder",
    moveTitle: "Move item",
    createFolderMessage: "The folder will be created inside the current folder. Nested paths are allowed.",
    renameFolderMessage: "Rename the selected folder. Existing destinations are never overwritten.",
    moveMessage: "Choose another folder inside the selected Mods root. Existing destinations are never overwritten.",
    fileType: "File",
    folderType: "Folder",
    itemType: "Type",
    structureSize: "Size",
    structurePath: "Path",
    structureComplete: "Structure updated",
    structureFailed: "Structure operation failed",
    structureNoItems: "This folder is empty.",
    openStructureLocation: "Open Location",
    manualOperations: "Manual structure operations",
    tools: "Tools",
    toolsIntro: "Profiles, health, snapshots, Inbox, metadata, dependencies and technical diagnostics.",
    profilesRules: "Profiles",
    healthResourceCfg: "Health",
    snapshotsCompare: "Snapshots & Compare",
    inboxNewCc: "Inbox",
    metadataGroups: "Metadata",
    technicalLab: "Technical",
    operationHistory: "History",
    workspaceSettings: "Workspace Settings",
    readOnlyMode: "Read-only mode",
    activeProfile: "Active profile",
    profileName: "Profile name",
    addProfile: "Add Profile",
    destinationPrefix: "Destination prefix",
    collapseCategory: "Collapse to main category",
    saveProfile: "Save Profile",
    deleteProfile: "Delete Profile",
    protectedFolders: "Protected Folders",
    customRules: "Custom Rules",
    add: "Add",
    ruleName: "Rule name",
    nameContains: "Name contains",
    pathContains: "Path contains",
    destination: "Destination",
    addRule: "Add Rule",
    analyzeHealth: "Analyze Health",
    resourceCfgViewer: "Resource.cfg Viewer / Load Order",
    healthFindings: "Health Findings",
    healthCategoryFilter: "Category",
    healthSeverityFilter: "Severity",
    healthSelectVisible: "Select visible",
    healthClearSelection: "Clear selection",
    healthExportSelected: "Export selected (JSON)",
    healthSelectedCount: "selected",
    healthAll: "All",
    healthNoFindings: "No findings for this filter.",
    coverageUnknown: "Coverage not verified",
    healthExported: "Health report saved",
    healthSelectVisible: "Select visible", healthClearSelection: "Clear selection", healthExport: "Export selected (JSON)", healthAll: "All findings", healthErrors: "Errors", healthWarnings: "Warnings", healthInformation: "Information", healthSelectedCount: "selected",
    createSnapshot: "Create Snapshot",
    snapshots: "Snapshots",
    compareFolders: "Compare Two Mods Folders",
    chooseFolder: "Choose Folder",
    compare: "Compare",
    chooseInbox: "Choose Inbox Folder",
    analyzeInbox: "Analyze Inbox",
    previewImport: "Preview Import",
    importNewCc: "Import to New CC",
    packageMetadata: "Package Metadata",
    tags: "Tags",
    testStatus: "Test status",
    untested: "Untested",
    working: "Working",
    problemStatus: "Problem",
    removedStatus: "Removed",
    favorite: "Favorite",
    saveMetadata: "Save Metadata",
    keepTogetherGroups: "Keep Together Groups",
    groupName: "Group name",
    createGroup: "Create Group",
    advancedSearch: "Advanced Technical Search",
    searchAction: "Search",
    sideBySideCompare: "Side-by-side Package Compare",
    analyzeDependencies: "Analyze Dependencies",
    beforeAfterTree: "Before → After",
    undoLast: "Undo Last",
    emptyFolders: "Empty folders",
    uncoveredPackages: "Not covered by Resource.cfg",
    outsidePackages: "Packages outside selected root",
    readablePackages: "Readable packages",
    loadOrderViewer: "Load order",
    snapshotCreated: "Snapshot created",
    snapshotCompare: "Compare to current",
    addedFiles: "Added",
    removedFiles: "Removed",
    modifiedFiles: "Modified",
    movedFiles: "Moved",
    inboxSource: "Inbox source",
    noInbox: "No Inbox folder selected.",
    importReady: "Import preview ready",
    technicalSearchHelp: "Use type:, group:, instance:, sha: or free text.",
    dependenciesConservative: "Only exact embedded TGI references in known reference-bearing resources are reported.",
    readOnlyEnabled: "Read-only mode enabled",
    readOnlyDisabled: "Read-only mode disabled",
    profileSaved: "Profile saved",
    metadataSaved: "Metadata saved",
    groupSaved: "Group saved",
    exportSaved: "Export saved",
    noData: "No data.",
    deleteGroup: "Remove Group",
    removeEmptyFolder: "Remove Empty Folder",

    organizeSelected: "Organize Selected",
    alreadyOrganized: "Already organized",
    collisionSame: "Same file exists",
    collisionDifferent: "Different file exists",
    current: "Current",
    proposed: "Proposed",
    noFoldersNeeded: "No new folders are needed.",
    selectedEligible: "eligible selected",
    notEligible: "Not eligible for automatic organization",
    executing: "Organizing selected packages…",
    executionComplete: "Organization completed",
    executionNoChanges: "Nothing needed to be moved",
    executionRolledBack: "Organization failed and was rolled back",
    confirmOrganizeTitle: "Organize selected packages?",
    confirmOrganizeMessage: "A restore manifest will be written before any move. Files will never overwrite existing destinations.",
    organizePartialHint: "Only {ready} ready packages will be moved. {blocked} blocked packages remain untouched for review.",
    restoreIntro: "Choose a restore manifest to preview the exact rollback before changing files.",
    chooseManifest: "Choose Restore Manifest",
    noManifest: "No manifest selected.",
    previewRestore: "Preview Restore",
    executeRestore: "Restore Structure",
    tracked: "Tracked",
    readyToRestore: "Ready",
    newFiles: "New Files",
    chooseManifestToBegin: "Choose a restore manifest to begin.",
    restorePreviewing: "Analyzing restore manifest…",
    restoreReady: "Restore preview ready",
    restoreFailed: "Restore failed",
    restoring: "Restoring previous structure…",
    restoreComplete: "Restore completed",
    confirmRestoreTitle: "Restore previous structure?",
    confirmRestoreMessage: "Tracked files will return to their original paths. Files added later will be moved to Not Categorized. Nothing is overwritten.",
    alreadyRestored: "Already restored",
    readyNew: "New → Not Categorized",
    alreadyUncategorized: "Already Not Categorized",
    changed: "Changed",
    missing: "Missing",
    ambiguous: "Ambiguous",
    duplicatesIntro: "Find true duplicates and distinguish them from related CC variants without deleting anything.",
    analyzeDuplicates: "Analyze Duplicates",
    reanalyzeDuplicates: "Reanalyze Duplicates",
    analyzingDuplicates: "Analyzing package fingerprints…",
    duplicatesReady: "Duplicate analysis complete",
    duplicatesFailed: "Duplicate analysis failed",
    duplicateSearch: "Search by name or Instance ID",
    allDuplicateTypes: "All findings",
    exactDuplicate: "Exact Duplicate",
    contentDuplicate: "Content Duplicate",
    retexture: "Retexture",
    recategorizedVariant: "Recategorized Variant",
    relatedVariant: "Related Variant",
    exactGroups: "Exact Groups",
    contentGroups: "Content Groups",
    retextures: "Retextures",
    relatedVariants: "Related Variants",
    analyzeDuplicatesToBegin: "Scan CCs in Manager to populate this list automatically, or analyze duplicates here.",
    selectDuplicateFinding: "Select a finding to see why it was detected.",
    duplicateDetailsTitle: "Duplicate Details",
    duplicateDetailsIntro: "Compare the detected files, previews and evidence before taking any action.",
    duplicateDetailsSafety: "No file is changed while reviewing duplicate details.",
    conflictDetailsTitle: "Conflict Details",
    conflictDetailsIntro: "Compare both packages, previews and resource evidence before deciding how to handle an overlap.",
    conflictDetailsSafety: "Reviewing a conflict does not change either package.",
    openLocation: "Open Location",
    contentType: "Content Type",
    contentSource: "Source",
    fileSize: "File Size",
    scriptedContent: "Scripted",
    yes: "Yes",
    no: "No",
    duplicateMembers: "Files in this group",
    duplicateReason: "Why it was flagged",
    fileHash: "File SHA-256",
    normalizedFingerprint: "Normalized fingerprint",
    resourceCount: "Resource count",
    sharedResources: "Shared resources",
    identicalTgiPayloads: "Same TGI + same payload",
    changedSameTgi: "Same TGI + different payload",
    sharedStructural: "Shared structural resources",
    changedTextures: "Changed textures/materials",
    changedCatalog: "Changed catalog resources",
    evidence: "Evidence",
    exactExplanation: "The entire .package file has the same SHA-256. These files are byte-for-byte identical.",
    contentExplanation: "The package containers differ, but the normalized set of TGI keys and decompressed resource payload hashes is identical.",
    retextureExplanation: "Structural resources match while texture/material content differs. This is a related visual variant, not a duplicate to delete.",
    recategorizedExplanation: "Substantive non-catalog content matches while catalog resources differ. This is a recategorized variant, not a duplicate to delete.",
    relatedExplanation: "Structural resources match, but the remaining package content differs. It is related content and requires user review.",
    unreadablePackages: "Unreadable packages",
    variantAnalysisTruncated: "Variant relation list was limited for performance.",
    noDuplicateFindings: "No findings match the current search and filter.",
    duplicatesNext: "Duplicate analysis is read-only. Confirmed duplicates may be moved to reversible Quarantine; nothing is deleted automatically.",
    selectExactDuplicates: "Select ALL Exact Duplicates (Keep One Each)",
    selectCurrentDuplicateGroup: "Select Current Exact Group (Keep One)",
    selectAllVisibleConflicts: "Select Visible Conflicts",
    selectAllDetectedConflicts: "Select All Detected Conflicts",
    clearReviewSelection: "Clear Review Selection",
    prioritizeSelectedConflicts: "Prioritize Selected Review",
    conflictReviewSelectedCount: "Conflicts selected for review",
    organizePlanning: "Preparing and checking organization plan…",
    organizePreparing: "Preparing recovery manifest and Resource.cfg…",
    organizeSnapshotting: "Verifying the library and recording recovery hashes…",
    organizeMoving: "Moving and verifying packages…",
    organizeCleaning: "Removing empty subfolders…",
    organizeRollingBack: "Rolling back package changes…",
    organizeFinishing: "Finishing organization…",
    quarantineSelectionCount: "Selected files across all groups",
    quarantineBatchHint: "Only byte-identical copies are selected automatically. Keep one per group and confirm quarantine before moving.",
    bulkExactSummary: "Exact copies selected for Quarantine. One per group stays in the library.",
    bulkExactNone: "No safe exact-duplicate groups available.",
    bulkExactUnsafe: "The selection would remove every copy in an exact-duplicate group. Keep at least one.",
    prioritizeConflicts: "Prioritize Review",
    previewSelectedQuarantine: "Preview Selected Quarantine",
    conflictSelectionHint: "Choose A or B for each conflict. Resource.cfg priority never chooses a file for quarantine automatically.",
    conflictNoSelection: "Choose one or more packages from active conflict pairs first.",
    conflictBothSelected: "Both packages in a conflict were selected. Keep at least one.",
    conflictSelectedSummary: "Manually selected files",
    conflictQuarantineTitle: "Move reviewed conflicting packages to Quarantine?",
    conflictQuarantineMessage: "Only packages explicitly selected by you will move. Load order is not evidence that a file is safe to remove. Review the manifest.",
    conflictPriorityUrgent: "Highest priority",
    conflictPriorityHigh: "High priority",
    conflictPriorityReview: "Review",
    conflictPriorityInfo: "Informational",
    conflictPriorityReason: "Review differing scripts and gameplay resources first; load order is not a deletion recommendation.",
    chooseConflictFile: "Select for Quarantine",
    conflictsIntro: "Compare shared TGIs by decompressed payload and classify the impact instead of treating every overlap as a conflict.",
    analyzeConflicts: "Analyze Conflicts",
    reanalyzeConflicts: "Reanalyze Conflicts",
    analyzingConflicts: "Analyzing shared resources…",
    conflictsReady: "Conflict analysis complete",
    conflictsFailed: "Conflict analysis failed",
    conflictSearch: "Search conflict findings…",
    allConflictTypes: "All findings",
    conflictNeedsReview: "Needs review",
    conflictOutsideLoad: "Outside loading rules",
    conflictInactiveOverlap: "Inactive resource overlap",
    conflictInactiveExplanation: "At least one package is outside the current Resource.cfg loading rules. This overlap is not an active in-game conflict with the current folder layout. Moving the files into a loaded folder could change that.",
    conflictInactiveCount: "Not loaded together",
    conflictDifferentResources: "Different resources",
    conflictIdenticalResources: "Identical resources",
    conflictEvidenceLimited: "The visible resources are a sample; totals include all shared resources.",
    conflictPreviewUnavailable: "No preview available",
    sharedIdentical: "Shared Identical",
    visualOverride: "Visual Override",
    catalogOverride: "Catalog Override",
    gameplayOverride: "Gameplay Override",
    scriptConflict: "Script Conflict",
    textOverride: "Text Override",
    potentialConflict: "Potential Conflict",
    filenameCollisionReview: "Same Name · Different Bytes",
    casMorphOverlap: "CAS Morph Overlap",
    conflictCasMorphExplanation: "Both packages contain different CAS morph data with the same Type, Group and Instance. This requires review, but does not alone prove that either mod is broken.",
    mixedOverride: "Mixed Override",
    packagePairs: "Package Pairs",
    realOverrides: "Needs review",
    scriptConflicts: "Script",
    potentialConflicts: "Potential",
    analyzeConflictsToBegin: "Scan CCs in Manager to populate this list automatically, or analyze conflicts here.",
    selectConflictFinding: "Select a finding to inspect its resource evidence.",
    conflictReason: "Why it was classified",
    conflictSamePayloadExplanation: "These packages share one or more TGIs with byte-identical decompressed payloads. This is shared content, not a conflict.",
    conflictVisualExplanation: "The same visual resource TGI exists in both packages but its decompressed payload differs. This is a visual override relationship.",
    conflictCatalogExplanation: "The same catalog resource TGI exists in both packages but its payload differs. Catalog behavior or categorization may be overridden.",
    conflictGameplayExplanation: "The same gameplay tuning TGI exists in both packages but its payload differs. Load order may change gameplay behavior.",
    conflictScriptExplanation: "The same script assembly TGI exists in both packages with a different payload. This is a high-risk script collision.",
    conflictTextExplanation: "The same STBL TGI exists with different text payloads. This is a text/localization override.",
    conflictPotentialExplanation: "The same TGI exists with a different payload, but its resource family is not yet classified deeply enough for a stronger conclusion.",
    conflictFilenameCollisionExplanation: "The packages have the same filename but different SHA-256 bytes. This is a review finding only; the filename does not prove a shared TGI conflict.",
    conflictMixedExplanation: "This package pair contains multiple classes of differing shared resources.",
    sharedCount: "Shared TGIs",
    samePayloadCount: "Identical payloads",
    differentPayloadCount: "Different payloads",
    resourceClass: "Resource class",
    payloadA: "Payload A",
    payloadB: "Payload B",
    impact: "Impact",
    noConflictFindings: "No findings match the current search and filter.",
    conflictAnalysisTruncated: "Conflict pair list was limited for performance.",
    resourceCfg: "Resource.cfg",
    resourceCfgWarnings: "Resource.cfg warnings",
    loadPriority: "Load priority",
    matchingRule: "Matching rule",
    loadOrder: "Load order",
    higherPriorityWins: "Higher Resource.cfg priority takes precedence for this pair.",
    samePriorityUnknown: "Both packages have the same Resource.cfg priority. The winner is not inferred.",
    partialPriorityUnknown: "Only one package matches Resource.cfg. This pair cannot cause an in-game resource conflict under these loading rules.",
    unmatchedPriorityUnknown: "Neither package matches Resource.cfg. These files may coexist in the scanned folder without being loaded by the game.",
    missingResourceCfg: "No Resource.cfg was found at the selected root or its parent.",
    advancedCfgUnknown: "This Resource.cfg uses advanced traversal or conditional directives. The Manager will not infer a winner from Priority alone.",
    likelyHigherPriority: "Higher priority",
    conflictsNext: "Resource-level conflict analysis is implemented in read-only mode.",
  },
  pt: {
    moreActions: "Mais Ações",
    selectExactDuplicatesMain: "Selecionar Duplicados Exatos",
    viewQuarantine: "Visualizar Quarentena",
    selectForReview: "Selecionar para Revisão",
    reviewSelected: "Revisar Selecionados",
    previousConflict: "Anterior",
    nextConflict: "Próximo",
    moreConflictActions: "Mais ações de revisão e quarentena",
    openOrganizedFolder: "Abrir Pasta Organizada",
    analysisNeedsRefresh: "Arquivos organizados. Analise novamente para atualizar duplicatas e conflitos.",
    remainingLegacyFiles: "arquivos ainda nas pastas originais",
    remainingLegacyExamples: "Ver arquivos restantes (nunca excluídos automaticamente)",
    disabledPackages: "Desativados",
    noEligibleConflictDecisions: "Nenhum conflito selecionado tem dados suficientes para salvar essa decisão.",
    organizeElapsed: "Tempo decorrido",
    selectVisibleExactGroups: "Selecionar Grupos Exatos Filtrados",
    exactOnlySelectionHint: "Limpe as outras seleções antes de selecionar duplicatas exatas em lote.",
    markSelectedIntentional: "Marcar Selecionados como Intencionais",
    markSelectedReviewed: "Marcar Selecionados como Revisados (Sessão)",
    ignoreSelectedSession: "Ignorar Selecionados (Sessão)",
    clearSelectedMarks: "Limpar Marcações dos Selecionados",
    reviewedSession: "Revisado Nesta Sessão",
    batchMarkedReviewed: "Revisados nesta sessão",
    batchMarkedIgnored: "Ignorados nesta sessão",
    batchMarkedIntentional: "Marcados como intencionais permanentemente",
    batchMarksCleared: "Marcações selecionadas removidas",
    organizer: "Organizador",
    duplicates: "Duplicados",
    conflicts: "Conflitos",
    catalog: "Catálogo",
    converter: "Conversor",
    maintenance: "Manutenção",
    converterIntro: "Converta arquivos Sims3Pack para .package com segurança, separadamente ou em conjunto.",
    restore: "Restaurar",
    language: "Idioma:",
    languageSelector: "Idioma",
    languageEnglish: "English",
    languagePortuguese: "Português",
    languageSpanish: "Español",
    theme: "Tema",
    themeSystem: "Sistema",
    themeVeigaLight: "Veiga Claro",
    themeVeigaDark: "Veiga Escuro",
    themeMeanGirls: "Meninas Malvadas",
    themeEaApp: "EA App",
    themeWinuiLight: "WinUI Claro",
    themeWinuiDark: "WinUI Escuro",
    themeSystemFootnote: "O tema e a cor de destaque são obtidos do sistema operacional.",
    chooseModsFolder: "Escolher Pasta de Mods",
    noFolder: "Nenhuma pasta selecionada.",
    scanCcs: "Analisar CCs",
    clearList: "Limpar Lista",
    scanning: "Analisando packages…",
    progressStarting: "Preparando análise…",
    progressScanning: "Analisando packages…",
    progressFingerprinting: "Gerando fingerprints…",
    progressIndexing: "Indexando resources…",
    progressComparing: "Comparando candidatos…",
    progressCancelling: "Cancelando…",
    analyzeBefore: "Analise antes de organizar",
    safetyMessage: "Os packages são classificados pelos resources reais. Revise o plano antes de qualquer movimentação.",
    readOnlyStage: "Analise primeiro, visualize cada movimento e só depois organize.",
    packages: "Packages",
    classified: "Classificados",
    simExportMerged: "Sim exportado (CC mesclado)",
    simExportRoot: "Manter package inteiro na raiz de Packages",
    simExportAlreadyRoot: "Já está na raiz de Packages",
    simExportCollision: "Colisão na raiz — sem sobrescrever",
    simExportOverride: "Mantido em Overrides",
    mixed: "Mistos",
    needsReview: "Requer Revisão",
    invalid: "Inválidos",
    unknown: "Desconhecidos",
    chooseToBegin: "Escolha uma pasta de Mods para começar.",
    search: "Buscar por nome ou Instance ID",
    filter: "Filtro",
    allStatuses: "Todos os estados",
    selectPackage: "Selecione um package para ver a classificação.",
    detectedFrom: "Detectado por",
    category: "Categoria",
    subCategory: "Subcategoria",
    gender: "Gênero",
    age: "Idade",
    species: "Espécie",
    usageCategories: "Categorias do catálogo",
    suggestedDestination: "Destino sugerido",
    possibleDestinations: "Destinos possíveis",
    resources: "Resources",
    originalPath: "Caminho atual",
    warnings: "Notas para revisão",
    noDestination: "Sem destino automático",
    noResults: "Nenhum package corresponde aos filtros atuais.",
    scanComplete: "Análise concluída",
    scanFailed: "Falha na análise",
    previewPlan: "Visualizar Plano",
    planning: "Montando plano…",
    planFailed: "Falha no plano",
    selectAllVisible: "Selecionar Tudo",
    selectNoneVisible: "Selecionar Nenhum",
    selected: "Selecionados",
    resourceCfgUpdateLabel: "Ampliar Resource.cfg para ler subpastas organizadas (com backup)",
  resourceCfgNewRules: "regras a adicionar",
  resourceCfgNoUpdate: "Este plano não altera o Resource.cfg.",
  resourceCfgAlreadyCovered: "Todas as pastas selecionadas já estão cobertas.",
  resourceCfgConfirm: "O Resource.cfg será copiado e atualizado:",
  organizationPlan: "Plano de Organização",
    simulationOnly: "Preview de segurança. Nada será movido até escolher Organizar Selecionados.",
    readyToMove: "Prontos",
    classifiedReady: "CCs classificados prontos",
    exactDuplicatesReady: "Duplicados exatos para desativar",
    alreadyOrganized: "Já organizados",
    reviewPreserved: "Preservados para revisão",
    unresolvedCollisions: "Colisões não resolvidas",
    legacyFoldersToRemove: "Pastas legadas a remover",
    legacyFoldersRetained: "Pastas legadas preservadas",
    loadingTitle: "Processando seus conteúdos",
    loadingPhaseIdentification: "Identificando arquivos",
    loadingPhaseAnalysis: "Analisando packages",
    loadingPhaseDuplicates: "Verificando duplicados",
    loadingPhasePlan: "Preparando o plano de organização",
    loadingPhaseOrganization: "Organizando packages",
    loadingPhaseIntegrity: "Verificando integridade",
    loadingPhaseCleanup: "Limpando pastas legadas",
    loadingPhaseRestore: "Restaurando arquivos",
    loadingPhaseTools: "Trabalhando em segundo plano",
    continueInBackground: "Continuar em Segundo Plano",
    loadingPhrase1: "Lendo os arquivos selecionados no Manager.",
    loadingPhrase2: "Comparando os detalhes antes de preparar a próxima etapa.",
    loadingPhrase3: "Conferindo o acervo para manter um resultado seguro e consistente.",
    loadingPhrase4: "Registrando o trabalho para revisão ou restauração.",
    collisions: "Colisões",
    planCollisionsSkipped: "Colisões para revisar",
    organizationResult: "Resultado da Organização",
    organizedPackages: "packages organizados",
    duplicatesPending: "duplicados idênticos pendentes de revisão",
    collisionsPending: "colisões pendentes de revisão",
    emptyOldFoldersRemoved: "pastas vazias removidas",
    oldFoldersStillOccupied: "pastas de origem ainda contêm arquivos (preservadas)",
    reviewDuplicates: "Revisar Duplicados",
    reviewCollisions: "Revisar Colisões",
    collisionReviewTitle: "Colisões Ignoradas",
    collisionReviewIntro: "Estes packages ficaram intactos porque arquivos diferentes apontam para o mesmo destino.",
    collisionReviewSafety: "Nenhum arquivo em colisão foi movido ou sobrescrito.",
    duplicatesSkipped: "Duplicados para revisar",
    duplicateSkipped: "Duplicado idêntico · revisar",
    moveToNotCategorized: "Manter no lugar",
    duplicateToReview: "Duplicado · manter no lugar",
    collisionToReview: "Colisão · manter no lugar",
    blocked: "Bloqueados",
    foldersToCreate: "Pastas",
    manifestPreview: "Preview do manifesto de restauração",
    reviewBeforeMove: "Revise todas as alterações antes de organizar.",
    close: "Fechar",
    cancel: "Cancelar",
    cancelAnalysis: "Cancelar",
    cancelling: "Cancelando…",
    cancelled: "Análise cancelada",
    technicalDetails: "Mostrar Detalhes Técnicos",
    hideTechnicalDetails: "Ocultar Detalhes Técnicos",
    previewLoading: "Carregando thumbnail…",
    previewUnavailable: "Thumbnail não disponível.",
    previewOpenLocation: "Clique na thumbnail para abrir a localização do package.",
    technicalDetailsLoading: "Lendo resources do package…",
    technicalDetailsError: "Não foi possível carregar os detalhes técnicos",
    showingResourceLimit: "Exibindo os primeiros 500 resources",
    fileSha256: "SHA-256 do arquivo",
    dbpfVersion: "Versão DBPF",
    compression: "Compressão",
    diskSize: "Tamanho no disco",
    memorySize: "Tamanho em memória",
    payloadHash: "SHA-256 do payload",
    resourceTechnicalDetails: "Detalhes dos resources",
    meshInformation: "Informações da Mesh",
    meshHighestPolycount: "Maior polycount visível",
    meshHighestVertices: "Vértices no maior polycount",
    meshLods: "Polycount por LOD",
    meshResources: "Resources de mesh",
    meshTriangles: "Triângulos",
    meshVertices: "Vértices",
    meshGroups: "Grupos",
    meshShadow: "Sombra",
    meshUnknownLod: "LOD desconhecido",
    meshNoData: "Nenhum dado de mesh GEOM/MLOD/MODL compatível encontrado.",
    meshWarnings: "Notas da análise de mesh",
    cacheReused: "Cache reutilizado",
    cacheUpdated: "Cache atualizado",
    restoreHistory: "Histórico de Restauração",
    restoreHistoryHint: "Manifestos criados anteriormente para a pasta de Mods selecionada.",
    removeRestoreManifest: "Remover Registro",
    confirmRemoveQuarantineHistoryTitle: "Remover registro da Quarentena concluída?",
    confirmRemoveQuarantineHistoryMessage: "Somente o registro da quarentena concluída será excluído. Nenhum arquivo .package ou pasta será removido. Quarentenas ativas ou com recuperação pendente estão protegidas.",
    historyRemovalUnavailable: "Restaure ou recupere esta Quarentena antes de remover o histórico.",
    historyRecordRemoved: "Registro do histórico removido",
    confirmRemoveRestoreTitle: "Remover registro de restauração?",
    confirmRemoveRestoreMessage: "Isso remove permanentemente apenas o manifesto selecionado, NÃO os arquivos .package. Sem esse manifesto, não será possível desfazer aquela organização pelo Restore.",
    restoreRecordRemoved: "Registro de restauração removido",
    removingRestoreRecord: "Removendo registro de restauração…",
    restoreSelectedHint: "Manifesto selecionado. Use Visualizar Restauração para revisar as alterações. Nenhum arquivo foi movido.",
    confirmQuarantineRestoreTitle: "Restaurar arquivos da Quarentena?",
    confirmQuarantineRestoreMessage: "Os arquivos dessa quarentena voltarão aos caminhos originais. Confirme para continuar; nenhum arquivo será sobrescrito.",
    refreshHistory: "Atualizar",
    noRestoreHistory: "Nenhum manifesto de restauração foi encontrado para esta pasta de Mods.",
    invalidManifest: "Manifesto inválido",
    manifestFiles: "arquivos",
    differentRoot: "Outra pasta de Mods",
    cacheTitle: "Cache de Análise",
    cacheNoData: "Sem informações de cache.",
    openCacheFolder: "Abrir Cache",
    clearCache: "Limpar Cache",
    fixIndexTitle: "Índice de assinaturas de Fixes",
    fixIndexNoData: "Nenhum índice do modelo carregado.",
    rebuildFixIndex: "Reconstruir a partir do modelo",
    fixIndexChooseModel: "Escolha a raiz do modelo manual",
    fixIndexRebuilt: "Índice de assinaturas reconstruído.",
    fixIndexBackupOk: "backup OK",
    fixIndexBackupPending: "backup será criado na próxima reconstrução",
    confirmClearCacheTitle: "Limpar o cache de análise?",
    confirmClearCacheMessage: "Somente os fingerprints em cache serão apagados. Seus packages e manifestos de restauração não serão alterados.",
    performanceDiagnostics: "Desempenho",
    scanTime: "Análise",
    totalTime: "Total",
    hashingTime: "Hashing",
    dbpfReadTime: "Leitura DBPF",
    resourceDecodeTime: "Descompressão/resources",
    comparisonTime: "Comparação",
    cacheEntries: "Itens no cache",
    cacheSize: "Tamanho do cache",
    openPackageLocation: "Abrir Localização do Package",
    classificationReason: "Evidência da classificação",
    classificationConfidence: "Confiança da classificação",
    creator: "Criador",
    modName: "Nome do mod",
    gameplayCategory: "Categoria de jogabilidade",
    confidenceHigh: "Alta",
    confidenceMedium: "Média",
    confidenceLow: "Baixa",
    confidenceManual: "Manual",
    keptUncategorized: "Mantidos no lugar",
    openManifestFolder: "Abrir Pasta do Manifesto",
    manifestRoot: "Raiz do manifesto",
    selectedRoot: "Raiz de Mods selecionada",
    rootMatch: "Raízes correspondem",
    rootMismatch: "Outra pasta de Mods — restauração bloqueada",
    intentionalOverride: "Override Intencional",
    markIntentional: "Marcar como Intencional",
    ignoreSession: "Ignorar Nesta Sessão",
    clearMark: "Limpar Marcação",
    ignoredSession: "Ignorado Nesta Sessão",
    selectForQuarantine: "Selecionar para preview de quarentena",
    previewQuarantine: "Visualizar Quarentena",
    quarantinePreviewOnly: "Preflight da quarentena — revise os arquivos e o destino antes de mover.",
    quarantineRoot: "Destino da quarentena",
    quarantineReady: "Prontos para quarentena",
    executeQuarantine: "Mover para Quarentena",
    confirmQuarantineTitle: "Mover os duplicados selecionados para a Quarentena?",
    confirmQuarantineMessage: "Os arquivos selecionados serão movidos para fora de Packages após um novo preflight de SHA-256. Nada será apagado ou sobrescrito e falhas serão revertidas.",
    quarantineComplete: "Quarentena concluída",
    restoreQuarantine: "Restaurar Quarentena",
    quarantineRestored: "Quarentena restaurada",
    recoverQuarantine: "Recuperar Quarentena interrompida",
    quarantineRecovered: "Recuperação da Quarentena concluída",
    dependencySkippedLarge: "Recursos muito grandes ignorados por segurança",
    dependencyTruncated: "Limite de resultados atingido; análise incompleta",
    manualReview: "Revisão Manual",
    manualDestination: "Destino aprovado",
    saveManualReview: "Salvar Revisão",
    clearManualReview: "Limpar Revisão",
    manualReviewHint: "Use uma pasta como CAS\\Sliders ou Jogabilidade\\Outros. A decisão é salva pelo SHA-256 e nunca altera o package.",
    manualReviewSaved: "Revisão manual salva",
    suggestedDependencyGroup: "Grupo Manter Juntos Sugerido",
    reviewSuggestedGroup: "Revisar Grupo",
    clearSelection: "Limpar Seleção",
    navigationFailed: "Não foi possível abrir a localização",
    savedIntentionalOverride: "Override Intencional Salvo",
    auditReport: "Relatório de Auditoria",
    auditReportHint: "Exporte classificações, duplicados, conflitos e decisões de revisão.",
    auditReportHintOrganizer: "Exporte a categorização dos packages e seus destinos.",
    auditReportHintDuplicates: "Exporte os grupos de duplicatas e variantes relacionadas.",
    auditReportHintConflicts: "Exporte os recursos compartilhados, conflitos e decisões de revisão.",
    exportAuditReport: "Exportar Relatório",
    exportingAuditReport: "Exportando relatório…",
    auditReportSaved: "Relatório de auditoria salvo",
    auditReportFailed: "Falha ao exportar auditoria",
    openReportFolder: "Abrir Relatórios",
    reportNotAnalyzed: "Não analisado",
    analysisQueued: "Aguardando a análise anterior…",
    analysisStatusLabel: "Estado da análise",
    analysisStatus_not_run: "Não executada",
    analysisStatus_running: "Em andamento",
    analysisStatus_queued: "Na fila",
    analysisStatus_completed: "Concluída",
    analysisStatus_cancelled: "Cancelada",
    analysisStatus_failed: "Falhou",
    analysisStatus_cleared: "Removida da lista",
    analysisErrorLabel: "Erro da análise",
    reportGeneratedAt: "Gerado em",
    reportDecision: "Decisão de revisão",
    confirmRemoveEmptyTitle: "Remover pasta vazia?",
    confirmRemoveEmptyMessage: "Somente esta pasta vazia será removida. Nenhum arquivo package será apagado.",
    removeEmptyFolderAction: "Remover Pasta Vazia",
    structure: "Estrutura",
    structureIntro: "Crie pastas, mova arquivos ou pastas e renomeie pastas dentro da raiz de Mods selecionada.",
    structureUp: "Subir",
    createFolder: "Criar Pasta",
    moveSelected: "Mover Selecionado",
    renameFolder: "Renomear Pasta",
    currentFolder: "Pasta atual",
    structureChooseRoot: "Escolha uma pasta de Mods para gerenciar sua estrutura.",
    selectStructureItem: "Selecione um arquivo ou uma pasta.",
    targetFolder: "Pasta de destino",
    newFolderPath: "Caminho da nova pasta",
    newFolderPathHint: "Você pode criar pastas aninhadas, por exemplo: Criador\\Cabelos\\Feminino.",
    newFolderName: "Novo nome da pasta",
    createFolderTitle: "Criar pasta",
    renameFolderTitle: "Renomear pasta",
    moveTitle: "Mover item",
    createFolderMessage: "A pasta será criada dentro da pasta atual. Caminhos aninhados são permitidos.",
    renameFolderMessage: "Renomeie a pasta selecionada. Destinos existentes nunca são sobrescritos.",
    moveMessage: "Escolha outra pasta dentro da raiz de Mods selecionada. Destinos existentes nunca são sobrescritos.",
    fileType: "Arquivo",
    folderType: "Pasta",
    itemType: "Tipo",
    structureSize: "Tamanho",
    structurePath: "Caminho",
    structureComplete: "Estrutura atualizada",
    structureFailed: "Falha na operação de estrutura",
    structureNoItems: "Esta pasta está vazia.",
    openStructureLocation: "Abrir Localização",
    manualOperations: "Operações manuais de estrutura",
    tools: "Ferramentas",
    toolsIntro: "Perfis, saúde, snapshots, Inbox, metadados, dependências e diagnósticos técnicos.",
    profilesRules: "Perfis",
    healthResourceCfg: "Saúde",
    snapshotsCompare: "Snapshots e Comparação",
    inboxNewCc: "Inbox",
    metadataGroups: "Metadados",
    technicalLab: "Técnico",
    operationHistory: "Histórico",
    workspaceSettings: "Configurações do Workspace",
    readOnlyMode: "Modo somente leitura",
    activeProfile: "Perfil ativo",
    profileName: "Nome do perfil",
    addProfile: "Adicionar Perfil",
    destinationPrefix: "Prefixo de destino",
    collapseCategory: "Reduzir à categoria principal",
    saveProfile: "Salvar Perfil",
    deleteProfile: "Excluir Perfil",
    protectedFolders: "Pastas Protegidas",
    customRules: "Regras Personalizadas",
    add: "Adicionar",
    ruleName: "Nome da regra",
    nameContains: "Nome contém",
    pathContains: "Caminho contém",
    destination: "Destino",
    addRule: "Adicionar Regra",
    analyzeHealth: "Analisar Saúde",
    resourceCfgViewer: "Resource.cfg / Ordem de Carregamento",
    healthFindings: "Resultados de Saúde",
    healthCategoryFilter: "Categoria",
    healthSeverityFilter: "Gravidade",
    healthSelectVisible: "Selecionar visíveis",
    healthClearSelection: "Limpar seleção",
    healthExportSelected: "Exportar selecionados (JSON)",
    healthSelectedCount: "selecionados",
    healthAll: "Todos",
    healthNoFindings: "Nenhum resultado neste filtro.",
    coverageUnknown: "Cobertura não verificada",
    healthExported: "Relatório de saúde salvo",
    healthSelectVisible: "Selecionar visíveis", healthClearSelection: "Limpar seleção", healthExport: "Exportar selecionados (JSON)", healthAll: "Todos os resultados", healthErrors: "Erros", healthWarnings: "Avisos", healthInformation: "Informações", healthSelectedCount: "selecionados",
    createSnapshot: "Criar Snapshot",
    snapshots: "Snapshots",
    compareFolders: "Comparar Duas Pastas de Mods",
    chooseFolder: "Escolher Pasta",
    compare: "Comparar",
    chooseInbox: "Escolher Pasta Inbox",
    analyzeInbox: "Analisar Inbox",
    previewImport: "Visualizar Importação",
    importNewCc: "Importar para New CC",
    packageMetadata: "Metadados do Package",
    tags: "Tags",
    testStatus: "Status de teste",
    untested: "Não testado",
    working: "Funcionando",
    problemStatus: "Com problema",
    removedStatus: "Removido",
    favorite: "Favorito",
    saveMetadata: "Salvar Metadados",
    keepTogetherGroups: "Grupos Manter Juntos",
    groupName: "Nome do grupo",
    createGroup: "Criar Grupo",
    advancedSearch: "Busca Técnica Avançada",
    searchAction: "Pesquisar",
    sideBySideCompare: "Comparar Packages Lado a Lado",
    analyzeDependencies: "Analisar Dependências",
    beforeAfterTree: "Antes → Depois",
    undoLast: "Desfazer Última",
    emptyFolders: "Pastas vazias",
    uncoveredPackages: "Não cobertos pelo Resource.cfg",
    outsidePackages: "Packages fora da raiz selecionada",
    readablePackages: "Packages legíveis",
    loadOrderViewer: "Ordem de carregamento",
    snapshotCreated: "Snapshot criado",
    snapshotCompare: "Comparar com atual",
    addedFiles: "Adicionados",
    removedFiles: "Removidos",
    modifiedFiles: "Modificados",
    movedFiles: "Movidos",
    inboxSource: "Origem da Inbox",
    noInbox: "Nenhuma pasta Inbox selecionada.",
    importReady: "Preview da importação pronto",
    technicalSearchHelp: "Use type:, group:, instance:, sha: ou texto livre.",
    dependenciesConservative: "Somente referências TGI exatas embutidas em resources conhecidos como portadores de referências são mostradas.",
    readOnlyEnabled: "Modo somente leitura ativado",
    readOnlyDisabled: "Modo somente leitura desativado",
    profileSaved: "Perfil salvo",
    metadataSaved: "Metadados salvos",
    groupSaved: "Grupo salvo",
    exportSaved: "Exportação salva",
    noData: "Sem dados.",
    deleteGroup: "Remover Grupo",
    removeEmptyFolder: "Remover Pasta Vazia",

    organizeSelected: "Organizar Selecionados",
    alreadyOrganized: "Já organizado",
    collisionSame: "Arquivo idêntico já existe",
    collisionDifferent: "Arquivo diferente já existe",
    current: "Atual",
    proposed: "Proposto",
    noFoldersNeeded: "Nenhuma nova pasta precisa ser criada.",
    selectedEligible: "aptos selecionados",
    notEligible: "Não elegível para organização automática",
    executing: "Organizando packages selecionados…",
    executionComplete: "Organização concluída",
    executionNoChanges: "Nenhum arquivo precisava ser movido",
    executionRolledBack: "A organização falhou e foi revertida",
    confirmOrganizeTitle: "Organizar os packages selecionados?",
    confirmOrganizeMessage: "Um manifesto de restauração será salvo antes de qualquer movimento. Nenhum arquivo sobrescreverá um destino existente.",
    organizePartialHint: "Apenas {ready} packages prontos serão movidos. Os {blocked} bloqueados permanecerão no lugar para revisão.",
    restoreIntro: "Escolha um manifesto de restauração para visualizar exatamente o que será desfeito antes de alterar os arquivos.",
    chooseManifest: "Escolher Manifesto de Restauração",
    noManifest: "Nenhum manifesto selecionado.",
    previewRestore: "Visualizar Restauração",
    executeRestore: "Restaurar Estrutura",
    tracked: "Rastreados",
    readyToRestore: "Prontos",
    newFiles: "Arquivos Novos",
    chooseManifestToBegin: "Escolha um manifesto de restauração para começar.",
    restorePreviewing: "Analisando manifesto de restauração…",
    restoreReady: "Preview da restauração pronto",
    restoreFailed: "Falha na restauração",
    restoring: "Restaurando estrutura anterior…",
    restoreComplete: "Restauração concluída",
    confirmRestoreTitle: "Restaurar a estrutura anterior?",
    confirmRestoreMessage: "Os arquivos rastreados voltarão aos caminhos originais. Arquivos adicionados depois irão para Não Categorizado. Nada será sobrescrito.",
    alreadyRestored: "Já restaurado",
    readyNew: "Novo → Não Categorizado",
    alreadyUncategorized: "Já em Não Categorizado",
    changed: "Alterado",
    missing: "Ausente",
    ambiguous: "Ambíguo",
    duplicatesIntro: "Encontre duplicados reais e diferencie-os de variantes relacionadas de CC sem apagar nada.",
    analyzeDuplicates: "Analisar Duplicados",
    reanalyzeDuplicates: "Reanalisar Duplicados",
    analyzingDuplicates: "Analisando fingerprints dos packages…",
    duplicatesReady: "Análise de duplicados concluída",
    duplicatesFailed: "Falha na análise de duplicados",
    duplicateSearch: "Pesquisar resultados de duplicados…",
    allDuplicateTypes: "Todos os resultados",
    exactDuplicate: "Duplicado Exato",
    contentDuplicate: "Duplicado por Conteúdo",
    retexture: "Retexture",
    recategorizedVariant: "Variante Recategorizada",
    relatedVariant: "Variante Relacionada",
    exactGroups: "Grupos Exatos",
    contentGroups: "Grupos por Conteúdo",
    retextures: "Retextures",
    relatedVariants: "Variantes Relacionadas",
    analyzeDuplicatesToBegin: "Use Analisar CCs no Manager para preencher esta lista automaticamente ou analise duplicados aqui.",
    selectDuplicateFinding: "Selecione um resultado para ver por que ele foi detectado.",
    duplicateDetailsTitle: "Detalhes dos Duplicados",
    duplicateDetailsIntro: "Compare os arquivos detectados, previews e evidências antes de realizar qualquer ação.",
    duplicateDetailsSafety: "Nenhum arquivo é alterado durante a revisão dos duplicados.",
    conflictDetailsTitle: "Detalhes dos Conflitos",
    conflictDetailsIntro: "Compare os dois packages, as prévias e as evidências dos recursos antes de decidir como tratar uma sobreposição.",
    conflictDetailsSafety: "Revisar um conflito não altera nenhum package.",
    openLocation: "Abrir Local",
    contentType: "Tipo de Conteúdo",
    contentSource: "Origem",
    fileSize: "Tamanho do Arquivo",
    scriptedContent: "Script",
    yes: "Sim",
    no: "Não",
    duplicateMembers: "Arquivos deste grupo",
    duplicateReason: "Por que foi marcado",
    fileHash: "SHA-256 do arquivo",
    normalizedFingerprint: "Fingerprint normalizado",
    resourceCount: "Quantidade de resources",
    sharedResources: "Resources compartilhados",
    identicalTgiPayloads: "Mesmo TGI + mesmo conteúdo",
    changedSameTgi: "Mesmo TGI + conteúdo diferente",
    sharedStructural: "Resources estruturais compartilhados",
    changedTextures: "Texturas/materiais alterados",
    changedCatalog: "Resources de catálogo alterados",
    evidence: "Evidências",
    exactExplanation: "O arquivo .package inteiro possui o mesmo SHA-256. Esses arquivos são idênticos byte por byte.",
    contentExplanation: "Os containers dos packages diferem, mas o conjunto normalizado de TGIs e hashes dos resources descomprimidos é idêntico.",
    retextureExplanation: "Os resources estruturais coincidem, enquanto texturas ou materiais diferem. É uma variante visual relacionada, não um duplicado para apagar.",
    recategorizedExplanation: "O conteúdo substancial fora do catálogo coincide, enquanto os resources de catálogo diferem. É uma variante recategorizada, não um duplicado para apagar.",
    relatedExplanation: "Os resources estruturais coincidem, mas o restante do conteúdo do package difere. É conteúdo relacionado e requer revisão.",
    unreadablePackages: "Packages não legíveis",
    variantAnalysisTruncated: "A lista de relações entre variantes foi limitada por desempenho.",
    noDuplicateFindings: "Nenhum resultado corresponde à pesquisa e ao filtro atuais.",
    duplicatesNext: "A análise de duplicados é somente leitura. Duplicados confirmados podem ser movidos para uma Quarentena reversível; nada é apagado automaticamente.",
    selectExactDuplicates: "Selecionar TODOS os duplicados exatos",
    selectCurrentDuplicateGroup: "Selecionar somente este grupo (manter um)",
    selectAllVisibleConflicts: "Selecionar conflitos visíveis",
    selectAllDetectedConflicts: "Selecionar TODOS os conflitos detectados",
    clearReviewSelection: "Limpar seleção de revisão",
    prioritizeSelectedConflicts: "Priorizar revisão dos selecionados",
    conflictReviewSelectedCount: "Conflitos selecionados para revisão",
    organizePlanning: "Preparando e conferindo o plano de organização…",
    organizePreparing: "Preparando o manifesto de recuperação e Resource.cfg…",
    organizeSnapshotting: "Verificando o acervo e registrando hashes para recuperação…",
    organizeMoving: "Movendo e verificando os packages…",
    organizeCleaning: "Removendo todas as subpastas vazias…",
    organizeRollingBack: "Revertendo movimentações…",
    organizeFinishing: "Finalizando organização…",
    quarantineSelectionCount: "Arquivos selecionados em todos os grupos",
    quarantineBatchHint: "Somente cópias idênticas são selecionadas automaticamente. Uma por grupo permanece no local. Revise a quarentena antes de mover.",
    bulkExactSummary: "Cópias exatas selecionadas para Quarentena. Uma cópia de cada grupo permanece na biblioteca.",
    bulkExactNone: "Nenhum grupo seguro de duplicatas exatas disponível.",
    bulkExactUnsafe: "A seleção deixaria um grupo de duplicatas exatas sem nenhuma cópia. Mantenha pelo menos uma.",
    prioritizeConflicts: "Priorizar revisão",
    previewSelectedQuarantine: "Visualizar quarentena da seleção",
    conflictSelectionHint: "Escolha A ou B em cada conflito. A prioridade do Resource.cfg nunca escolhe automaticamente o arquivo para quarentena.",
    conflictNoSelection: "Escolha um ou mais packages de conflitos ativos.",
    conflictBothSelected: "Ambos os packages de um conflito estão selecionados. Mantenha pelo menos um.",
    conflictSelectedSummary: "Arquivos selecionados manualmente",
    conflictQuarantineTitle: "Mover os packages de conflito revisados para a Quarentena?",
    conflictQuarantineMessage: "Só serão movidos os packages selecionados por você. A ordem de carregamento não prova que um arquivo pode ser removido. Confira o manifesto.",
    conflictPriorityUrgent: "Prioridade máxima",
    conflictPriorityHigh: "Prioridade alta",
    conflictPriorityReview: "Revisar",
    conflictPriorityInfo: "Informativo",
    conflictPriorityReason: "Revise primeiro diferenças em scripts e gameplay; ordem de carregamento não é recomendação de exclusão.",
    chooseConflictFile: "Selecionar para quarentena",
    conflictsIntro: "Compare TGIs compartilhados pelo payload descomprimido e classifique o impacto em vez de tratar toda sobreposição como conflito.",
    analyzeConflicts: "Analisar Conflitos",
    reanalyzeConflicts: "Reanalisar Conflitos",
    analyzingConflicts: "Analisando resources compartilhados…",
    conflictsReady: "Análise de conflitos concluída",
    conflictsFailed: "Falha na análise de conflitos",
    conflictSearch: "Pesquisar resultados de conflitos…",
    allConflictTypes: "Todos os resultados",
    conflictNeedsReview: "Exige revisão",
    conflictOutsideLoad: "Fora das regras de carregamento",
    conflictInactiveOverlap: "Sobreposição inativa de recursos",
    conflictInactiveExplanation: "Pelo menos um package está fora das regras de carregamento do Resource.cfg atual. Esta sobreposição não é um conflito ativo no jogo com a organização atual. Mover os arquivos para pastas carregadas pode mudar isso.",
    conflictInactiveCount: "Não carregados juntos",
    conflictDifferentResources: "Recursos diferentes",
    conflictIdenticalResources: "Recursos idênticos",
    conflictEvidenceLimited: "Os recursos exibidos são uma amostra; os totais incluem todos os recursos compartilhados.",
    conflictPreviewUnavailable: "Prévia indisponível",
    sharedIdentical: "Compartilhado Idêntico",
    visualOverride: "Override Visual",
    catalogOverride: "Override de Catálogo",
    gameplayOverride: "Override de Gameplay",
    scriptConflict: "Conflito de Script",
    textOverride: "Override de Texto",
    potentialConflict: "Conflito Potencial",
    filenameCollisionReview: "Mesmo Nome · Bytes Diferentes",
    casMorphOverlap: "Sobreposição de Morfologia CAS",
    conflictCasMorphExplanation: "Os packages contêm dados diferentes de morfologia CAS com o mesmo Type, Group e Instance. É necessário revisar, mas isso não comprova que algum mod esteja com defeito.",
    mixedOverride: "Override Misto",
    packagePairs: "Pares de Packages",
    realOverrides: "Exige revisão",
    scriptConflicts: "Script",
    potentialConflicts: "Potenciais",
    analyzeConflictsToBegin: "Use Analisar CCs no Manager para preencher esta lista automaticamente ou analise conflitos aqui.",
    selectConflictFinding: "Selecione um resultado para inspecionar as evidências por resource.",
    conflictReason: "Por que foi classificado",
    conflictSamePayloadExplanation: "Esses packages compartilham um ou mais TGIs com payload descomprimido idêntico. Isso é conteúdo compartilhado, não conflito.",
    conflictVisualExplanation: "O mesmo TGI de resource visual existe nos dois packages, mas o payload descomprimido difere. É uma relação de override visual.",
    conflictCatalogExplanation: "O mesmo TGI de catálogo existe nos dois packages, mas o payload difere. O comportamento ou a categorização de catálogo pode ser sobrescrito.",
    conflictGameplayExplanation: "O mesmo TGI de tuning/gameplay existe nos dois packages, mas o payload difere. A ordem de carregamento pode alterar o comportamento no jogo.",
    conflictScriptExplanation: "O mesmo TGI de assembly/script existe nos dois packages com payload diferente. É uma colisão de script de alto risco.",
    conflictTextExplanation: "O mesmo TGI STBL existe com textos diferentes. É um override de texto/localização.",
    conflictPotentialExplanation: "O mesmo TGI existe com payload diferente, mas a família do resource ainda não está classificada em profundidade suficiente para uma conclusão mais forte.",
    conflictFilenameCollisionExplanation: "Os packages têm o mesmo nome, mas bytes SHA-256 diferentes. É apenas um resultado para revisão; o nome não comprova conflito de TGI compartilhado.",
    conflictMixedExplanation: "Este par de packages contém várias classes de resources compartilhados com conteúdo diferente.",
    sharedCount: "TGIs compartilhados",
    samePayloadCount: "Payloads idênticos",
    differentPayloadCount: "Payloads diferentes",
    resourceClass: "Classe do resource",
    payloadA: "Payload A",
    payloadB: "Payload B",
    impact: "Impacto",
    noConflictFindings: "Nenhum resultado corresponde à pesquisa e ao filtro atuais.",
    conflictAnalysisTruncated: "A lista de pares de conflito foi limitada por desempenho.",
    resourceCfg: "Resource.cfg",
    resourceCfgWarnings: "Avisos do Resource.cfg",
    loadPriority: "Prioridade de carregamento",
    matchingRule: "Regra correspondente",
    loadOrder: "Ordem de carregamento",
    higherPriorityWins: "A prioridade mais alta do Resource.cfg tem precedência neste par.",
    samePriorityUnknown: "Os dois packages possuem a mesma prioridade no Resource.cfg. A ferramenta não infere qual vence.",
    partialPriorityUnknown: "Apenas um package corresponde ao Resource.cfg. Esse par não provoca conflito de recursos no jogo sob essas regras de carregamento.",
    unmatchedPriorityUnknown: "Nenhum dos packages corresponde ao Resource.cfg. Eles podem estar na pasta analisada sem serem carregados pelo jogo.",
    missingResourceCfg: "Nenhum Resource.cfg foi encontrado na pasta selecionada nem na pasta pai.",
    likelyHigherPriority: "Prioridade mais alta",
    conflictsNext: "A análise de conflitos por resource está implementada em modo somente leitura.",
  },
  es: {
    moreActions: "Más acciones",
    selectExactDuplicatesMain: "Seleccionar Duplicados Exactos",
    viewQuarantine: "Ver Cuarentena",
    selectForReview: "Seleccionar para Revisión",
    reviewSelected: "Revisar Seleccionados",
    previousConflict: "Anterior",
    nextConflict: "Siguiente",
    moreConflictActions: "Más acciones de revisión y cuarentena",
    openOrganizedFolder: "Abrir Carpeta Organizada",
    analysisNeedsRefresh: "Archivos organizados. Analiza de nuevo para actualizar duplicados y conflictos.",
    remainingLegacyFiles: "archivos aún en las carpetas originales",
    remainingLegacyExamples: "Ver archivos restantes (nunca eliminados automáticamente)",
    disabledPackages: "Desactivados",
    noEligibleConflictDecisions: "Ningún conflicto seleccionado tiene datos suficientes para guardar esta decisión.",
    organizeElapsed: "Tiempo transcurrido",
    selectVisibleExactGroups: "Seleccionar Grupos Exactos Filtrados",
    exactOnlySelectionHint: "Limpia las otras selecciones antes de seleccionar duplicados exactos por lotes.",
    markSelectedIntentional: "Marcar Seleccionados como Intencionales",
    markSelectedReviewed: "Marcar Seleccionados como Revisados (Sesión)",
    ignoreSelectedSession: "Ignorar Seleccionados (Sesión)",
    clearSelectedMarks: "Limpiar Marcas de Seleccionados",
    reviewedSession: "Revisado en Esta Sesión",
    batchMarkedReviewed: "Revisados en esta sesión",
    batchMarkedIgnored: "Ignorados en esta sesión",
    batchMarkedIntentional: "Marcados como intencionales permanentemente",
    batchMarksCleared: "Marcas seleccionadas eliminadas",
    organizer: "Organizador",
    duplicates: "Duplicados",
    conflicts: "Conflictos",
    catalog: "Catálogo",
    converter: "Conversor",
    maintenance: "Mantenimiento",
    converterIntro: "Convierte archivos Sims3Pack a .package de forma segura, por separado o en conjunto.",
    restore: "Restaurar",
    language: "Idioma:",
    languageSelector: "Idioma",
    languageEnglish: "English",
    languagePortuguese: "Português",
    languageSpanish: "Español",
    theme: "Tema",
    themeSystem: "Sistema",
    themeVeigaLight: "Veiga Claro",
    themeVeigaDark: "Veiga Oscuro",
    themeMeanGirls: "Chicas Malas",
    themeEaApp: "EA App",
    themeWinuiLight: "WinUI Claro",
    themeWinuiDark: "WinUI Oscuro",
    themeSystemFootnote: "El tema y el color de énfasis se leen del sistema operativo.",
    chooseModsFolder: "Elegir Carpeta de Mods",
    noFolder: "Ninguna carpeta seleccionada.",
    scanCcs: "Analizar CCs",
    clearList: "Limpiar Lista",
    scanning: "Analizando packages…",
    progressStarting: "Preparando análisis…",
    progressScanning: "Analizando packages…",
    progressFingerprinting: "Generando fingerprints…",
    progressIndexing: "Indexando resources…",
    progressComparing: "Comparando candidatos…",
    progressCancelling: "Cancelando…",
    analyzeBefore: "Analiza antes de organizar",
    safetyMessage: "Los packages se clasifican por sus resources reales. Revisa el plan antes de cualquier movimiento.",
    readOnlyStage: "Analiza primero, revisa cada movimiento y luego organiza.",
    packages: "Packages",
    classified: "Clasificados",
    simExportMerged: "Sim exportado (CC combinado)",
    simExportRoot: "Mantener package completo en la raíz de Packages",
    simExportAlreadyRoot: "Ya está en la raíz de Packages",
    simExportCollision: "Colisión en la raíz — sin sobrescribir",
    simExportOverride: "Conservado en Overrides",
    mixed: "Mixtos",
    needsReview: "Requiere Revisión",
    invalid: "Inválidos",
    unknown: "Desconocidos",
    chooseToBegin: "Elige una carpeta de Mods para comenzar.",
    search: "Buscar por nombre o Instance ID",
    filter: "Filtro",
    allStatuses: "Todos los estados",
    selectPackage: "Selecciona un package para ver su clasificación.",
    detectedFrom: "Detectado por",
    category: "Categoría",
    subCategory: "Subcategoría",
    gender: "Género",
    age: "Edad",
    species: "Especie",
    usageCategories: "Categorías del catálogo",
    suggestedDestination: "Destino sugerido",
    possibleDestinations: "Destinos posibles",
    resources: "Resources",
    originalPath: "Ruta actual",
    warnings: "Notas para revisión",
    noDestination: "Sin destino automático",
    noResults: "Ningún package coincide con los filtros actuales.",
    scanComplete: "Análisis completado",
    scanFailed: "Error en el análisis",
    previewPlan: "Ver Plan",
    planning: "Creando plan…",
    planFailed: "Error en el plan",
    selectAllVisible: "Seleccionar Todo",
    selectNoneVisible: "Seleccionar Ninguno",
    selected: "Seleccionados",
    resourceCfgUpdateLabel: "Ampliar Resource.cfg para leer subcarpetas organizadas (con copia)",
  resourceCfgNewRules: "reglas por añadir",
  resourceCfgNoUpdate: "Este plan no modifica Resource.cfg.",
  resourceCfgAlreadyCovered: "Todas las carpetas seleccionadas ya están cubiertas.",
  resourceCfgConfirm: "Se guardará una copia de Resource.cfg y se actualizará:",
  organizationPlan: "Plan de Organización",
    simulationOnly: "Vista previa de seguridad. Nada se moverá hasta elegir Organizar Seleccionados.",
    readyToMove: "Listos",
    classifiedReady: "CC clasificados listos",
    exactDuplicatesReady: "Duplicados exactos para desactivar",
    alreadyOrganized: "Ya organizados",
    reviewPreserved: "Preservados para revisión",
    unresolvedCollisions: "Colisiones sin resolver",
    legacyFoldersToRemove: "Carpetas antiguas a eliminar",
    legacyFoldersRetained: "Carpetas antiguas conservadas",
    loadingTitle: "Procesando tus contenidos",
    loadingPhaseIdentification: "Identificando archivos",
    loadingPhaseAnalysis: "Analizando packages",
    loadingPhaseDuplicates: "Verificando duplicados",
    loadingPhasePlan: "Preparando el plan de organización",
    loadingPhaseOrganization: "Organizando packages",
    loadingPhaseIntegrity: "Comprobando integridad",
    loadingPhaseCleanup: "Limpiando carpetas antiguas",
    loadingPhaseRestore: "Restaurando archivos",
    loadingPhaseTools: "Trabajando en segundo plano",
    continueInBackground: "Continuar en Segundo Plano",
    loadingPhrase1: "Leyendo los archivos seleccionados en el Manager.",
    loadingPhrase2: "Comparando los detalles antes de preparar el siguiente paso.",
    loadingPhrase3: "Revisando el contenido para mantener un resultado seguro y coherente.",
    loadingPhrase4: "Registrando el trabajo para revisarlo o restaurarlo.",
    collisions: "Colisiones",
    planCollisionsSkipped: "Colisiones para revisar",
    organizationResult: "Resultado de la Organización",
    organizedPackages: "packages organizados",
    duplicatesPending: "duplicados idénticos pendientes de revisión",
    collisionsPending: "colisiones pendientes de revisión",
    emptyOldFoldersRemoved: "carpetas vacías eliminadas",
    oldFoldersStillOccupied: "carpetas de origen todavía contienen archivos (conservadas)",
    reviewDuplicates: "Revisar Duplicados",
    reviewCollisions: "Revisar Colisiones",
    collisionReviewTitle: "Colisiones Omitidas",
    collisionReviewIntro: "Estos packages quedaron intactos porque archivos diferentes apuntan al mismo destino.",
    collisionReviewSafety: "Ningún archivo en colisión fue movido ni sobrescrito.",
    duplicatesSkipped: "Duplicados para revisar",
    duplicateSkipped: "Duplicado idéntico · revisar",
    moveToNotCategorized: "Mantener en su lugar",
    duplicateToReview: "Duplicado · mantener en su lugar",
    collisionToReview: "Colisión · mantener en su lugar",
    blocked: "Bloqueados",
    foldersToCreate: "Carpetas",
    manifestPreview: "Vista previa del manifiesto de restauración",
    reviewBeforeMove: "Revisa todos los cambios antes de organizar.",
    close: "Cerrar",
    cancel: "Cancelar",
    cancelAnalysis: "Cancelar",
    cancelling: "Cancelando…",
    cancelled: "Análisis cancelado",
    technicalDetails: "Mostrar Detalles Técnicos",
    hideTechnicalDetails: "Ocultar Detalles Técnicos",
    previewLoading: "Cargando thumbnail…",
    previewUnavailable: "Thumbnail no disponible.",
    previewOpenLocation: "Haz clic en la thumbnail para abrir la ubicación del package.",
    technicalDetailsLoading: "Leyendo resources del package…",
    technicalDetailsError: "No se pudieron cargar los detalles técnicos",
    showingResourceLimit: "Mostrando los primeros 500 resources",
    fileSha256: "SHA-256 del archivo",
    dbpfVersion: "Versión DBPF",
    compression: "Compresión",
    diskSize: "Tamaño en disco",
    memorySize: "Tamaño en memoria",
    payloadHash: "SHA-256 del payload",
    resourceTechnicalDetails: "Detalles de los resources",
    meshInformation: "Información de Mesh",
    meshHighestPolycount: "Polycount visible más alto",
    meshHighestVertices: "Vértices en el polycount más alto",
    meshLods: "Polycount por LOD",
    meshResources: "Resources de mesh",
    meshTriangles: "Triángulos",
    meshVertices: "Vértices",
    meshGroups: "Grupos",
    meshShadow: "Sombra",
    meshUnknownLod: "LOD desconocido",
    meshNoData: "No se encontraron datos de mesh GEOM/MLOD/MODL compatibles.",
    meshWarnings: "Notas del análisis de mesh",
    cacheReused: "Caché reutilizada",
    cacheUpdated: "Caché actualizada",
    restoreHistory: "Historial de Restauración",
    restoreHistoryHint: "Manifiestos creados anteriormente para la carpeta de Mods seleccionada.",
    removeRestoreManifest: "Eliminar Registro",
    confirmRemoveQuarantineHistoryTitle: "¿Eliminar el registro de Cuarentena finalizada?",
    confirmRemoveQuarantineHistoryMessage: "Solo se eliminará el registro de cuarentena finalizada. No se borrarán archivos .package ni carpetas. Las cuarentenas activas o con recuperación pendiente están protegidas.",
    historyRemovalUnavailable: "Restaura o recupera esta Cuarentena antes de eliminar su historial.",
    historyRecordRemoved: "Registro del historial eliminado",
    confirmRemoveRestoreTitle: "¿Eliminar el registro de restauración?",
    confirmRemoveRestoreMessage: "Solo se elimina permanentemente el manifiesto seleccionado, NO los archivos .package. Sin él, no podrás deshacer esa organización desde Restore.",
    restoreRecordRemoved: "Registro de restauración eliminado",
    removingRestoreRecord: "Eliminando registro de restauración…",
    restoreSelectedHint: "Manifiesto seleccionado. Usa Ver Restauración para revisar los cambios. No se ha movido ningún archivo.",
    confirmQuarantineRestoreTitle: "¿Restaurar archivos de Cuarentena?",
    confirmQuarantineRestoreMessage: "Los archivos de esta cuarentena volverán a sus rutas originales. Confirma para continuar; no se sobrescribirá ningún archivo.",
    refreshHistory: "Actualizar",
    noRestoreHistory: "No se encontraron manifiestos de restauración para esta carpeta de Mods.",
    invalidManifest: "Manifiesto inválido",
    manifestFiles: "archivos",
    differentRoot: "Otra carpeta de Mods",
    cacheTitle: "Caché de Análisis",
    cacheNoData: "Sin información de caché.",
    openCacheFolder: "Abrir Caché",
    clearCache: "Limpiar Caché",
    fixIndexTitle: "Índice de firmas de Fixes",
    fixIndexNoData: "No hay índice del modelo cargado.",
    rebuildFixIndex: "Reconstruir desde el modelo",
    fixIndexChooseModel: "Elige la raíz del modelo manual",
    fixIndexRebuilt: "Índice de firmas reconstruido.",
    fixIndexBackupOk: "backup OK",
    fixIndexBackupPending: "el backup se creará al reconstruir",
    confirmClearCacheTitle: "¿Limpiar la caché de análisis?",
    confirmClearCacheMessage: "Solo se eliminarán los fingerprints en caché. Tus packages y manifiestos de restauración no se modificarán.",
    performanceDiagnostics: "Rendimiento",
    scanTime: "Análisis",
    totalTime: "Total",
    hashingTime: "Hashing",
    dbpfReadTime: "Lectura DBPF",
    resourceDecodeTime: "Descompresión/resources",
    comparisonTime: "Comparación",
    cacheEntries: "Elementos en caché",
    cacheSize: "Tamaño de caché",
    openPackageLocation: "Abrir Ubicación del Package",
    classificationReason: "Evidencia de clasificación",
    classificationConfidence: "Confianza de clasificación",
    creator: "Creador",
    modName: "Nombre del mod",
    gameplayCategory: "Categoría de jugabilidad",
    confidenceHigh: "Alta",
    confidenceMedium: "Media",
    confidenceLow: "Baja",
    confidenceManual: "Manual",
    keptUncategorized: "Mantenidos en su lugar",
    openManifestFolder: "Abrir Carpeta del Manifiesto",
    manifestRoot: "Raíz del manifiesto",
    selectedRoot: "Raíz de Mods seleccionada",
    rootMatch: "Las raíces coinciden",
    rootMismatch: "Otra carpeta de Mods — restauración bloqueada",
    intentionalOverride: "Override Intencional",
    markIntentional: "Marcar como Intencional",
    ignoreSession: "Ignorar Esta Sesión",
    clearMark: "Quitar Marca",
    ignoredSession: "Ignorado Esta Sesión",
    selectForQuarantine: "Seleccionar para vista previa de cuarentena",
    previewQuarantine: "Ver Cuarentena",
    quarantinePreviewOnly: "Preflight de cuarentena — revisa los archivos y el destino antes de mover.",
    quarantineRoot: "Destino de cuarentena",
    quarantineReady: "Listos para cuarentena",
    executeQuarantine: "Mover a Cuarentena",
    confirmQuarantineTitle: "¿Mover los duplicados seleccionados a Cuarentena?",
    confirmQuarantineMessage: "Los archivos seleccionados se moverán fuera de Packages tras un nuevo preflight SHA-256. Nada se elimina ni se sobrescribe y los fallos se revierten.",
    quarantineComplete: "Cuarentena completada",
    restoreQuarantine: "Restaurar Cuarentena",
    quarantineRestored: "Cuarentena restaurada",
    recoverQuarantine: "Recuperar Cuarentena interrumpida",
    quarantineRecovered: "Recuperación de la Cuarentena completada",
    dependencySkippedLarge: "Recursos demasiado grandes omitidos por seguridad",
    dependencyTruncated: "Se alcanzó el límite de resultados; análisis incompleto",
    manualReview: "Revisión Manual",
    manualDestination: "Destino aprobado",
    saveManualReview: "Guardar Revisión",
    clearManualReview: "Quitar Revisión",
    manualReviewHint: "Usa una carpeta como CAS\\Sliders o Jugabilidad\\Otros. La decisión se guarda por SHA-256 y nunca modifica el package.",
    manualReviewSaved: "Revisión manual guardada",
    suggestedDependencyGroup: "Grupo Mantener Juntos Sugerido",
    reviewSuggestedGroup: "Revisar Grupo",
    clearSelection: "Limpiar Selección",
    navigationFailed: "No se pudo abrir la ubicación",
    savedIntentionalOverride: "Override Intencional Guardado",
    auditReport: "Informe de Auditoría",
    auditReportHint: "Exporta clasificaciones, duplicados, conflictos y decisiones de revisión.",
    auditReportHintOrganizer: "Exporta las clasificaciones de packages y sus destinos.",
    auditReportHintDuplicates: "Exporta los grupos de duplicados y variantes relacionadas.",
    auditReportHintConflicts: "Exporta recursos compartidos, conflictos y decisiones de revisión.",
    exportAuditReport: "Exportar Informe",
    exportingAuditReport: "Exportando informe…",
    auditReportSaved: "Informe de auditoría guardado",
    auditReportFailed: "Error al exportar auditoría",
    openReportFolder: "Abrir Informes",
    reportNotAnalyzed: "No analizado",
    analysisQueued: "Esperando el análisis anterior…",
    analysisStatusLabel: "Estado del análisis",
    analysisStatus_not_run: "No ejecutado",
    analysisStatus_running: "En curso",
    analysisStatus_queued: "En espera",
    analysisStatus_completed: "Completado",
    analysisStatus_cancelled: "Cancelado",
    analysisStatus_failed: "Fallido",
    analysisStatus_cleared: "Borrado de la lista",
    analysisErrorLabel: "Error del análisis",
    reportGeneratedAt: "Generado en",
    reportDecision: "Decisión de revisión",
    confirmRemoveEmptyTitle: "¿Eliminar carpeta vacía?",
    confirmRemoveEmptyMessage: "Solo se eliminará esta carpeta vacía. No se borrará ningún archivo package.",
    removeEmptyFolderAction: "Eliminar Carpeta Vacía",
    structure: "Estructura",
    structureIntro: "Crea carpetas, mueve archivos o carpetas y renombra carpetas dentro de la raíz de Mods seleccionada.",
    structureUp: "Subir",
    createFolder: "Crear Carpeta",
    moveSelected: "Mover Seleccionado",
    renameFolder: "Renombrar Carpeta",
    currentFolder: "Carpeta actual",
    structureChooseRoot: "Elige una carpeta de Mods para administrar su estructura.",
    selectStructureItem: "Selecciona un archivo o una carpeta.",
    targetFolder: "Carpeta de destino",
    newFolderPath: "Ruta de la nueva carpeta",
    newFolderPathHint: "Puedes crear carpetas anidadas, por ejemplo: Creador\\Cabello\\Femenino.",
    newFolderName: "Nuevo nombre de la carpeta",
    createFolderTitle: "Crear carpeta",
    renameFolderTitle: "Renombrar carpeta",
    moveTitle: "Mover elemento",
    createFolderMessage: "La carpeta se creará dentro de la carpeta actual. Se permiten rutas anidadas.",
    renameFolderMessage: "Renombra la carpeta seleccionada. Los destinos existentes nunca se sobrescriben.",
    moveMessage: "Elige otra carpeta dentro de la raíz de Mods seleccionada. Los destinos existentes nunca se sobrescriben.",
    fileType: "Archivo",
    folderType: "Carpeta",
    itemType: "Tipo",
    structureSize: "Tamaño",
    structurePath: "Ruta",
    structureComplete: "Estructura actualizada",
    structureFailed: "Error en la operación de estructura",
    structureNoItems: "Esta carpeta está vacía.",
    openStructureLocation: "Abrir Ubicación",
    manualOperations: "Operaciones manuales de estructura",
    tools: "Herramientas",
    toolsIntro: "Perfiles, salud, snapshots, Inbox, metadatos, dependencias y diagnósticos técnicos.",
    profilesRules: "Perfiles",
    healthResourceCfg: "Salud",
    snapshotsCompare: "Snapshots y Comparación",
    inboxNewCc: "Inbox / New CC",
    metadataGroups: "Metadatos",
    technicalLab: "Técnico",
    operationHistory: "Historial",
    workspaceSettings: "Configuración del Workspace",
    readOnlyMode: "Modo de solo lectura",
    activeProfile: "Perfil activo",
    profileName: "Nombre del perfil",
    addProfile: "Añadir Perfil",
    destinationPrefix: "Prefijo de destino",
    collapseCategory: "Reducir a categoría principal",
    saveProfile: "Guardar Perfil",
    deleteProfile: "Eliminar Perfil",
    protectedFolders: "Carpetas Protegidas",
    customRules: "Reglas Personalizadas",
    add: "Añadir",
    ruleName: "Nombre de la regla",
    nameContains: "Nombre contiene",
    pathContains: "Ruta contiene",
    destination: "Destino",
    addRule: "Añadir Regla",
    analyzeHealth: "Analizar Salud",
    resourceCfgViewer: "Resource.cfg / Orden de Carga",
    healthFindings: "Resultados de Salud",
    healthCategoryFilter: "Categoría",
    healthSeverityFilter: "Gravedad",
    healthSelectVisible: "Seleccionar visibles",
    healthClearSelection: "Limpiar selección",
    healthExportSelected: "Exportar seleccionados (JSON)",
    healthSelectedCount: "seleccionados",
    healthAll: "Todos",
    healthNoFindings: "No hay resultados para este filtro.",
    coverageUnknown: "Cobertura no verificada",
    healthExported: "Informe de salud guardado",
    healthSelectVisible: "Seleccionar visibles", healthClearSelection: "Limpiar selección", healthExport: "Exportar seleccionados (JSON)", healthAll: "Todos los resultados", healthErrors: "Errores", healthWarnings: "Avisos", healthInformation: "Información", healthSelectedCount: "seleccionados",
    createSnapshot: "Crear Snapshot",
    snapshots: "Snapshots",
    compareFolders: "Comparar Dos Carpetas de Mods",
    chooseFolder: "Elegir Carpeta",
    compare: "Comparar",
    chooseInbox: "Elegir Carpeta Inbox",
    analyzeInbox: "Analizar Inbox",
    previewImport: "Ver Importación",
    importNewCc: "Importar a New CC",
    packageMetadata: "Metadatos del Package",
    tags: "Tags",
    testStatus: "Estado de prueba",
    untested: "Sin probar",
    working: "Funciona",
    problemStatus: "Con problema",
    removedStatus: "Eliminado",
    favorite: "Favorito",
    saveMetadata: "Guardar Metadatos",
    keepTogetherGroups: "Grupos Mantener Juntos",
    groupName: "Nombre del grupo",
    createGroup: "Crear Grupo",
    advancedSearch: "Búsqueda Técnica Avanzada",
    searchAction: "Buscar",
    sideBySideCompare: "Comparar Packages Lado a Lado",
    analyzeDependencies: "Analizar Dependencias",
    beforeAfterTree: "Antes → Después",
    undoLast: "Deshacer Última",
    emptyFolders: "Carpetas vacías",
    uncoveredPackages: "No cubiertos por Resource.cfg",
    outsidePackages: "Packages fuera de la raíz seleccionada",
    readablePackages: "Packages legibles",
    loadOrderViewer: "Orden de carga",
    snapshotCreated: "Snapshot creado",
    snapshotCompare: "Comparar con actual",
    addedFiles: "Añadidos",
    removedFiles: "Eliminados",
    modifiedFiles: "Modificados",
    movedFiles: "Movidos",
    inboxSource: "Origen de Inbox",
    noInbox: "Ninguna carpeta Inbox seleccionada.",
    importReady: "Vista previa de importación lista",
    technicalSearchHelp: "Usa type:, group:, instance:, sha: o texto libre.",
    dependenciesConservative: "Solo se muestran referencias TGI exactas incrustadas en resources conocidos por contener referencias.",
    readOnlyEnabled: "Modo de solo lectura activado",
    readOnlyDisabled: "Modo de solo lectura desactivado",
    profileSaved: "Perfil guardado",
    metadataSaved: "Metadatos guardados",
    groupSaved: "Grupo guardado",
    exportSaved: "Exportación guardada",
    noData: "Sin datos.",
    deleteGroup: "Eliminar Grupo",
    removeEmptyFolder: "Eliminar Carpeta Vacía",

    organizeSelected: "Organizar Seleccionados",
    alreadyOrganized: "Ya organizado",
    collisionSame: "Ya existe un archivo idéntico",
    collisionDifferent: "Ya existe un archivo diferente",
    current: "Actual",
    proposed: "Propuesto",
    noFoldersNeeded: "No es necesario crear carpetas nuevas.",
    selectedEligible: "aptos seleccionados",
    notEligible: "No apto para organización automática",
    executing: "Organizando packages seleccionados…",
    executionComplete: "Organización completada",
    executionNoChanges: "No era necesario mover archivos",
    executionRolledBack: "La organización falló y fue revertida",
    confirmOrganizeTitle: "¿Organizar los packages seleccionados?",
    confirmOrganizeMessage: "Se guardará un manifiesto de restauración antes de cualquier movimiento. Ningún archivo sobrescribirá un destino existente.",
    organizePartialHint: "Solo se moverán {ready} packages listos. Los {blocked} bloqueados permanecerán sin cambios para revisión.",
    restoreIntro: "Elige un manifiesto de restauración para ver exactamente lo que se deshará antes de modificar archivos.",
    chooseManifest: "Elegir Manifiesto de Restauración",
    noManifest: "Ningún manifiesto seleccionado.",
    previewRestore: "Ver Restauración",
    executeRestore: "Restaurar Estructura",
    tracked: "Rastreados",
    readyToRestore: "Listos",
    newFiles: "Archivos Nuevos",
    chooseManifestToBegin: "Elige un manifiesto de restauración para comenzar.",
    restorePreviewing: "Analizando manifiesto de restauración…",
    restoreReady: "Vista previa de restauración lista",
    restoreFailed: "Error de restauración",
    restoring: "Restaurando estructura anterior…",
    restoreComplete: "Restauración completada",
    confirmRestoreTitle: "¿Restaurar la estructura anterior?",
    confirmRestoreMessage: "Los archivos rastreados volverán a sus rutas originales. Los archivos añadidos después irán a Sin categorizar. Nada será sobrescrito.",
    alreadyRestored: "Ya restaurado",
    readyNew: "Nuevo → Sin categorizar",
    alreadyUncategorized: "Ya en Sin categorizar",
    changed: "Modificado",
    missing: "Ausente",
    ambiguous: "Ambiguo",
    duplicatesIntro: "Encuentra duplicados reales y distínguelos de variantes relacionadas de CC sin eliminar nada.",
    analyzeDuplicates: "Analizar Duplicados",
    reanalyzeDuplicates: "Reanalizar Duplicados",
    analyzingDuplicates: "Analizando fingerprints de los packages…",
    duplicatesReady: "Análisis de duplicados completado",
    duplicatesFailed: "Error en el análisis de duplicados",
    duplicateSearch: "Buscar por nome ou Instance ID",
    allDuplicateTypes: "Todos los resultados",
    exactDuplicate: "Duplicado Exacto",
    contentDuplicate: "Duplicado por Contenido",
    retexture: "Retexture",
    recategorizedVariant: "Variante Recategorizada",
    relatedVariant: "Variante Relacionada",
    exactGroups: "Grupos Exactos",
    contentGroups: "Grupos por Contenido",
    retextures: "Retextures",
    relatedVariants: "Variantes Relacionadas",
    analyzeDuplicatesToBegin: "Usa Analizar CC en Manager para llenar esta lista automáticamente o analiza duplicados aquí.",
    selectDuplicateFinding: "Selecciona un resultado para ver por qué fue detectado.",
    duplicateDetailsTitle: "Detalles de Duplicados",
    duplicateDetailsIntro: "Compara los archivos detectados, vistas previas y evidencias antes de realizar cualquier acción.",
    duplicateDetailsSafety: "Ningún archivo se modifica durante la revisión de duplicados.",
    conflictDetailsTitle: "Detalles de Conflictos",
    conflictDetailsIntro: "Compara ambos packages, vistas previas y evidencias de recursos antes de decidir cómo tratar una superposición.",
    conflictDetailsSafety: "Revisar un conflicto no modifica ningún package.",
    openLocation: "Abrir Ubicación",
    contentType: "Tipo de Contenido",
    contentSource: "Origen",
    fileSize: "Tamaño del Archivo",
    scriptedContent: "Script",
    yes: "Sí",
    no: "No",
    duplicateMembers: "Archivos de este grupo",
    duplicateReason: "Por qué fue marcado",
    fileHash: "SHA-256 del archivo",
    normalizedFingerprint: "Fingerprint normalizado",
    resourceCount: "Cantidad de resources",
    sharedResources: "Resources compartidos",
    identicalTgiPayloads: "Mismo TGI + mismo contenido",
    changedSameTgi: "Mismo TGI + contenido diferente",
    sharedStructural: "Resources estructurales compartidos",
    changedTextures: "Texturas/materiales modificados",
    changedCatalog: "Resources de catálogo modificados",
    evidence: "Evidencias",
    exactExplanation: "El archivo .package completo tiene el mismo SHA-256. Estos archivos son idénticos byte por byte.",
    contentExplanation: "Los contenedores de los packages difieren, pero el conjunto normalizado de TGIs y hashes de los resources descomprimidos es idéntico.",
    retextureExplanation: "Los resources estructurales coinciden mientras las texturas o materiales difieren. Es una variante visual relacionada, no un duplicado para eliminar.",
    recategorizedExplanation: "El contenido sustancial fuera del catálogo coincide mientras los resources de catálogo difieren. Es una variante recategorizada, no un duplicado para eliminar.",
    relatedExplanation: "Los resources estructurales coinciden, pero el resto del contenido del package difiere. Es contenido relacionado y requiere revisión.",
    unreadablePackages: "Packages no legibles",
    variantAnalysisTruncated: "La lista de relaciones entre variantes fue limitada por rendimiento.",
    noDuplicateFindings: "Ningún resultado coincide con la búsqueda y el filtro actuales.",
    duplicatesNext: "El análisis de duplicados es de solo lectura. Los duplicados confirmados pueden moverse a una Cuarentena reversible; nada se elimina automáticamente.",
    selectExactDuplicates: "Seleccionar TODOS los duplicados exactos",
    selectCurrentDuplicateGroup: "Seleccionar solo este grupo (conservar uno)",
    selectAllVisibleConflicts: "Seleccionar conflictos visibles",
    selectAllDetectedConflicts: "Seleccionar TODOS los conflictos detectados",
    clearReviewSelection: "Limpiar selección de revisión",
    prioritizeSelectedConflicts: "Priorizar revisión de los seleccionados",
    conflictReviewSelectedCount: "Conflictos seleccionados para revisión",
    organizePlanning: "Preparando y verificando el plan de organización…",
    organizePreparing: "Preparando el manifiesto de recuperación y Resource.cfg…",
    organizeSnapshotting: "Verificando la biblioteca y guardando hashes de recuperación…",
    organizeMoving: "Moviendo y verificando paquetes…",
    organizeCleaning: "Eliminando las subcarpetas vacías…",
    organizeRollingBack: "Revirtiendo los cambios…",
    organizeFinishing: "Finalizando organización…",
    quarantineSelectionCount: "Archivos seleccionados de todos los grupos",
    quarantineBatchHint: "Solo se seleccionan automáticamente copias idénticas. Se conserva una por grupo y se requiere confirmación.",
    bulkExactSummary: "Copias exactas seleccionadas para Cuarentena. Se conserva una copia de cada grupo.",
    bulkExactNone: "No hay grupos seguros de duplicados exactos.",
    bulkExactUnsafe: "La selección dejaría un grupo sin copias. Conserva al menos una.",
    prioritizeConflicts: "Priorizar revisión",
    previewSelectedQuarantine: "Ver cuarentena de seleccionados",
    conflictSelectionHint: "Elige A o B en cada conflicto. La prioridad de Resource.cfg nunca elige automáticamente un archivo para cuarentena.",
    conflictNoSelection: "Elige uno o varios packages de conflictos activos.",
    conflictBothSelected: "Se seleccionaron ambos packages de un conflicto. Conserva al menos uno.",
    conflictSelectedSummary: "Archivos seleccionados manualmente",
    conflictQuarantineTitle: "¿Mover a Cuarentena los packages de conflicto revisados?",
    conflictQuarantineMessage: "Solo se moverán los packages que hayas seleccionado. El orden de carga no garantiza que puedan retirarse. Revisa el manifiesto.",
    conflictPriorityUrgent: "Prioridad máxima",
    conflictPriorityHigh: "Prioridad alta",
    conflictPriorityReview: "Revisar",
    conflictPriorityInfo: "Informativo",
    conflictPriorityReason: "Revisa primero scripts y gameplay; el orden de carga no recomienda eliminar archivos.",
    chooseConflictFile: "Seleccionar para cuarentena",
    conflictsIntro: "Compara TGIs compartidos por el payload descomprimido y clasifica el impacto en lugar de tratar cada coincidencia como conflicto.",
    analyzeConflicts: "Analizar Conflictos",
    reanalyzeConflicts: "Reanalizar Conflictos",
    analyzingConflicts: "Analizando resources compartidos…",
    conflictsReady: "Análisis de conflictos completado",
    conflictsFailed: "Error en el análisis de conflictos",
    conflictSearch: "Buscar resultados de conflictos…",
    allConflictTypes: "Todos los resultados",
    conflictNeedsReview: "Requiere revisión",
    conflictOutsideLoad: "Fuera de las reglas de carga",
    conflictInactiveOverlap: "Superposición de recursos inactiva",
    conflictInactiveExplanation: "Al menos un package está fuera de las reglas de carga del Resource.cfg actual. Esta superposición no es un conflicto activo en el juego con las carpetas actuales. Mover los archivos a carpetas cargadas podría cambiarlo.",
    conflictInactiveCount: "No cargados juntos",
    conflictDifferentResources: "Recursos diferentes",
    conflictIdenticalResources: "Recursos idénticos",
    conflictEvidenceLimited: "Los recursos mostrados son una muestra; los totales incluyen todos los recursos compartidos.",
    conflictPreviewUnavailable: "Vista previa no disponible",
    sharedIdentical: "Compartido Idéntico",
    visualOverride: "Override Visual",
    catalogOverride: "Override de Catálogo",
    gameplayOverride: "Override de Gameplay",
    scriptConflict: "Conflicto de Script",
    textOverride: "Override de Texto",
    potentialConflict: "Conflicto Potencial",
    filenameCollisionReview: "Mismo Nombre · Bytes Diferentes",
    casMorphOverlap: "Superposición de Morfología CAS",
    conflictCasMorphExplanation: "Los packages contienen datos distintos de morfología CAS con el mismo Type, Group e Instance. Requiere revisión, pero no demuestra por sí solo que algún mod esté dañado.",
    mixedOverride: "Override Mixto",
    packagePairs: "Pares de Packages",
    realOverrides: "Requiere revisión",
    scriptConflicts: "Script",
    potentialConflicts: "Potenciales",
    analyzeConflictsToBegin: "Usa Analizar CC en Manager para llenar esta lista automáticamente o analiza conflictos aquí.",
    selectConflictFinding: "Selecciona un resultado para inspeccionar sus evidencias por resource.",
    conflictReason: "Por qué fue clasificado",
    conflictSamePayloadExplanation: "Estos packages comparten uno o más TGIs con payload descomprimido idéntico. Es contenido compartido, no un conflicto.",
    conflictVisualExplanation: "El mismo TGI de resource visual existe en ambos packages, pero el payload descomprimido es diferente. Es una relación de override visual.",
    conflictCatalogExplanation: "El mismo TGI de catálogo existe en ambos packages, pero el payload es diferente. El comportamiento o la categorización del catálogo puede quedar sobrescrito.",
    conflictGameplayExplanation: "El mismo TGI de tuning/gameplay existe en ambos packages, pero el payload es diferente. El orden de carga puede cambiar el comportamiento del juego.",
    conflictScriptExplanation: "El mismo TGI de assembly/script existe en ambos packages con un payload diferente. Es una colisión de script de alto riesgo.",
    conflictTextExplanation: "El mismo TGI STBL existe con textos diferentes. Es un override de texto/localización.",
    conflictPotentialExplanation: "El mismo TGI existe con un payload diferente, pero la familia del resource todavía no está clasificada con suficiente profundidad para una conclusión más fuerte.",
    conflictFilenameCollisionExplanation: "Los packages tienen el mismo nombre, pero bytes SHA-256 diferentes. Es solo un resultado para revisión; el nombre no demuestra un conflicto de TGI compartido.",
    conflictMixedExplanation: "Este par de packages contiene varias clases de resources compartidos con contenido diferente.",
    sharedCount: "TGIs compartidos",
    samePayloadCount: "Payloads idénticos",
    differentPayloadCount: "Payloads diferentes",
    resourceClass: "Clase del resource",
    payloadA: "Payload A",
    payloadB: "Payload B",
    impact: "Impacto",
    noConflictFindings: "Ningún resultado coincide con la búsqueda y el filtro actuales.",
    conflictAnalysisTruncated: "La lista de pares de conflicto fue limitada por rendimiento.",
    resourceCfg: "Resource.cfg",
    resourceCfgWarnings: "Avisos de Resource.cfg",
    loadPriority: "Prioridad de carga",
    matchingRule: "Regla correspondiente",
    loadOrder: "Orden de carga",
    higherPriorityWins: "La prioridad más alta de Resource.cfg tiene precedencia en este par.",
    samePriorityUnknown: "Ambos packages tienen la misma prioridad en Resource.cfg. La herramienta no infiere cuál gana.",
    partialPriorityUnknown: "Solo un package coincide con Resource.cfg. Este par no causa un conflicto de recursos en el juego bajo estas reglas de carga.",
    unmatchedPriorityUnknown: "Ninguno de los packages coincide con Resource.cfg. Pueden estar en la carpeta analizada sin que el juego los cargue.",
    missingResourceCfg: "No se encontró Resource.cfg en la carpeta seleccionada ni en su carpeta superior.",
    advancedCfgUnknown: "Este Resource.cfg usa directivas avanzadas de recorrido o condición. El Manager no inferirá un ganador solo por Priority.",
    likelyHigherPriority: "Prioridad más alta",
    conflictsNext: "El análisis de conflictos por resource está implementado en modo de solo lectura.",
  },
};

const LANGUAGE_ORDER = ["en", "pt", "es"];

const PREFS_KEY = "s3cc-organizer-preferences-v1";
const VALID_TABS = new Set(["organizer", "duplicates", "conflicts", "tools", "restore"]);

function loadPreferences() {
  try {
    const parsed = JSON.parse(localStorage.getItem(PREFS_KEY) || "{}");
    return parsed && typeof parsed === "object" ? parsed : {};
  } catch (_) {
    return {};
  }
}

const preferences = loadPreferences();
const savedLanguage =
  localStorage.getItem("s3cc-organizer-language") || preferences.language || "en";


const state = {
  language: savedLanguage,
  tab: "organizer",
  folder: typeof preferences.folder === "string" ? preferences.folder : "",
  items: [],
  stats: null,
  selectedId: "",
  selectedForPlan: new Set(),
  search: typeof preferences.search === "string" ? preferences.search : "",
  status: typeof preferences.status === "string" ? preferences.status : "all",
  isStatusFilterOpen: false,
  scanning: false,
  planning: false,
  executing: false,
  organizeProgress: null,
  error: "",
  planError: "",
  notice: "",
  plan: null,
  updateResourceCfg: false,
  organizationReview: null,
  organizationCollisionItems: [],
  lastOrganizedDirectory: "",
  restoreManifest: "",
  restorePlan: null,
  restorePreviewManifest: "",
  restoreBusy: false,
  restoreOperationActive: false,
  restoreOperationStatus: null,
  restoreError: "",
  restoreNotice: "",
  duplicatesAnalysis: null,
  duplicatesBusy: false,
  duplicatesError: "",
  duplicatesNotice: "",
  duplicatesSearch: typeof preferences.duplicatesSearch === "string" ? preferences.duplicatesSearch : "",
  duplicatesFilter: typeof preferences.duplicatesFilter === "string" ? preferences.duplicatesFilter : "all",
  duplicateSelectedId: "",
  conflictsAnalysis: null,
  conflictsBusy: false,
  analysisRunId: 0,
  analysisPipelineBusy: false,
  analysisRefreshQueued: false,
  analysisStatus: { manager: "not_run", duplicates: "not_run", conflicts: "not_run" },
  conflictsError: "",
  conflictsNotice: "",
  conflictsSearch: typeof preferences.conflictsSearch === "string" ? preferences.conflictsSearch : "",
  conflictsFilter:
    typeof preferences.conflictsFilter === "string" &&
    preferences.conflictsFilter !== "intentional_override" &&
    preferences.conflictsFilter !== "ignored_session"
      ? preferences.conflictsFilter
      : "attention",
  conflictSelectedId: "",
  operations: { scan: null, duplicates: null, conflicts: null },
  technicalDetails: {},
  technicalDetailsLoading: "",
  technicalDetailsErrors: {},
  technicalDetailsOpen: new Set(),
  packagePreviews: {},
  packagePreviewLoading: {},
  packagePreviewErrors: {},
  restoreHistory: [],
  restoreHistoryLoading: false,
  restoreHistoryError: "",
  cacheInfo: null,
  cacheBusy: false,
  cacheError: "",
  conflictMarks: {},
  persistentConflictMarks: {},
  persistentConflictRecords: [],
  reviewBusy: false,
  quarantineSelected: new Set(),
  quarantinePlan: null,
  quarantineExactBatch: false,
  lastExactGroupAnchor: null,
  postQuarantineNotice: null,
  conflictReviewSelected: new Set(),
  lastConflictReviewAnchor: null,
  conflictsSelectedOnly: false,
  conflictQuarantineSelected: new Set(),
  conflictQuarantinePlan: null,
  quarantineBusy: false,
  auditBusy: false,
  auditBusyKind: null,
  auditErrors: { organizer: "", duplicates: "", conflicts: "" },
  auditReports: { organizer: null, duplicates: null, conflicts: null },
  structureListing: null,
  structureCurrent: "",
  structureSelectedPath: "",
  structureDirectories: [],
  structureBusy: false,
  structureError: "",
  structureNotice: "",
  structureModalAction: "",
  manualOperations: [],
  toolsTab: "technical",
  workspaceStore: null,
  toolsBusy: false,
  toolsOperation: null,
  externalOperation: null,
  toolsError: "",
  toolsNotice: "",
  fixIndexStatus: null,
  fixIndexBusy: false,
  fixIndexError: "",
  healthReport: null,
  healthSelected: new Set(),
  healthFilter: "all",
  healthSeverity: "all",
  healthExported: null,
  snapshots: [],
  snapshotDiff: null,
  compareRoot: "",
  inboxFolder: "",
  inboxScan: null,
  inboxSelected: new Set(),
  inboxPlan: null,
  metadataGroupSelected: new Set(),
  technicalResults: [],
  technicalSelected: new Set(),
  packageCompare: null,
  dependenciesAnalysis: null,
  operationHistory: [],
  pendingEmptyFolder: "",
  pendingAction: "",
  pendingRemoveManifest: "",
  pendingRestoreManifest: "",
  pendingQuarantineRestore: null,
  pendingHistoryRemoval: null,
};

if (!LANGUAGE_ORDER.includes(state.language)) state.language = "en";

const MANAGER_ROW_STRIDE = 66;
const ANALYSIS_ROW_STRIDE = 62;
const VIRTUAL_OVERSCAN = 10;
const SEARCH_DEBOUNCE_MS = 100;

const managerSearchIndex = new WeakMap();
const managerVisibleMemo = { items: null, status: "", search: "", result: [] };
const duplicateVisibleMemo = { analysis: null, filter: "", search: "", result: [] };
const conflictVisibleMemo = { analysis: null, filter: "", search: "", marksVersion: 0, result: [] };
const virtualViews = {
  manager: { items: null, start: -1, end: -1, raf: 0 },
  duplicates: { items: null, start: -1, end: -1, raf: 0 },
  conflicts: { items: null, start: -1, end: -1, raf: 0 },
};
let conflictMarksVersion = 0;
let loadingPhraseIndex = -1;
let loadingPhraseAt = 0;
let loadingPhraseKey = "";
let loadingOverlayDismissedKey = "";
let operationMonitorSequence = 0;
const operationMonitorTokens = new Map();
let toolsOperationSequence = 0;
let restoreOperationSequence = 0;
const completedExternalOperationIds = new Set();

function debounce(callback, delay = SEARCH_DEBOUNCE_MS) {
  let timer = 0;
  return (...args) => {
    window.clearTimeout(timer);
    timer = window.setTimeout(() => callback(...args), delay);
  };
}

function normalizedInstanceQuery(query) {
  const lower = String(query || "").trim().toLowerCase();
  if (!lower) return null;
  const pos = lower.lastIndexOf("0x");
  const candidate = (pos >= 0 ? lower.slice(pos + 2) : lower).trim();
  if (candidate.length < 8 || candidate.length > 16 || !/^[0-9a-f]+$/i.test(candidate)) {
    return null;
  }
  return candidate.toUpperCase().padStart(16, "0");
}

function managerSearchText(item) {
  if (!item) return "";
  const cached = managerSearchIndex.get(item);
  if (cached) return cached;
  const text = [
    item.name,
    item.relativePath,
    item.category,
    item.subCategory,
    item.gender,
    item.age,
    item.species,
    item.classificationReason,
    item.classificationConfidence,
    item.creator,
    item.modName,
    item.gameplayCategory,
    item.contentSource,
    item.scripted ? "script scripted s3sa" : "",
    ...(item.detectedFrom || []),
    ...(item.usageCategories || []),
    ...(item.resourceTypes || []),
    ...(item.instances || []),
  ]
    .filter(Boolean)
    .join("\n")
    .toLocaleLowerCase();
  managerSearchIndex.set(item, text);
  return text;
}

function itemMatchesManagerSearch(item, query) {
  const normalized = String(query || "").trim().toLocaleLowerCase();
  if (!normalized) return true;
  if (managerSearchText(item).includes(normalized)) return true;

  const instance = normalizedInstanceQuery(normalized);
  if (!instance) return false;
  return (item.instances || []).some((value) =>
    String(value).replace(/^0x/i, "").toUpperCase().padStart(16, "0") === instance
  );
}

function virtualSpacer(height) {
  const spacer = document.createElement("div");
  spacer.className = "virtual-list-spacer";
  spacer.style.height = `${Math.max(0, height)}px`;
  spacer.setAttribute("aria-hidden", "true");
  return spacer;
}

function virtualRange(container, itemCount, stride) {
  const viewport = Math.max(container?.clientHeight || 0, 420);
  const scrollTop = container?.scrollTop || 0;
  const start = Math.max(0, Math.floor(scrollTop / stride) - VIRTUAL_OVERSCAN);
  const end = Math.min(
    itemCount,
    Math.ceil((scrollTop + viewport) / stride) + VIRTUAL_OVERSCAN
  );
  return { start, end };
}

function updateActiveVirtualRow(container, dataKey, id, className) {
  if (!container) return;
  const attribute = dataKey.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`);
  for (const row of container.querySelectorAll(`[data-${attribute}]`)) {
    row.classList.toggle(className, row.dataset[dataKey] === id);
  }
}

const el = {
  tabs: [...document.querySelectorAll(".tabs button")],
  pages: [...document.querySelectorAll(".tool-page")],
  languageButton: document.querySelector("#language-button"),
  languageCode: document.querySelector("#language-code"),
  languageMenu: document.querySelector("#language-menu"),
  languageMenuItems: [...document.querySelectorAll(".lang-menu-item")],
  chooseFolderBtn: document.querySelector("#choose-folder-btn"),
  scanBtn: document.querySelector("#scan-btn"),
  clearListBtn: document.querySelector("#clear-list-btn"),
  planBtn: document.querySelector("#plan-btn"),
  selectionSummary: document.querySelector("#selection-summary"),
  folderPath: document.querySelector("#folder-path"),
  scanState: document.querySelector("#scan-state"),
  emptyState: document.querySelector("#empty-state"),
  resultsState: document.querySelector("#results-state"),
  packageList: document.querySelector("#package-list"),
  previewCard: document.querySelector("#preview-card"),
  searchInput: document.querySelector("#search-input"),
  statusFilter: document.querySelector("#status-filter"),
  statusFilterLabel: document.querySelector("#status-filter-label"),
  statusFilterDropdown: document.querySelector("#status-filter-dropdown"),
  statusFilterToggleBtn: document.querySelector("#status-filter-toggle-btn"),
  statusFilterCurrent: document.querySelector("#status-filter-current"),
  statusFilterToggleIcon: document.querySelector("#status-filter-toggle-icon"),
  statusFilterMenu: document.querySelector("#status-filter-menu"),
  selectAllBtn: document.querySelector("#select-all-btn"),
  selectNoneBtn: document.querySelector("#select-none-btn"),
  statPackages: document.querySelector("#stat-packages"),
  statClassified: document.querySelector("#stat-classified"),
  statMixed: document.querySelector("#stat-mixed"),
  statReview: document.querySelector("#stat-review"),
  statUnknown: document.querySelector("#stat-unknown"),
  statInvalid: document.querySelector("#stat-invalid"),
  statDisabled: document.querySelector("#stat-disabled"),
  planModal: document.querySelector("#plan-modal"),
  planCloseBtn: document.querySelector("#plan-close-btn"),
  planCloseFooterBtn: document.querySelector("#plan-close-footer-btn"),
  planExecuteBtn: document.querySelector("#plan-execute-btn"),
  planPartialNotice: document.querySelector("#plan-partial-notice"),
  organizeProgress: document.querySelector("#organize-progress"),
  organizeProgressBar: document.querySelector("#organize-progress-bar"),
  organizeProgressText: document.querySelector("#organize-progress-text"),
  organizeProgressPercent: document.querySelector("#organize-progress-percent"),
  organizeProgressCount: document.querySelector("#organize-progress-count"),
  planCfgRow: document.querySelector("#plan-cfg-row"),
  planCfgCheckbox: document.querySelector("#plan-cfg-checkbox"),
  planCfgSummary: document.querySelector("#plan-cfg-summary"),
  planItems: document.querySelector("#plan-items"),
  planDirectories: document.querySelector("#plan-directories"),
  planTree: document.querySelector("#plan-tree"),
  manifestPreviewText: document.querySelector("#manifest-preview-text"),
  planStatSelected: document.querySelector("#plan-stat-selected"),
  planStatReady: document.querySelector("#plan-stat-ready"),
  planStatDuplicatesReady: document.querySelector("#plan-stat-duplicates-ready"),
  planStatOrganized: document.querySelector("#plan-stat-organized"),
  planStatKept: document.querySelector("#plan-stat-kept"),
  planStatSkipped: document.querySelector("#plan-stat-skipped"),
  planStatCollisions: document.querySelector("#plan-stat-collisions"),
  planStatBlocked: document.querySelector("#plan-stat-blocked"),
  planStatFolders: document.querySelector("#plan-stat-folders"),
  planStatFoldersRemove: document.querySelector("#plan-stat-folders-remove"),
  planStatFoldersRetained: document.querySelector("#plan-stat-folders-retained"),
  loadingOverlay: document.querySelector("#loading-overlay"),
  loadingOverlayPhase: document.querySelector("#loading-overlay-phase"),
  loadingOverlayPercent: document.querySelector("#loading-overlay-percent"),
  loadingOverlayBar: document.querySelector("#loading-overlay-bar"),
  loadingOverlayCount: document.querySelector("#loading-overlay-count"),
  loadingOverlayPhrase: document.querySelector("#loading-overlay-phrase"),
  loadingOverlayCancel: document.querySelector("#loading-overlay-cancel"),
  loadingOverlayContinue: document.querySelector("#loading-overlay-continue"),
  organizationReviewPanel: document.querySelector("#organization-review-panel"),
  organizationReviewSummary: document.querySelector("#organization-review-summary"),
  organizationReviewLeftovers: document.querySelector("#organization-review-leftovers"),
  organizationReviewLeftoverPaths: document.querySelector("#organization-review-leftover-paths"),
  reviewDuplicatesBtn: document.querySelector("#review-duplicates-btn"),
  reviewCollisionsBtn: document.querySelector("#review-collisions-btn"),
  collisionReviewModal: document.querySelector("#collision-review-modal"),
  collisionReviewList: document.querySelector("#collision-review-list"),
  collisionReviewCloseBtn: document.querySelector("#collision-review-close-btn"),
  collisionReviewCloseFooterBtn: document.querySelector("#collision-review-close-footer-btn"),
  openOrganizedFolderBtn: document.querySelector("#open-organized-folder-btn"),
  openOrganizedResultBtn: document.querySelector("#open-organized-result-btn"),
  chooseManifestBtn: document.querySelector("#choose-manifest-btn"),
  previewRestoreBtn: document.querySelector("#preview-restore-btn"),
  executeRestoreBtn: document.querySelector("#execute-restore-btn"),
  restoreManifestPath: document.querySelector("#restore-manifest-path"),
  removeRestoreHistoryBtn: document.querySelector("#remove-restore-history-btn"),
  restoreState: document.querySelector("#restore-state"),
  restoreEmpty: document.querySelector("#restore-empty"),
  restoreResults: document.querySelector("#restore-results"),
  restoreItems: document.querySelector("#restore-items"),
  restoreStatTracked: document.querySelector("#restore-stat-tracked"),
  restoreStatReady: document.querySelector("#restore-stat-ready"),
  restoreStatNew: document.querySelector("#restore-stat-new"),
  restoreStatCollisions: document.querySelector("#restore-stat-collisions"),
  restoreStatBlocked: document.querySelector("#restore-stat-blocked"),
  confirmModal: document.querySelector("#confirm-modal"),
  confirmTitle: document.querySelector("#confirm-title"),
  confirmMessage: document.querySelector("#confirm-message"),
  confirmCancelBtn: document.querySelector("#confirm-cancel-btn"),
  confirmActionBtn: document.querySelector("#confirm-action-btn"),
  analyzeDuplicatesBtn: document.querySelector("#analyze-duplicates-btn"),
  duplicatesState: document.querySelector("#duplicates-state"),
  duplicatesSearch: document.querySelector("#duplicates-search"),
  duplicatesFilter: document.querySelector("#duplicates-filter"),
  duplicatesEmpty: document.querySelector("#duplicates-empty"),
  duplicatesResults: document.querySelector("#duplicates-results"),
  duplicatesList: document.querySelector("#duplicates-list"),
  duplicatesPreview: document.querySelector("#duplicate-details-content"),
  duplicateDetailsModal: document.querySelector("#duplicate-details-modal"),
  duplicateDetailsCloseBtn: document.querySelector("#duplicate-details-close-btn"),
  duplicateDetailsCloseFooterBtn: document.querySelector("#duplicate-details-close-footer-btn"),
  dupStatPackages: document.querySelector("#dup-stat-packages"),
  dupStatExact: document.querySelector("#dup-stat-exact"),
  dupStatContent: document.querySelector("#dup-stat-content"),
  dupStatRetexture: document.querySelector("#dup-stat-retexture"),
  dupStatRelated: document.querySelector("#dup-stat-related"),
  analyzeConflictsBtn: document.querySelector("#analyze-conflicts-btn"),
  conflictsState: document.querySelector("#conflicts-state"),
  conflictsSearch: document.querySelector("#conflicts-search"),
  conflictsFilter: document.querySelector("#conflicts-filter"),
  conflictsEmpty: document.querySelector("#conflicts-empty"),
  conflictsResults: document.querySelector("#conflicts-results"),
  conflictsList: document.querySelector("#conflicts-list"),
  conflictsPreview: document.querySelector("#conflicts-preview"),
  conflictDetailsModal: document.querySelector("#conflict-details-modal"),
  conflictDetailsCloseBtn: document.querySelector("#conflict-details-close-btn"),
  conflictDetailsCloseFooterBtn: document.querySelector("#conflict-details-close-footer-btn"),
  conflictPreviousBtn: document.querySelector("#conflict-previous-btn"),
  conflictNextBtn: document.querySelector("#conflict-next-btn"),
  confStatPairs: document.querySelector("#conf-stat-pairs"),
  confStatReal: document.querySelector("#conf-stat-real"),
  confStatScript: document.querySelector("#conf-stat-script"),
  confStatPotential: document.querySelector("#conf-stat-potential"),
  confStatShared: document.querySelector("#conf-stat-shared"),
  scanProgress: document.querySelector("#scan-progress"),
  scanProgressBar: document.querySelector("#scan-progress-bar"),
  scanProgressText: document.querySelector("#scan-progress-text"),
  scanProgressPercent: document.querySelector("#scan-progress-percent"),
  scanProgressCount: document.querySelector("#scan-progress-count"),
  scanCancelBtn: document.querySelector("#scan-cancel-btn"),
  duplicatesProgress: document.querySelector("#duplicates-progress"),
  duplicatesProgressBar: document.querySelector("#duplicates-progress-bar"),
  duplicatesProgressText: document.querySelector("#duplicates-progress-text"),
  duplicatesProgressPercent: document.querySelector("#duplicates-progress-percent"),
  duplicatesProgressCount: document.querySelector("#duplicates-progress-count"),
  duplicatesCancelBtn: document.querySelector("#duplicates-cancel-btn"),
  conflictsProgress: document.querySelector("#conflicts-progress"),
  conflictsProgressBar: document.querySelector("#conflicts-progress-bar"),
  conflictsProgressText: document.querySelector("#conflicts-progress-text"),
  conflictsProgressPercent: document.querySelector("#conflicts-progress-percent"),
  conflictsProgressCount: document.querySelector("#conflicts-progress-count"),
  conflictsCancelBtn: document.querySelector("#conflicts-cancel-btn"),
  restoreHistoryList: document.querySelector("#restore-history-list"),
  refreshRestoreHistoryBtn: document.querySelector("#refresh-restore-history-btn"),
  appShell: document.querySelector(".app-shell"),
  layout: document.querySelector(".layout"),
  cacheStatusDot: document.querySelector("#cache-status-dot"),
  cacheSummary: document.querySelector("#cache-summary"),
  openCacheBtn: document.querySelector("#open-cache-btn"),
  clearCacheBtn: document.querySelector("#clear-cache-btn"),
  fixIndexStatusDot: document.querySelector("#fix-index-status-dot"),
  fixIndexSummary: document.querySelector("#fix-index-summary"),
  rebuildFixIndexBtn: document.querySelector("#rebuild-fix-index-btn"),
  diagnosticsPanel: document.querySelector("#diagnostics-panel"),
  diagnosticsContent: document.querySelector("#diagnostics-content"),
  openManifestFolderBtn: document.querySelector("#open-manifest-folder-btn"),
  restoreRootCheck: document.querySelector("#restore-root-check"),
  restoreManifestRoot: document.querySelector("#restore-manifest-root"),
  restoreSelectedRoot: document.querySelector("#restore-selected-root"),
  restoreRootStatus: document.querySelector("#restore-root-status"),
  auditStatuses: [...document.querySelectorAll("[data-audit-status]")],
  exportAuditButtons: [...document.querySelectorAll("[data-export-audit]")],
  openReportFolderButtons: [...document.querySelectorAll("[data-open-report-folder]")],
  duplicatesClearListBtn: document.querySelector("#duplicates-clear-list-btn"),
  duplicatesSelectExactBtn: document.querySelector("#duplicates-select-exact-btn"),
  duplicatesSelectCurrentBtn: document.querySelector("#duplicates-select-current-btn"),
  duplicatesSelectVisibleBtn: document.querySelector("#duplicates-select-visible-btn"),
  duplicatesPreviewAllBtn: document.querySelector("#duplicates-preview-all-btn"),
  duplicatesClearSelectionBtn: document.querySelector("#duplicates-clear-selection-btn"),
  duplicatesBatchSummary: document.querySelector("#duplicates-batch-summary"),
  duplicatesBatchPreview: document.querySelector("#duplicates-batch-preview"),
  conflictsClearListBtn: document.querySelector("#conflicts-clear-list-btn"),
  conflictsSelectAllBtn: document.querySelector("#conflicts-select-all-btn"),
  conflictsSelectDetectedBtn: document.querySelector("#conflicts-select-detected-btn"),
  conflictsReviewSelectedBtn: document.querySelector("#conflicts-review-selected-btn"),
  conflictsClearReviewBtn: document.querySelector("#conflicts-clear-review-btn"),
  conflictsPrioritizeSelectedBtn: document.querySelector("#conflicts-prioritize-selected-btn"),
  conflictsMarkIntentionalBtn: document.querySelector("#conflicts-mark-intentional-btn"),
  conflictsMarkReviewedBtn: document.querySelector("#conflicts-mark-reviewed-btn"),
  conflictsMarkIgnoredBtn: document.querySelector("#conflicts-mark-ignored-btn"),
  conflictsClearMarksBtn: document.querySelector("#conflicts-clear-marks-btn"),
  conflictsReviewSelectionCount: document.querySelector("#conflicts-review-selection-count"),
  conflictsPrioritizeBtn: document.querySelector("#conflicts-prioritize-btn"),
  conflictsPreviewQuarantineBtn: document.querySelector("#conflicts-preview-quarantine-btn"),
  conflictsClearQuarantineBtn: document.querySelector("#conflicts-clear-quarantine-btn"),
  structureState: document.querySelector("#structure-state"),
  structureUpBtn: document.querySelector("#structure-up-btn"),
  structureRefreshBtn: document.querySelector("#structure-refresh-btn"),
  structureCreateBtn: document.querySelector("#structure-create-btn"),
  structureMoveBtn: document.querySelector("#structure-move-btn"),
  structureRenameBtn: document.querySelector("#structure-rename-btn"),
  structureUndoBtn: document.querySelector("#structure-undo-btn"),
  structureCurrentPath: document.querySelector("#structure-current-path"),
  structureEmpty: document.querySelector("#structure-empty"),
  structureResults: document.querySelector("#structure-results"),
  structureList: document.querySelector("#structure-list"),
  structurePreview: document.querySelector("#structure-preview"),
  structureModal: document.querySelector("#structure-modal"),
  structureModalTitle: document.querySelector("#structure-modal-title"),
  structureModalMessage: document.querySelector("#structure-modal-message"),
  structureInputWrap: document.querySelector("#structure-input-wrap"),
  structureInputLabel: document.querySelector("#structure-input-label"),
  structureInput: document.querySelector("#structure-input"),
  structureTargetWrap: document.querySelector("#structure-target-wrap"),
  structureTargetSelect: document.querySelector("#structure-target-select"),
  structureModalCancelBtn: document.querySelector("#structure-modal-cancel-btn"),
  structureModalActionBtn: document.querySelector("#structure-modal-action-btn"),
  toolsState: document.querySelector("#tools-state"),
  toolsCancel: document.querySelector("#tools-cancel"),
  toolsSubtabs: [...document.querySelectorAll(".tools-subtab")],
  toolsPanels: [...document.querySelectorAll(".tools-panel")],
  toolsReadOnly: document.querySelector("#tools-readonly"),
  toolsProfileSelect: document.querySelector("#tools-profile-select"),
  toolsProfileName: document.querySelector("#tools-profile-name"),
  toolsAddProfile: document.querySelector("#tools-add-profile"),
  toolsProfilePrefix: document.querySelector("#tools-profile-prefix"),
  toolsProfileCollapse: document.querySelector("#tools-profile-collapse"),
  toolsSaveProfile: document.querySelector("#tools-save-profile"),
  toolsDeleteProfile: document.querySelector("#tools-delete-profile"),
  toolsProtectedInput: document.querySelector("#tools-protected-input"),
  toolsAddProtected: document.querySelector("#tools-add-protected"),
  toolsProtectedList: document.querySelector("#tools-protected-list"),
  ruleName: document.querySelector("#rule-name"),
  ruleCategory: document.querySelector("#rule-category"),
  ruleSubcategory: document.querySelector("#rule-subcategory"),
  ruleDetected: document.querySelector("#rule-detected"),
  ruleNameContains: document.querySelector("#rule-name-contains"),
  rulePathContains: document.querySelector("#rule-path-contains"),
  ruleDestination: document.querySelector("#rule-destination"),
  toolsAddRule: document.querySelector("#tools-add-rule"),
  toolsRulesList: document.querySelector("#tools-rules-list"),
  toolsHealthRun: document.querySelector("#tools-health-run"),
  toolsHealthFilter: document.querySelector("#tools-health-filter"),
  toolsHealthSeverity: document.querySelector("#tools-health-severity"),
  toolsHealthSelectVisible: document.querySelector("#tools-health-select-visible"),
  toolsHealthClearSelection: document.querySelector("#tools-health-clear-selection"),
  toolsHealthExport: document.querySelector("#tools-health-export"),
  toolsHealthSelectionCount: document.querySelector("#tools-health-selection-count"),
  toolsHealthSummary: document.querySelector("#tools-health-summary"),
  toolsResourcecfg: document.querySelector("#tools-resourcecfg"),
  toolsHealthFindings: document.querySelector("#tools-health-findings"),
  toolsCreateSnapshot: document.querySelector("#tools-create-snapshot"),
  toolsRefreshSnapshots: document.querySelector("#tools-refresh-snapshots"),
  toolsSnapshotList: document.querySelector("#tools-snapshot-list"),
  toolsCompareRoot: document.querySelector("#tools-compare-root"),
  toolsChooseCompareRoot: document.querySelector("#tools-choose-compare-root"),
  toolsCompareRoots: document.querySelector("#tools-compare-roots"),
  toolsSnapshotDiff: document.querySelector("#tools-snapshot-diff"),
  toolsChooseInbox: document.querySelector("#tools-choose-inbox"),
  toolsInboxPath: document.querySelector("#tools-inbox-path"),
  toolsScanInbox: document.querySelector("#tools-scan-inbox"),
  toolsPreviewImport: document.querySelector("#tools-preview-import"),
  toolsExecuteImport: document.querySelector("#tools-execute-import"),
  toolsInboxResults: document.querySelector("#tools-inbox-results"),
  toolsInboxPlan: document.querySelector("#tools-inbox-plan"),
  toolsMetadataPackage: document.querySelector("#tools-metadata-package"),
  toolsTags: document.querySelector("#tools-tags"),
  toolsTestStatus: document.querySelector("#tools-test-status"),
  toolsFavorite: document.querySelector("#tools-favorite"),
  toolsSaveMetadata: document.querySelector("#tools-save-metadata"),
  toolsGroupName: document.querySelector("#tools-group-name"),
  toolsGroupPackageList: document.querySelector("#tools-group-package-list"),
  toolsSaveGroup: document.querySelector("#tools-save-group"),
  toolsGroupsList: document.querySelector("#tools-groups-list"),
  toolsTechQuery: document.querySelector("#tools-tech-query"),
  toolsTechSearch: document.querySelector("#tools-tech-search"),
  toolsExportTxt: document.querySelector("#tools-export-txt"),
  toolsExportCsv: document.querySelector("#tools-export-csv"),
  toolsExportJson: document.querySelector("#tools-export-json"),
  toolsTechResults: document.querySelector("#tools-tech-results"),
  toolsCompareLeft: document.querySelector("#tools-compare-left"),
  toolsCompareRight: document.querySelector("#tools-compare-right"),
  toolsComparePackages: document.querySelector("#tools-compare-packages"),
  toolsPackageCompare: document.querySelector("#tools-package-compare"),
  toolsDependencies: document.querySelector("#tools-dependencies"),
  toolsDependencyResults: document.querySelector("#tools-dependency-results"),
  toolsRefreshHistory: document.querySelector("#tools-refresh-history"),
  toolsOperationHistory: document.querySelector("#tools-operation-history"),
};

function persistPreferences() {
  const data = {
    language: state.language,
    tab: state.tab,
    folder: state.folder,
    search: state.search,
    status: state.status,
    duplicatesSearch: state.duplicatesSearch,
    duplicatesFilter: state.duplicatesFilter,
    conflictsSearch: state.conflictsSearch,
    conflictsFilter:
      state.conflictsFilter === "intentional_override" ||
      state.conflictsFilter === "ignored_session"
        ? "all"
        : state.conflictsFilter,
  };
  localStorage.setItem(PREFS_KEY, JSON.stringify(data));
}


function formatMs(value) {
  const ms = Number(value || 0);
  if (ms < 1000) return `${Math.round(ms)} ms`;
  return `${(ms / 1000).toFixed(ms < 10000 ? 2 : 1)} s`;
}

async function revealSafe(path) {
  if (!path) return;
  try {
    await invoke("reveal_path", { path });
  } catch (error) {
    state.notice = `${t("navigationFailed")}: ${String(error)}`;
    render();
  }
}

async function openDirectorySafe(path) {
  if (!path) return;
  try {
    await invoke("open_directory", { path });
  } catch (error) {
    state.notice = `${t("navigationFailed")}: ${String(error)}`;
    render();
  }
}

async function refreshCacheInfo() {
  if (!state.folder) {
    state.cacheInfo = null;
    state.cacheError = "";
    renderCachePanel();
    return;
  }
  state.cacheBusy = true;
  state.cacheError = "";
  renderCachePanel();
  try {
    state.cacheInfo = await invoke("get_cache_info", { folder: state.folder });
  } catch (error) {
    state.cacheInfo = null;
    state.cacheError = String(error);
  } finally {
    state.cacheBusy = false;
    renderCachePanel();
    renderDiagnostics();
  }
}

async function refreshFixIndexStatus() {
  if (!el.fixIndexSummary) return;
  state.fixIndexError = "";
  try {
    state.fixIndexStatus = await invoke("get_persistent_fix_signature_index_status");
  } catch (error) {
    state.fixIndexStatus = null;
    state.fixIndexError = String(error);
  }
  renderFixIndexPanel();
}

async function rebuildFixIndex() {
  if (state.fixIndexBusy || toolsOperationLocked()) return;
  const selected = await open({
    directory: true,
    multiple: false,
    title: t("fixIndexChooseModel"),
  });
  if (!selected || Array.isArray(selected)) return;
  state.fixIndexBusy = true;
  state.fixIndexError = "";
  renderFixIndexPanel();
  try {
    await invoke("rebuild_persistent_fix_signature_index", { referenceRoot: selected });
    state.toolsNotice = t("fixIndexRebuilt");
    await refreshFixIndexStatus();
  } catch (error) {
    state.fixIndexError = String(error);
  } finally {
    state.fixIndexBusy = false;
    renderFixIndexPanel();
  }
}

function renderFixIndexPanel() {
  if (!el.fixIndexSummary) return;
  const status = state.fixIndexStatus;
  el.fixIndexStatusDot.className =
    "utility-dot " + (state.fixIndexBusy ? "busy" : state.fixIndexError ? "error" : status?.available ? "ready" : "");
  if (state.fixIndexBusy) {
    el.fixIndexSummary.textContent = t("scanning");
  } else if (state.fixIndexError) {
    el.fixIndexSummary.textContent = state.fixIndexError;
  } else if (status?.available) {
    const backup = status.backupAvailable ? ` · ${t("fixIndexBackupOk")}` : ` · ${t("fixIndexBackupPending")}`;
    el.fixIndexSummary.textContent = `${status.records} · v${status.version} · ${status.path}${backup}`;
  } else {
    el.fixIndexSummary.textContent = t("fixIndexNoData");
  }
  el.rebuildFixIndexBtn.disabled = state.fixIndexBusy || toolsOperationLocked();
}

async function clearAnalysisCache() {
  if (
    !state.folder ||
    state.cacheBusy ||
    state.scanning ||
    state.duplicatesBusy ||
    state.conflictsBusy ||
    state.structureBusy ||
    state.technicalDetailsLoading
  ) return;
  state.cacheBusy = true;
  renderCachePanel();
  try {
    await invoke("clear_cache", { folder: state.folder });
    state.technicalDetails = {};
    state.technicalDetailsOpen.clear();
    state.cacheError = "";
    await refreshCacheInfo();
  } catch (error) {
    state.cacheError = String(error);
  } finally {
    state.cacheBusy = false;
    renderCachePanel();
  }
}

function renderCachePanel() {
  if (!el.cacheSummary) return;
  el.cacheStatusDot.className =
    "utility-dot " + (state.cacheBusy ? "busy" : state.cacheError ? "error" : state.cacheInfo?.entries ? "ready" : "");
  if (state.cacheBusy) {
    el.cacheSummary.textContent = t("technicalDetailsLoading");
  } else if (state.cacheError) {
    el.cacheSummary.textContent = state.cacheError;
  } else if (state.cacheInfo) {
    el.cacheSummary.textContent =
      `${state.cacheInfo.entries} · ${bytesLabel(state.cacheInfo.bytes)} · ${state.cacheInfo.path}`;
  } else {
    el.cacheSummary.textContent = t("cacheNoData");
  }
  const hasCache = !!state.cacheInfo?.bytes;
  el.openCacheBtn.classList.toggle("hidden", !hasCache);
  el.clearCacheBtn.classList.toggle("hidden", !hasCache);
  el.openCacheBtn.disabled = state.cacheBusy;
  el.clearCacheBtn.disabled =
    state.cacheBusy ||
    state.scanning ||
    state.duplicatesBusy ||
    state.conflictsBusy ||
    state.structureBusy ||
    !!state.technicalDetailsLoading;
}

function appendDiagnostic(label, value) {
  const row = document.createElement("div");
  row.className = "diagnostic-row";
  const span = document.createElement("span");
  span.textContent = label;
  const strong = document.createElement("strong");
  strong.textContent = value;
  row.append(span, strong);
  el.diagnosticsContent.appendChild(row);
}

function renderDiagnostics() {
  if (!el.diagnosticsContent) return;

  const hasScanDiagnostics = !!state.stats;
  el.diagnosticsPanel?.classList.toggle("hidden", !hasScanDiagnostics);
  if (!hasScanDiagnostics) {
    if (el.diagnosticsPanel) el.diagnosticsPanel.open = false;
    el.diagnosticsContent.innerHTML = "";
    return;
  }

  el.diagnosticsContent.innerHTML = "";
  if (state.stats?.totalMs != null) {
    appendDiagnostic(t("scanTime"), formatMs(state.stats.totalMs));
  }
  const dup = state.duplicatesAnalysis?.stats;
  if (dup) {
    appendDiagnostic(`${t("duplicates")} · ${t("totalTime")}`, formatMs(dup.totalMs));
    appendDiagnostic(t("hashingTime"), formatMs(dup.hashingMs));
    appendDiagnostic(t("dbpfReadTime"), formatMs(dup.dbpfLoadMs));
    appendDiagnostic(t("resourceDecodeTime"), formatMs(dup.resourceDecodeMs));
    appendDiagnostic(t("comparisonTime"), formatMs(dup.comparisonMs));
  }
  const conf = state.conflictsAnalysis?.stats;
  if (conf) {
    const elapsed = state.operations.conflicts?.elapsedMs || 0;
    appendDiagnostic(`${t("conflicts")} · ${t("totalTime")}`, formatMs(conf.totalMs || elapsed));
    appendDiagnostic(`${t("conflicts")} · ${t("hashingTime")}`, formatMs(conf.hashingMs));
    appendDiagnostic(`${t("conflicts")} · ${t("dbpfReadTime")}`, formatMs(conf.dbpfLoadMs));
    appendDiagnostic(`${t("conflicts")} · ${t("resourceDecodeTime")}`, formatMs(conf.resourceDecodeMs));
    appendDiagnostic(`${t("conflicts")} · ${t("comparisonTime")}`, formatMs(conf.comparisonMs));
  }
  if (state.cacheInfo) {
    appendDiagnostic(t("cacheEntries"), String(state.cacheInfo.entries ?? 0));
    appendDiagnostic(t("cacheSize"), bytesLabel(state.cacheInfo.bytes));
  }
  if (!el.diagnosticsContent.children.length) {
    appendDiagnostic(t("performanceDiagnostics"), "—");
  }
}

function applyPersistentDecisionRecords(records) {
  state.persistentConflictRecords = Array.isArray(records) ? records : [];
  state.persistentConflictMarks = {};
  for (const record of state.persistentConflictRecords) {
    if (record?.decisionKey && record?.mark === "intentional_override") {
      state.persistentConflictMarks[record.decisionKey] = "intentional";
    }
  }
  conflictMarksVersion += 1;
}

async function refreshConflictDecisions() {
  if (!state.folder) {
    applyPersistentDecisionRecords([]);
    renderConflicts();
    renderAuditPanel();
    return;
  }

  state.reviewBusy = true;
  try {
    const records = await invoke("load_conflict_decisions", { folder: state.folder });
    applyPersistentDecisionRecords(records);
  } catch (error) {
    state.conflictsNotice = String(error);
    applyPersistentDecisionRecords([]);
  } finally {
    state.reviewBusy = false;
    renderConflicts();
    renderAuditPanel();
    renderStructure();
  }
}

function effectiveConflictMark(finding) {
  if (!finding) return null;
  if (state.conflictMarks[finding.id] === "ignored") return "ignored";
  if (state.conflictMarks[finding.id] === "reviewed") return "reviewed";
  if (state.persistentConflictMarks[finding.decisionKey] === "intentional") return "intentional";
  return null;
}

async function setConflictMark(finding, mark) {
  if (!finding || state.reviewBusy) return;

  if (mark === "ignored" || mark === "reviewed") {
    state.conflictMarks[finding.id] = mark;
    conflictMarksVersion += 1;
    renderConflicts();
    return;
  }

  if (mark === null && ["ignored", "reviewed"].includes(state.conflictMarks[finding.id])) {
    delete state.conflictMarks[finding.id];
    conflictMarksVersion += 1;
    renderConflicts();
    return;
  }

  state.reviewBusy = true;
  renderConflicts();

  try {
    const records = await invoke("set_conflict_decision", {
      folder: state.folder,
      decisionKey: finding.decisionKey,
      mark: mark === "intentional" ? "intentional_override" : null,
      leftSha256: finding.left?.fileSha256 || "",
      rightSha256: finding.right?.fileSha256 || "",
      leftRelativePath: finding.left?.relativePath || "",
      rightRelativePath: finding.right?.relativePath || "",
    });
    applyPersistentDecisionRecords(records);
    delete state.conflictMarks[finding.id];
    conflictMarksVersion += 1;
  } catch (error) {
    state.conflictsNotice = String(error);
  } finally {
    state.reviewBusy = false;
    renderConflicts();
    renderAuditPanel();
    renderStructure();
  }
}

function reportCell(value) {
  return String(value ?? "")
    .replaceAll("|", "\\|")
    .replaceAll("\n", " ");
}

function analysisReadsBusy() {
  return state.scanning || state.analysisPipelineBusy ||
    state.duplicatesBusy || state.conflictsBusy;
}

function analysisSessionMatches(folder, runId) {
  return state.folder === folder && state.analysisRunId === runId;
}

function analysisReportStatus(kind) {
  return {
    status: state.analysisStatus[kind],
    error: kind === "manager" ? state.error :
      kind === "duplicates" ? state.duplicatesError : state.conflictsError,
  };
}

function buildAuditSnapshot() {
  const plannedBySource = new Map(
    (state.plan?.items || []).map((item) => [
      String(item.sourcePath || "").replaceAll(String.fromCharCode(92), "/").toLocaleLowerCase(), item
    ])
  );
  const conflicts = state.conflictsAnalysis
    ? {
        ...state.conflictsAnalysis,
        findings: (state.conflictsAnalysis.findings || []).map((finding) => ({
          ...finding,
          reviewDecision: effectiveConflictMark(finding),
          persistentDecision:
            state.persistentConflictMarks[finding.decisionKey] === "intentional"
              ? "intentional_override"
              : null,
        })),
      }
    : null;

  return {
    schemaVersion: 4,
    analysisStatus: {
      manager: analysisReportStatus("manager"),
      duplicates: analysisReportStatus("duplicates"),
      conflicts: analysisReportStatus("conflicts"),
    },
    generatedAt: new Date().toISOString(),
    root: state.folder,
    language: state.language,
    organizer: state.stats
      ? {
          stats: state.stats,
          planStatus: state.plan ? "preview_available" : "not_generated",
          planStats: state.plan?.stats ?? null,
          lastExecution: state.organizationReview ?? null,
          // A scanner classification is NOT proof of a physical move.
          packages: state.items.map((item) => ({
            plannedAction: plannedBySource.get(
              String(item.path || "").replaceAll(String.fromCharCode(92), "/").toLocaleLowerCase()
            )?.planStatus ?? "not_planned",
            planDestination: plannedBySource.get(
              String(item.path || "").replaceAll(String.fromCharCode(92), "/").toLocaleLowerCase()
            )?.destinationRelativePath ?? null,
            planWarnings: plannedBySource.get(
              String(item.path || "").replaceAll(String.fromCharCode(92), "/").toLocaleLowerCase()
            )?.warnings ?? [],
            name: item.name,
            path: item.path,
            relativePath: item.relativePath,
            status: item.status,
            classificationConfidence: item.classificationConfidence,
            resourceTypes: item.resourceTypes || [],
            catalogResourceCount: item.catalogResourceCount ?? 0,
            resourceCount: item.resourceCount ?? 0,
            detectedFrom: item.detectedFrom || [],
            candidateDestinations: item.candidateDestinations || [],
            fileSize: item.fileSize ?? 0,
            scripted: !!item.scripted,
            selectableForPlan: eligibleForPlan(item),
            creator: item.creator,
            modName: item.modName,
            gameplayCategory: item.gameplayCategory,
            category: item.category,
            subCategory: item.subCategory,
            destinationPath: item.destinationPath,
            classificationReason: item.classificationReason,
            warnings: item.warnings || [],
          })),
        }
      : null,
    duplicates: state.duplicatesAnalysis,
    conflicts,
    persistentConflictDecisions: state.persistentConflictRecords,
    manualOperations: state.manualOperations,
    restorePreview: state.restorePlan,
    quarantinePreview: state.quarantinePlan,
    cache: state.cacheInfo,
    workspace: state.workspaceStore,
    health: state.healthReport,
    snapshots: state.snapshots,
    snapshotDiff: state.snapshotDiff,
    dependencies: state.dependenciesAnalysis,
    operationHistory: state.operationHistory,
    inbox: {
      sourceFolder: state.inboxFolder || null,
      scan: state.inboxScan,
      importPlan: state.inboxPlan,
    },
    performance: {
      scan: state.stats?.totalMs ?? null,
      duplicates: state.duplicatesAnalysis?.stats ?? null,
      conflicts: state.conflictsAnalysis?.stats ?? null,
    },
  };
}

function buildAuditMarkdown(snapshot) {
  const lines = [
    "# S3CC Manager Audit Report",
    "",
    `- ${t("reportGeneratedAt")}: ${snapshot.generatedAt}`,
    `- Root: ${snapshot.root || "—"}`,
    `- ${t("language")}: ${snapshot.language}`,
    "",
    "## Manager",
    "",
  ];

  if (!snapshot.organizer) {
    lines.push(t("reportNotAnalyzed"), "");
  } else {
    const stats = snapshot.organizer.stats || {};
    lines.push(
      `Packages: ${stats.packages ?? snapshot.organizer.packages.length} · ${t("classified")}: ${stats.classified ?? 0} · ${t("mixed")}: ${stats.mixed ?? 0} · ${t("needsReview")}: ${stats.needsReview ?? 0} · ${t("invalid")}: ${stats.invalid ?? 0}`,
      "",
      "| Package | Status | Confidence | Creator / Mod | Category | Destination | Evidence |",
      "| --- | --- | --- | --- | --- | --- | --- |"
    );
    for (const item of snapshot.organizer.packages) {
      lines.push(
        `| ${reportCell(item.relativePath || item.name)} | ${reportCell(item.status)} | ${reportCell(item.classificationConfidence)} | ${reportCell([item.creator, item.modName].filter(Boolean).join(" / "))} | ${reportCell([item.gameplayCategory, item.category, item.subCategory].filter(Boolean).join(" / "))} | ${reportCell(item.destinationPath)} | ${reportCell(item.classificationReason)} |`
      );
    }
    lines.push("");
  }

  lines.push("## Duplicates", "");
  if (!snapshot.duplicates) {
    lines.push(t("reportNotAnalyzed"), "");
  } else {
    const ds = snapshot.duplicates.stats || {};
    lines.push(
      `Packages: ${ds.packagesScanned ?? 0} · Exact groups: ${ds.exactGroups ?? 0} · Content groups: ${ds.contentGroups ?? 0} · Retextures: ${ds.retextureRelations ?? 0} · Related: ${ds.relatedVariantRelations ?? 0}`,
      ""
    );
    for (const group of snapshot.duplicates.groups || []) {
      lines.push(
        `### ${duplicateKindLabel(group.kind)}`,
        "",
        ...((group.members || []).map((member) => `- ${member.relativePath} — ${member.fileSha256 || ""}`)),
        ""
      );
    }
    for (const relation of snapshot.duplicates.relations || []) {
      lines.push(
        `### ${duplicateKindLabel(relation.kind)}`,
        "",
        `- A: ${relation.left?.relativePath || "—"}`,
        `- B: ${relation.right?.relativePath || "—"}`,
        `- Shared resources: ${relation.sharedResourceCount ?? 0}`,
        ""
      );
    }
  }

  lines.push("## Conflicts", "");
  if (!snapshot.conflicts) {
    lines.push(t("reportNotAnalyzed"), "");
  } else {
    const cs = snapshot.conflicts.stats || {};
    lines.push(
      `Pairs: ${cs.packagePairs ?? 0} · Visual: ${cs.visualOverrides ?? 0} · Catalog: ${cs.catalogOverrides ?? 0} · Gameplay: ${cs.gameplayOverrides ?? 0} · Script: ${cs.scriptConflicts ?? 0}`,
      ""
    );
    for (const finding of snapshot.conflicts.findings || []) {
      lines.push(
        `### ${finding.reviewDecision === "intentional" ? t("savedIntentionalOverride") : conflictKindLabel(finding.kind)}`,
        "",
        `- A: ${finding.left?.relativePath || "—"}`,
        `- B: ${finding.right?.relativePath || "—"}`,
        `- Decision key: ${finding.decisionKey || "—"}`,
        `- ${t("reportDecision")}: ${finding.reviewDecision || "—"}`,
        `- Shared TGIs: ${finding.sharedResourceCount ?? 0}`,
        `- Different payloads: ${finding.differentPayloadCount ?? 0}`,
        `- Load order: ${finding.loadOrderStatus || "—"}`,
        ""
      );
    }
  }

  lines.push("## Saved Conflict Decisions", "");
  if (!snapshot.persistentConflictDecisions?.length) {
    lines.push("—", "");
  } else {
    for (const decision of snapshot.persistentConflictDecisions) {
      lines.push(
        `- ${decision.mark}: ${decision.leftRelativePath} ↔ ${decision.rightRelativePath} (${decision.decisionKey})`
      );
    }
    lines.push("");
  }

  lines.push(`## ${t("manualOperations")}`, "");
  if (!snapshot.manualOperations?.length) {
    lines.push("—", "");
  } else {
    for (const operation of snapshot.manualOperations) {
      const source = operation.sourceRelativePath || "—";
      const destination = operation.destinationRelativePath || "—";
      lines.push(
        `- ${operation.createdAt} · ${operation.operation} · ${source} → ${destination}`
      );
    }
    lines.push("");
  }

  lines.push("## Workspace", "");
  if (snapshot.workspace) {
    lines.push(
      `- Read-only: ${snapshot.workspace.readOnly}`,
      `- Active profile: ${snapshot.workspace.activeProfileId}`,
      `- Profiles: ${snapshot.workspace.profiles?.length || 0}`,
      `- Groups: ${snapshot.workspace.groups?.length || 0}`,
      `- Tagged packages: ${Object.keys(snapshot.workspace.packageMetadata || {}).length}`,
      ""
    );
  } else lines.push("—", "");

  lines.push("## Mods Health", "");
  if (snapshot.health) {
    const hs = snapshot.health.stats || {};
    lines.push(
      `- Packages: ${hs.packages ?? 0}`,
      `- Unreadable: ${hs.unreadable ?? 0}`,
      `- Empty folders: ${hs.emptyFolders ?? 0}`,
      `- Resource.cfg uncovered: ${hs.resourceCfgUncovered ?? 0}`,
      `- Outside root: ${hs.packagesOutsideRoot ?? 0}`,
      ""
    );
  } else lines.push(t("reportNotAnalyzed"), "");

  lines.push("## Snapshots / Dependencies", "");
  lines.push(
    `- Snapshots: ${snapshot.snapshots?.length || 0}`,
    `- Potential dependencies: ${snapshot.dependencies?.findings?.length || 0}`,
    `- History entries: ${snapshot.operationHistory?.length || 0}`,
    ""
  );

  lines.push("## Performance", "");
  if (snapshot.performance.scan != null) {
    lines.push(`- Scan: ${formatMs(snapshot.performance.scan)}`);
  }
  if (snapshot.performance.duplicates?.totalMs != null) {
    lines.push(`- Duplicates: ${formatMs(snapshot.performance.duplicates.totalMs)}`);
  }
  if (snapshot.performance.conflicts?.totalMs != null) {
    lines.push(`- Conflicts: ${formatMs(snapshot.performance.conflicts.totalMs)}`);
  }
  lines.push("");

  return lines.join("\n");
}


const AUDIT_REPORT_KINDS = ["organizer", "duplicates", "conflicts"];

function buildScopedAuditSnapshot(kind) {
  if (!AUDIT_REPORT_KINDS.includes(kind)) return null;
  const full = buildAuditSnapshot();
  const shared = {
    schemaVersion: full.schemaVersion,
    reportKind: kind,
    generatedAt: full.generatedAt,
    root: full.root,
    language: full.language,
    analysisStatus: full.analysisStatus,
  };

  if (kind === "organizer") {
    return { ...shared, organizer: full.organizer };
  }
  if (kind === "duplicates") {
    return { ...shared, duplicates: full.duplicates };
  }
  return {
    ...shared,
    conflicts: full.conflicts,
    persistentConflictDecisions: full.persistentConflictDecisions,
  };
}

function buildScopedAuditMarkdown(snapshot) {
  const kind = snapshot.reportKind;
  const label = kind === "organizer" ? "Organizer"
    : kind === "duplicates" ? "Duplicates" : "Conflicts";
  const lines = [
    `# Veiga's S3CC Manager — ${label} Audit Report`,
    "",
    `- ${t("reportGeneratedAt")}: ${snapshot.generatedAt}`,
    `- Root: ${snapshot.root || "—"}`,
    `- ${t("language")}: ${snapshot.language}`,
    `- Manager: ${snapshot.analysisStatus?.manager?.status ?? "not_run"}`,
    `- Duplicates: ${snapshot.analysisStatus?.duplicates?.status ?? "not_run"}`,
    `- Conflicts: ${snapshot.analysisStatus?.conflicts?.status ?? "not_run"}`,
    "",
  ];

  if (kind === "organizer") {
    const organizer = snapshot.organizer;
    const stats = organizer?.stats || {};
    lines.push("## Organizer", "");
    lines.push("IMPORTANT: Classification destinations are suggestions, not proof of moved files. A move only occurs after an executable plan and confirmed transaction.");
    lines.push("Plan: " + (organizer?.planStatus || "not_generated") +
      " · Ready: " + (organizer?.planStats?.ready ?? "not_calculated") +
      " · Blocked: " + (organizer?.planStats?.blocked ?? "not_calculated") +
      " · Empty directories: " + (organizer?.planStats?.emptyFoldersToClean ?? "not_calculated"), "");
    const execution = organizer?.lastExecution;
    if (execution?.analysisBefore || execution?.execution || execution?.analysisCurrent) {
      const phase = snapshot => snapshot
        ? `${snapshot.phase}: ${snapshot.packages ?? 0} packages, ${snapshot.unknown ?? 0} unknown, ${snapshot.remainingLegacyFiles ?? 0} legacy leftovers`
        : "not available";
      lines.push(
        `Analysis phases: ${phase(execution.analysisBefore)} · ${phase(execution.execution)} · ${phase(execution.analysisCurrent)}`,
        ""
      );
    }
    lines.push(
      `Packages: ${stats.packages ?? organizer?.packages?.length ?? 0} · ${t("classified")}: ${stats.classified ?? 0} · ${t("mixed")}: ${stats.mixed ?? 0} · ${t("needsReview")}: ${stats.needsReview ?? 0} · ${t("invalid")}: ${stats.invalid ?? 0}`,
      "",
      "| Package | Status | Confidence | Creator / Mod | Category | Classification Destination | Planned Action | Actual Plan Destination | Evidence | Warnings |",
      "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |"
    );
    for (const item of organizer?.packages || []) {
      lines.push(
        `| ${reportCell(item.relativePath || item.name)} | ${reportCell(item.status)} | ${reportCell(item.classificationConfidence)} | ${reportCell([item.creator, item.modName].filter(Boolean).join(" / "))} | ${reportCell([item.gameplayCategory, item.category, item.subCategory].filter(Boolean).join(" / "))} | ${reportCell(item.destinationPath)} | ${reportCell(item.plannedAction)} | ${reportCell(item.planDestination)} | ${reportCell(item.classificationReason)} | ${reportCell([...(item.warnings || []), ...(item.planWarnings || [])].join("; "))} |`
      );
    }
    lines.push("");
  } else if (kind === "duplicates") {
    const duplicates = snapshot.duplicates || {};
    const stats = duplicates.stats || {};
    lines.push(
      "## Duplicates", "",
      `Packages: ${stats.packagesScanned ?? 0} · Exact groups: ${stats.exactGroups ?? 0} · Content groups: ${stats.contentGroups ?? 0} · Retextures: ${stats.retextureRelations ?? 0} · Related: ${stats.relatedVariantRelations ?? 0}`, ""
    );
    for (const group of duplicates.groups || []) {
      lines.push(`### ${duplicateKindLabel(group.kind)}`, "");
      for (const member of group.members || []) {
        lines.push(`- ${member.relativePath || "—"} — SHA-256: ${member.fileSha256 || "—"}`);
      }
      lines.push("");
    }
    for (const relation of duplicates.relations || []) {
      lines.push(
        `### ${duplicateKindLabel(relation.kind)}`, "",
        `- A: ${relation.left?.relativePath || "—"}`,
        `- B: ${relation.right?.relativePath || "—"}`,
        `- Shared resources: ${relation.sharedResourceCount ?? 0}`, ""
      );
    }
  } else {
    const conflicts = snapshot.conflicts || {};
    const stats = conflicts.stats || {};
    lines.push(
      "## Conflicts", "",
      `Pairs: ${stats.packagePairs ?? 0} · Visual: ${stats.visualOverrides ?? 0} · Catalog: ${stats.catalogOverrides ?? 0} · Gameplay: ${stats.gameplayOverrides ?? 0} · Script: ${stats.scriptConflicts ?? 0}`,
      ""
    );
    for (const finding of conflicts.findings || []) {
      lines.push(
        `### ${finding.reviewDecision === "intentional" ? t("savedIntentionalOverride") : conflictKindLabel(finding.kind)}`, "",
        `- A: ${finding.left?.relativePath || "—"}`,
        `- B: ${finding.right?.relativePath || "—"}`,
        `- Decision key: ${finding.decisionKey || "—"}`,
        `- ${t("reportDecision")}: ${finding.reviewDecision || "—"}`,
        `- Shared TGIs: ${finding.sharedResourceCount ?? 0}`,
        `- Different payloads: ${finding.differentPayloadCount ?? 0}`,
        `- Load order: ${finding.loadOrderStatus || "—"}`, ""
      );
    }
    lines.push("## Saved Conflict Decisions", "");
    for (const decision of snapshot.persistentConflictDecisions || []) {
      lines.push(`- ${decision.mark}: ${decision.leftRelativePath} ↔ ${decision.rightRelativePath} (${decision.decisionKey})`);
    }
    if (!snapshot.persistentConflictDecisions?.length) lines.push("—");
    lines.push("");
  }
  return lines.join("\n");
}

async function exportAuditReport(kind) {
  if (!AUDIT_REPORT_KINDS.includes(kind) || !state.folder ||
      state.auditBusy || state.scanning || state.duplicatesBusy ||
      state.conflictsBusy || state.reviewBusy || state.structureBusy ||
      (kind === "organizer" && !state.stats) ||
      (kind === "duplicates" && !state.duplicatesAnalysis) ||
      (kind === "conflicts" && !state.conflictsAnalysis)) return;

  const selectedRoot = state.folder;
  state.auditBusy = true;
  state.auditBusyKind = kind;
  state.auditErrors[kind] = "";
  renderAuditPanel();
  renderStructure();

  try {
    const snapshot = buildScopedAuditSnapshot(kind);
    const result = await invoke("save_audit_report", {
      folder: selectedRoot,
      kind,
      markdown: buildScopedAuditMarkdown(snapshot),
      jsonContent: JSON.stringify(snapshot, null, 2),
    });
    if (state.folder === selectedRoot) state.auditReports[kind] = result;
  } catch (error) {
    if (state.folder === selectedRoot) state.auditErrors[kind] = String(error);
  } finally {
    state.auditBusy = false;
    state.auditBusyKind = null;
    renderAuditPanel();
    renderStructure();
  }
}

function renderAuditPanel() {
  for (const kind of AUDIT_REPORT_KINDS) {
    const status = el.auditStatuses.find((element) => element.dataset.auditStatus === kind);
    const saved = state.auditReports[kind];
    const error = state.auditErrors[kind];
    const exporting = state.auditBusy && state.auditBusyKind === kind;
    const message = exporting
      ? t("exportingAuditReport")
      : error
        ? `${t("auditReportFailed")}: ${error}`
        : saved
          ? `${t("auditReportSaved")}: ${saved.markdownPath}`
          : kind === "organizer"
            ? t("auditReportHintOrganizer")
            : kind === "duplicates"
              ? t("auditReportHintDuplicates")
              : t("auditReportHintConflicts");

    if (status) {
      status.textContent = message;
      status.classList.toggle("error", !!error);
    }

    const ready = kind === "organizer" ? !!state.stats
      : kind === "duplicates" ? !!state.duplicatesAnalysis
      : !!state.conflictsAnalysis;
    const canExport = !!state.folder && ready &&
      !state.scanning && !state.duplicatesBusy && !state.conflictsBusy &&
      !state.auditBusy && !state.reviewBusy && !state.structureBusy && !state.analysisPipelineBusy;
    const exportButton = el.exportAuditButtons.find((button) => button.dataset.exportAudit === kind);
    if (exportButton) exportButton.disabled = !canExport;

    const openButton = el.openReportFolderButtons.find((button) => button.dataset.openReportFolder === kind);
    if (openButton) {
      openButton.classList.toggle("hidden", !saved?.directory);
      openButton.disabled = state.auditBusy;
    }
  }

  el.duplicatesClearListBtn.disabled =
    !state.duplicatesAnalysis || state.duplicatesBusy || state.conflictsBusy ||
    state.scanning || state.quarantineBusy || state.auditBusy;
  el.duplicatesSelectExactBtn.disabled =
    !exactDuplicateGroups().length || state.duplicatesBusy || state.conflictsBusy ||
    state.scanning || state.analysisPipelineBusy || state.quarantineBusy ||
    state.auditBusy || workspaceReadOnly();
  el.conflictsClearListBtn.disabled =
    !state.conflictsAnalysis || state.conflictsBusy || state.duplicatesBusy ||
    state.scanning || state.reviewBusy || state.auditBusy || state.quarantineBusy;
}

function t(key) {
  return I18N[state.language]?.[key] ?? I18N.en[key] ?? key;
}

// Selection is based on the package's safety state, not just a completed
// classification. Unknown packages may move to Not Categorized; mixed,
// ambiguous and corrupt packages must stay available for explicit review.
function eligibleForPlan(item) {
  if (!item?.path) return false;
  const status = String(item.status || "");
  if (status === "unknown") return true;
  if (status === "sim_export_merged") return true;
  if (status !== "classified" || !item.destinationPath) return false;
  const verifiedCompanion =
    item.classificationConfidence === "medium" &&
    (item.detectedFrom || []).includes("ModFolderCompanion");
  return item.classificationConfidence === "high" ||
    item.classificationConfidence === "manual" || verifiedCompanion;
}

function statusLabel(status) {
  return {
    classified: t("classified"),
    sim_export_merged: t("simExportMerged"),
    mixed: t("mixed"),
    needs_review: t("needsReview"),
    unknown: t("unknown"),
    invalid: t("invalid"),
  }[status] || status;
}

function supportsResourceCfgUpdate(folder) {
  const parts = String(folder || "").replaceAll(String.fromCharCode(92), "/").split("/").filter(Boolean);
  const mods = parts.findLastIndex((part, index) =>
    /^mods(?:[ _-].+| \\(.+\\))?$/i.test(part) &&
    (index === parts.length - 1 || /^(packages|overrides)$/i.test(parts[index + 1]))
  );
  return mods >= 0;
}

function planCanExecute(plan) {
  const stats = plan?.stats || {};
  return ((stats.ready ?? 0) > 0 || (stats.emptyFoldersToClean ?? 0) > 0)
    && !!plan?.canExecute && !workspaceReadOnly();
}

function planStatusLabel(status) {
  return {
    ready: t("readyToMove"),
    keep_uncategorized: t("keptUncategorized"),
    ready_uncategorized: t("moveToNotCategorized"),
    ready_duplicate: t("duplicateToReview"),
    ready_duplicate_quarantine: t("exactDuplicatesReady"),
    ready_duplicate_quarantine_migration: t("exactDuplicatesReady"),
    ready_collision: t("collisionToReview"),
    already_organized: t("alreadyOrganized"),
    ready_sim_export_merged_root: t("simExportRoot"),
    already_sim_export_merged_root: t("simExportAlreadyRoot"),
    keep_sim_export_merged_override: t("simExportOverride"),
    sim_export_merged_collision_same_content: t("simExportCollision"),
    sim_export_merged_collision_different_content: t("simExportCollision"),
    duplicate_skipped: t("duplicateSkipped"),
    collision_same_content: t("collisionSame"),
    collision_different_content: t("collisionDifferent"),
    blocked: t("blocked"),
  }[status] || status;
}

function confidenceLabel(value) {
  return {
    high: t("confidenceHigh"),
    medium: t("confidenceMedium"),
    low: t("confidenceLow"),
    manual: t("confidenceManual"),
  }[value] || value || "—";
}

function restoreStatusLabel(status) {
  return {
    ready_restore: t("readyToRestore"),
    already_restored: t("alreadyRestored"),
    ready_new: t("readyNew"),
    already_uncategorized: t("alreadyUncategorized"),
    collision_same_content: t("collisionSame"),
    collision_different_content: t("collisionDifferent"),
    changed: t("changed"),
    missing: t("missing"),
    ambiguous: t("ambiguous"),
  }[status] || status;
}

function renderLanguage() {
  document.documentElement.lang = state.language;
  for (const element of document.querySelectorAll("[data-i18n]")) {
    const key = element.dataset.i18n;
    element.textContent = t(key);
  }
  for (const element of document.querySelectorAll("[data-i18n-aria-label]")) {
    element.setAttribute("aria-label", t(element.dataset.i18nAriaLabel));
  }
  for (const element of document.querySelectorAll("[data-i18n-title]")) {
    element.title = t(element.dataset.i18nTitle);
  }
  for (const element of document.querySelectorAll("[data-i18n-placeholder]")) {
    element.placeholder = t(element.dataset.i18nPlaceholder);
  }
  const languageLabels = {
    en: t("languageEnglish"),
    pt: t("languagePortuguese"),
    es: t("languageSpanish"),
  };
  if (el.languageCode) {
    el.languageCode.textContent = languageLabels[state.language] || languageLabels.en;
  }
  for (const option of el.languageMenuItems) {
    option.setAttribute("aria-selected", String(option.dataset.lang === state.language));
  }
  el.languageButton?.setAttribute(
    "aria-expanded",
    String(!el.languageMenu?.classList.contains("hidden"))
  );
  if (el.searchInput) el.searchInput.placeholder = t("search");
  if (el.duplicatesSearch) el.duplicatesSearch.placeholder = t("search");
  if (el.conflictsSearch) el.conflictsSearch.placeholder = t("search");
  renderStatusFilter();
  renderDuplicateFilter();
  renderConflictFilter();
  renderTestStatusFilter();
  renderSelectionSummary();
  window.dispatchEvent(new CustomEvent("s3cc-language-changed", { detail: state.language }));
}

function renderTestStatusFilter() {
  if (!el.toolsTestStatus) return;
  const current = el.toolsTestStatus.value;
  const options = [
    ["", "—"],
    ["untested", t("untested")],
    ["working", t("working")],
    ["problem", t("problemStatus")],
    ["removed", t("removedStatus")],
  ];
  el.toolsTestStatus.innerHTML = "";
  for (const [value, label] of options) {
    const option = document.createElement("option");
    option.value = value;
    option.textContent = label;
    option.selected = value === current;
    el.toolsTestStatus.appendChild(option);
  }
}

function renderStatusFilter() {
  if (!el.statusFilterMenu) return;

  const options = [
    ["all", t("allStatuses")],
    ["classified", t("classified")],
    ["sim_export_merged", t("simExportMerged")],
    ["mixed", t("mixed")],
    ["needs_review", t("needsReview")],
    ["unknown", t("unknown")],
    ["invalid", t("invalid")],
  ];

  const counts = { all: state.items.length };
  for (const [value] of options) {
    if (value === "all") continue;
    counts[value] = state.items.filter((item) => item.status === value).length;
  }

  const active = options.find(([value]) => value === state.status) || options[0];
  el.statusFilterLabel.textContent = `${t("filter")}:`;
  el.statusFilterCurrent.textContent = `${active[1]} (${counts[active[0]] ?? 0})`;
  el.statusFilterToggleBtn.setAttribute("aria-expanded", String(state.isStatusFilterOpen));
  el.statusFilterToggleIcon.className =
    `fa-solid ${state.isStatusFilterOpen ? "fa-angle-up" : "fa-angle-down"}`;
  el.statusFilterMenu.classList.toggle("hidden", !state.isStatusFilterOpen);
  el.statusFilterMenu.innerHTML = "";

  for (const [value, label] of options) {
    const button = document.createElement("button");
    const activeFilter = value === state.status;
    button.type = "button";
    button.className =
      "lang-menu-item content-filter-menu-item" + (activeFilter ? " active" : "");
    button.textContent = `${label} (${counts[value] ?? 0})`;
    button.disabled = activeFilter;
    button.addEventListener("click", (event) => {
      event.stopPropagation();
      state.status = value;
      state.isStatusFilterOpen = false;
      persistPreferences();
      renderStatusFilter();
      renderResults();
    });
    el.statusFilterMenu.appendChild(button);
  }
}

function renderDuplicateFilter() {
  if (!el.duplicatesFilter) return;
  const options = [
    ["all", t("allDuplicateTypes")],
    ["exact_duplicate", t("exactDuplicate")],
    ["content_duplicate", t("contentDuplicate")],
    ["retexture", t("retexture")],
    ["recategorized_variant", t("recategorizedVariant")],
    ["related_variant", t("relatedVariant")],
  ];
  el.duplicatesFilter.innerHTML = "";
  for (const [value, label] of options) {
    const option = document.createElement("option");
    option.value = value;
    option.textContent = label;
    option.selected = state.duplicatesFilter === value;
    el.duplicatesFilter.appendChild(option);
  }
}

function duplicateKindLabel(kind) {
  return {
    exact_duplicate: t("exactDuplicate"),
    content_duplicate: t("contentDuplicate"),
    retexture: t("retexture"),
    recategorized_variant: t("recategorizedVariant"),
    related_variant: t("relatedVariant"),
  }[kind] || kind;
}

function duplicateExplanation(kind) {
  return {
    exact_duplicate: t("exactExplanation"),
    content_duplicate: t("contentExplanation"),
    retexture: t("retextureExplanation"),
    recategorized_variant: t("recategorizedExplanation"),
    related_variant: t("relatedExplanation"),
  }[kind] || "";
}

function flattenedDuplicateFindings() {
  const analysis = state.duplicatesAnalysis;
  if (!analysis) return [];
  return [
    ...(analysis.groups || []).map((item) => ({ ...item, findingType: "group" })),
    ...(analysis.relations || []).map((item) => ({ ...item, findingType: "relation" })),
  ];
}

// Exact groups come from whole-file SHA-256, not normalized content or
// shared-TGI relationships. A batch always retains at least one physical
// copy per group; the user can review/override the keeper before preview.
function exactDuplicateGroups() {
  return (state.duplicatesAnalysis?.groups || []).filter((group) =>
    group.kind === "exact_duplicate" && (group.members || []).length > 1 &&
    group.members.every((member) =>
      member.path && member.fileSha256 &&
      member.fileSha256 === group.members[0].fileSha256
    )
  );
}

function exactKeeperOrder(member) {
  const path = String(member.relativePath || member.path || "").replace(/\\/g, "/");
  const branch = /(^|\/)Overrides\//i.test(path) ? 0
    : /(^|\/)Packages\//i.test(path) ? 1 : 2;
  return [branch, path.split("/").length, path.toLocaleLowerCase()];
}

function compareExactKeepers(left, right) {
  const a = exactKeeperOrder(left);
  const b = exactKeeperOrder(right);
  return a[0] - b[0] || a[1] - b[1] || a[2].localeCompare(b[2]);
}

function exactSurvivorIsSafe(selection = state.quarantineSelected) {
  return exactDuplicateGroups().every((group) =>
    (group.members || []).some((member) => !selection.has(member.path))
  );
}

function retainedExactDuplicatePaths(selection = state.quarantineSelected) {
  const retained = new Set();
  for (const group of exactDuplicateGroups()) {
    if (!(group.members || []).some((member) => selection.has(member.path))) continue;
    for (const member of group.members || []) {
      if (!selection.has(member.path)) retained.add(member.path);
    }
  }
  return [...retained];
}

async function selectExactCopiesForQuarantine() {
  if (!state.folder || !state.duplicatesAnalysis || state.quarantineBusy ||
      state.scanning || state.analysisPipelineBusy || state.auditBusy ||
      workspaceReadOnly()) return;
  const groups = exactDuplicateGroups();
  const selected = new Set();
  for (const group of groups) {
    const ordered = [...group.members].sort(compareExactKeepers);
    for (const member of ordered.slice(1)) selected.add(member.path);
  }
  if (!groups.length || !selected.size) {
    state.duplicatesNotice = t("bulkExactNone");
    renderDuplicates();
    return;
  }
  state.quarantineSelected = selected;
  state.quarantineExactBatch = true;
  state.quarantinePlan = null;
  state.duplicatesError = "";
  state.duplicatesFilter = "exact_duplicate";
  el.duplicatesFilter.value = "exact_duplicate";
  state.duplicatesSearch = "";
  el.duplicatesSearch.value = "";
  persistPreferences();
  state.duplicatesNotice = `${t("bulkExactSummary")} (${groups.length} / ${selected.size})`;
  renderDuplicates();
  await buildQuarantinePreview();
}

function selectFilteredExactDuplicateGroups() {
  if (!state.duplicatesAnalysis || state.quarantineBusy || workspaceReadOnly()) return;
  const groups = exactDuplicateGroups();
  const exactPaths = new Set(groups.flatMap(group => (group.members || []).map(member => member.path)));
  if ([...state.quarantineSelected].some(path => !exactPaths.has(path))) {
    state.duplicatesError = t("exactOnlySelectionHint");
    renderDuplicates();
    return;
  }
  const visibleIds = new Set(visibleDuplicateFindings().filter(item =>
    item.findingType === "group" && item.kind === "exact_duplicate"
  ).map(item => item.id));
  const selected = new Set(state.quarantineSelected);
  for (const group of groups) {
    if (!visibleIds.has(group.id)) continue;
    const ordered = [...group.members].sort(compareExactKeepers);
    const keeper = ordered.find(member => !selected.has(member.path));
    if (!keeper) continue;
    for (const member of ordered) if (member.path !== keeper.path) selected.add(member.path);
  }
  state.quarantineSelected = selected;
  state.quarantineExactBatch = true;
  state.quarantinePlan = null;
  state.duplicatesError = "";
  state.duplicatesNotice = `${t("bulkExactSummary")} (${selected.size})`;
  renderDuplicates();
  renderDuplicatesPreview();
}

function selectExactGroupRows(ids, select = true) {
  const groups = exactDuplicateGroups().filter(group => ids.has(group.id));
  const exactPaths = new Set(exactDuplicateGroups().flatMap(group => group.members.map(m => m.path)));
  if ([...state.quarantineSelected].some(path => !exactPaths.has(path))) {
    state.duplicatesError = t("exactOnlySelectionHint");
    renderDuplicates();
    return;
  }
  const selected = new Set(state.quarantineSelected);
  for (const group of groups) {
    if (!select) {
      for (const member of group.members) selected.delete(member.path);
    } else {
      const ordered = [...group.members].sort(compareExactKeepers);
      const keeper = ordered.find(m => !selected.has(m.path));
      if (!keeper) continue;
      for (const member of ordered) if (member.path !== keeper.path) selected.add(member.path);
    }
  }
  state.quarantineSelected = selected;
  state.quarantineExactBatch = true;
  state.quarantinePlan = null;
  renderDuplicates();
  renderDuplicatesPreview();
}

function toggleExactGroupFromRow(group, selected, event) {
  if (event?.shiftKey && state.lastExactGroupAnchor) {
    const groups = visibleDuplicateFindings().filter(item =>
      item.findingType === "group" && item.kind === "exact_duplicate"
    );
    const from = groups.findIndex(g => g.id === state.lastExactGroupAnchor);
    const to = groups.findIndex(g => g.id === group.id);
    if (from >= 0 && to >= 0) {
      selectExactGroupRows(new Set(groups.slice(Math.min(from,to), Math.max(from,to)+1).map(g=>g.id)), selected);
      state.lastExactGroupAnchor = group.id;
      return;
    }
  }
  state.lastExactGroupAnchor = group.id;
  selectExactGroupRows(new Set([group.id]), selected);
}

function selectOnlyCurrentExactDuplicateGroup() {
  if (!state.duplicatesAnalysis || state.quarantineBusy || workspaceReadOnly()) return;
  const group = exactDuplicateGroups().find(item => item.id === state.duplicateSelectedId);
  if (!group) return;
  const sorted = [...group.members].sort(compareExactKeepers);
  const selected = new Set(sorted.slice(1).map(member => member.path));
  if (!selected.size) return;
  state.quarantineSelected = selected;
  state.quarantineExactBatch = true;
  state.quarantinePlan = null;
  state.duplicatesError = "";
  state.duplicatesNotice = `${t("bulkExactSummary")} (1 / ${selected.size})`;
  renderDuplicates();
  renderDuplicatesPreview();
}

function visibleDuplicateFindings() {
  const analysis = state.duplicatesAnalysis;
  const search = state.duplicatesSearch.trim().toLocaleLowerCase();
  if (
    duplicateVisibleMemo.analysis === analysis &&
    duplicateVisibleMemo.filter === state.duplicatesFilter &&
    duplicateVisibleMemo.search === search
  ) {
    return duplicateVisibleMemo.result;
  }

  const queryInstance = normalizedInstanceQuery(search);
  const result = flattenedDuplicateFindings().filter((item) => {
    if (state.duplicatesFilter !== "all" && item.kind !== state.duplicatesFilter) return false;
    if (!search) return true;

    const values = item.findingType === "group"
      ? (item.members || []).flatMap((member) => [member.name, member.relativePath, ...(member.instances || [])])
      : [
          item.left?.name,
          item.left?.relativePath,
          ...(item.left?.instances || []),
          item.right?.name,
          item.right?.relativePath,
          ...(item.right?.instances || []),
        ];

    const haystack = values.filter(Boolean).join("\n").toLocaleLowerCase();
    if (haystack.includes(search)) return true;
    if (!queryInstance) return false;
    return values.some((value) =>
      /^0x[0-9a-f]+$/i.test(String(value)) &&
      String(value).replace(/^0x/i, "").toUpperCase().padStart(16, "0") === queryInstance
    );
  });

  duplicateVisibleMemo.analysis = analysis;
  duplicateVisibleMemo.filter = state.duplicatesFilter;
  duplicateVisibleMemo.search = search;
  duplicateVisibleMemo.result = result;
  return result;
}

async function buildQuarantinePreview() {
  if (!state.folder || !state.quarantineSelected.size || state.quarantineBusy ||
      state.analysisPipelineBusy || state.scanning || state.auditBusy) return;
  if (!exactSurvivorIsSafe()) {
    state.duplicatesError = t("bulkExactUnsafe");
    renderDuplicatesPreview();
    renderDuplicateBatchControls();
    return;
  }
  state.quarantineBusy = true;
  state.quarantinePlan = null;
  renderDuplicatesPreview();
  renderDuplicateBatchControls();
  try {
    // Handpicked subsets of SHA-identical groups get the same survivor
    // safeguards and "N identical files" grouping as Select All.
    const exactPaths = new Set(exactDuplicateGroups().flatMap(group =>
      (group.members || []).map(member => member.path)));
    const useExactBatch = [...state.quarantineSelected].every(path => exactPaths.has(path));
    const command = useExactBatch
      ? "build_exact_duplicate_quarantine_plan" : "build_quarantine_plan";
    const folder = state.folder;
    const selected = [...state.quarantineSelected].sort();
    const plan = await invoke(command, {
      folder,
      selectedPaths: selected,
      ...(useExactBatch
        ? { retainedPaths: retainedExactDuplicatePaths() } : {}),
    });
    if (state.folder !== folder ||
        selected.join("\n") !== [...state.quarantineSelected].sort().join("\n")) return;
    state.quarantineExactBatch = useExactBatch;
    state.quarantinePlan = plan;
  } catch (error) {
    state.duplicatesError = String(error);
  } finally {
    state.quarantineBusy = false;
    renderDuplicatesPreview();
    renderDuplicateBatchControls();
  }
}

async function executeQuarantine() {
  if (!state.folder || !state.quarantinePlan?.canExecute ||
      !state.quarantineSelected.size || state.quarantineBusy ||
      !exactSurvivorIsSafe() || state.analysisPipelineBusy) return;
  state.quarantineBusy = true;
  renderDuplicatesPreview();
  try {
    const command = state.quarantineExactBatch
      ? "execute_exact_duplicate_quarantine" : "execute_quarantine";
    const result = await invoke(command, {
      folder: state.folder,
      selectedPaths: [...state.quarantineSelected],
      plannedQuarantineRoot: state.quarantinePlan.quarantineRoot,
      ...(state.quarantineExactBatch
        ? { retainedPaths: retainedExactDuplicatePaths() } : {}),
    });
    const quarantineNotice = `${t("quarantineComplete")}: ${result.moved}`;
    state.duplicatesNotice = quarantineNotice;
    state.postQuarantineNotice = { tab: "duplicates", text: quarantineNotice };
    state.quarantineSelected.clear();
    state.quarantinePlan = null;
    state.quarantineExactBatch = false;
    state.duplicatesAnalysis = null;
    state.duplicateSelectedId = "";
    closeDuplicateDetails();
    state.tab = "duplicates";
    persistPreferences();
    await Promise.all([refreshOperationHistory(), refreshCacheInfo()]);
    state.quarantineBusy = false;
    await scanFolder(false, true);
    state.tab = "duplicates";
    persistPreferences();
  } catch (error) {
    state.duplicatesError = String(error);
  } finally {
    state.quarantineBusy = false;
    state.pendingAction = "";
    el.confirmModal.classList.add("hidden");
    render();
  }
}

function openDuplicateDetails() {
  if (!state.duplicateSelectedId || !el.duplicateDetailsModal) return;
  renderDuplicatesPreview();
  renderDuplicateBatchControls();
  el.duplicateDetailsModal.classList.remove("hidden");
  el.duplicateDetailsModal.setAttribute("aria-hidden", "false");
}

function closeDuplicateDetails() {
  if (!el.duplicateDetailsModal) return;
  el.duplicateDetailsModal.classList.add("hidden");
  el.duplicateDetailsModal.setAttribute("aria-hidden", "true");
}

function libraryItemForDuplicateMember(member) {
  if (!member) return null;
  return state.items.find((candidate) =>
    candidate.path === member.path ||
    candidate.relativePath === member.relativePath
  ) || null;
}

function readableContentSource(value) {
  if (value === "the_sims_3_store") return "The Sims 3 Store";
  if (value === "custom_content") return "Custom Content";
  return value || "—";
}

async function loadDuplicateMemberPreview(member) {
  const path = member?.path;
  if (!path || !state.folder || state.packagePreviewLoading[path] || path in state.packagePreviews) {
    return;
  }

  state.packagePreviewLoading[path] = true;
  try {
    state.packagePreviews[path] = await invoke("get_package_preview", {
      folder: state.folder,
      packagePath: path,
    });
  } catch (error) {
    state.packagePreviewErrors[path] = String(error);
    state.packagePreviews[path] = { thumbnailBase64: null, mimeType: null };
  } finally {
    delete state.packagePreviewLoading[path];
    if (el.duplicateDetailsModal && !el.duplicateDetailsModal.classList.contains("hidden")) {
      renderDuplicatesPreview();
    }
    // Share the cached package thumbnails with Conflicts, without loading
    // previews for all findings in a potentially very large scan.
    if (state.tab === "conflicts" &&
        el.conflictDetailsModal && !el.conflictDetailsModal.classList.contains("hidden") &&
        (state.conflictsAnalysis?.findings || []).some(
      (finding) => finding.id === state.conflictSelectedId &&
        (finding.left?.path === path || finding.right?.path === path)
    )) renderConflictsPreview();
  }
}

function appendDuplicateMemberDetails(card, member, label = "") {
  const visual = document.createElement("div");
  visual.className = "duplicate-member-visual";

  const preview = state.packagePreviews[member?.path];
  if (preview?.thumbnailBase64) {
    const image = document.createElement("img");
    image.className = "duplicate-member-thumb";
    image.alt = member?.name || "";
    image.src = `data:${preview.mimeType || "image/png"};base64,${preview.thumbnailBase64}`;
    visual.appendChild(image);
  } else {
    const fallback = document.createElement("div");
    fallback.className = "duplicate-member-thumb duplicate-member-thumb-fallback";
    fallback.innerHTML = '<i class="fa-regular fa-image" aria-hidden="true"></i>';
    visual.appendChild(fallback);
    if (member?.path && !(member.path in state.packagePreviews) && !state.packagePreviewLoading[member.path]) {
      queueMicrotask(() => loadDuplicateMemberPreview(member));
    }
  }

  const body = document.createElement("div");
  body.className = "duplicate-member-body";

  if (label) {
    const mark = document.createElement("b");
    mark.className = "duplicate-member-mark";
    mark.textContent = label;
    card.appendChild(mark);
  }

  const name = document.createElement("strong");
  name.textContent = member?.name || "—";
  const path = document.createElement("code");
  path.textContent = member?.relativePath || "";
  body.append(name, path);

  const libraryItem = libraryItemForDuplicateMember(member);
  const details = document.createElement("div");
  details.className = "duplicate-member-meta-grid";

  const rows = [
    [t("contentType"), [libraryItem?.category, libraryItem?.subCategory].filter(Boolean).join(" · ") || "—"],
    [t("contentSource"), readableContentSource(libraryItem?.contentSource)],
    [t("scriptedContent"), libraryItem?.scripted ? t("yes") : t("no")],
    [t("resourceCount"), String(member?.resourceCount ?? 0)],
    [t("fileSize"), bytesLabel(member?.size ?? 0)],
    [t("fileHash"), member?.fileSha256 ? `${member.fileSha256.slice(0, 16)}…` : "—"],
  ];

  for (const [keyText, valueText] of rows) {
    const row = document.createElement("div");
    const key = document.createElement("span");
    key.textContent = keyText;
    const value = document.createElement("strong");
    value.textContent = valueText;
    row.append(key, value);
    details.appendChild(row);
  }

  const actions = document.createElement("div");
  actions.className = "duplicate-member-actions";
  const openButton = document.createElement("button");
  openButton.type = "button";
  openButton.className = "secondary-btn";
  openButton.textContent = t("openLocation");
  openButton.addEventListener("click", () => revealSafe(member?.path));
  actions.appendChild(openButton);

  body.append(details, actions);
  card.append(visual, body);
}

function renderDuplicatesPreview() {
  if (!el.duplicatesPreview) return;
  const item = flattenedDuplicateFindings().find(
    (finding) => finding.id === state.duplicateSelectedId
  );

  el.duplicatesPreview.innerHTML = "";
  if (!item) {
    const empty = document.createElement("div");
    empty.className = "preview-empty";
    empty.textContent = t("selectDuplicateFinding");
    el.duplicatesPreview.appendChild(empty);
    return;
  }

  const header = document.createElement("div");
  header.className = "preview-header";
  const title = document.createElement("h3");
  title.textContent = duplicateKindLabel(item.kind);
  const badge = document.createElement("span");
  badge.className = `duplicate-kind duplicate-kind-${item.kind}`;
  badge.textContent = item.findingType === "group"
    ? `${item.members?.length || 0} ${t("packages")}`
    : "2";
  header.append(title, badge);

  const reason = document.createElement("div");
  reason.className = "duplicate-reason";
  const reasonTitle = document.createElement("strong");
  reasonTitle.textContent = t("duplicateReason");
  const reasonText = document.createElement("p");
  reasonText.textContent = duplicateExplanation(item.kind);
  reason.append(reasonTitle, reasonText);

  el.duplicatesPreview.append(header, reason);

  if (item.findingType === "group") {
    const listTitle = document.createElement("h4");
    listTitle.textContent = t("duplicateMembers");
    el.duplicatesPreview.appendChild(listTitle);

    const members = document.createElement("div");
    members.className = "duplicate-member-list";
    const quarantineEligible =
      item.kind === "exact_duplicate" ||
      (item.kind === "content_duplicate" && !state.quarantineExactBatch);
    for (const member of item.members || []) {
      const card = document.createElement("article");
      card.className = "duplicate-member";
      if (quarantineEligible) {
        const selector = document.createElement("label");
        selector.className = "quarantine-member-select";
        const checkbox = document.createElement("input");
        checkbox.type = "checkbox";
        checkbox.checked = state.quarantineSelected.has(member.path);
        checkbox.disabled = state.quarantineBusy || state.analysisPipelineBusy || workspaceReadOnly();
        checkbox.title = t("selectForQuarantine");
        checkbox.addEventListener("change", () => {
          if (checkbox.checked) {
            state.quarantineSelected.add(member.path);
            if (!exactSurvivorIsSafe()) {
              state.quarantineSelected.delete(member.path);
              state.duplicatesError = t("bulkExactUnsafe");
            }
          } else {
            state.quarantineSelected.delete(member.path);
          }
          state.quarantinePlan = null;
          renderDuplicatesPreview();
          renderDuplicateBatchControls();
        });
        selector.appendChild(checkbox);
        card.appendChild(selector);
      }

      appendDuplicateMemberDetails(card, member);
      members.appendChild(card);
    }
    el.duplicatesPreview.appendChild(members);

    if (quarantineEligible) {
      const actions = document.createElement("div");
      actions.className = "quarantine-actions";
      const previewButton = document.createElement("button");
      previewButton.type = "button";
      previewButton.className = "secondary-btn";
      previewButton.textContent = t("previewQuarantine");
      previewButton.disabled = !state.quarantineSelected.size || state.quarantineBusy;
      previewButton.addEventListener("click", buildQuarantinePreview);

      const clearButton = document.createElement("button");
      clearButton.type = "button";
      clearButton.className = "secondary-btn";
      clearButton.textContent = t("clearSelection");
      clearButton.disabled = !state.quarantineSelected.size;
      clearButton.addEventListener("click", () => {
        state.quarantineSelected.clear();
        state.quarantinePlan = null;
        state.quarantineExactBatch = false;
        renderDuplicatesPreview();
        renderDuplicateBatchControls();
      });
      const summary = document.createElement("span");
      summary.className = "analysis-batch-summary";
      summary.textContent = `${state.quarantineSelected.size} ${t("conflictSelectedSummary")}`;
      actions.append(previewButton, clearButton, summary);
      el.duplicatesPreview.appendChild(actions);

      if (state.quarantinePlan) {
        const preview = document.createElement("div");
        preview.className = "quarantine-preview";
        const title = document.createElement("h4");
        title.textContent = t("quarantinePreviewOnly");
        const destination = document.createElement("code");
        destination.textContent =
          `${t("quarantineRoot")}: ${state.quarantinePlan.quarantineRoot}`;
        const stats = document.createElement("p");
        stats.textContent =
          `${t("quarantineReady")}: ${state.quarantinePlan.stats?.ready ?? 0} · ${t("blocked")}: ${state.quarantinePlan.stats?.blocked ?? 0}`;
        const manifest = document.createElement("pre");
        manifest.textContent = state.quarantinePlan.manifestPreview || "";
        preview.append(title, destination, stats, manifest);
        if (state.quarantinePlan.canExecute) {
          const execute = document.createElement("button");
          execute.type = "button";
          execute.className = "primary-btn";
          execute.textContent = t("executeQuarantine");
          execute.disabled = state.quarantineBusy || workspaceReadOnly();
          execute.addEventListener("click", () => openConfirm("quarantine"));
          preview.appendChild(execute);
        }
        el.duplicatesPreview.appendChild(preview);
      }
    }
  } else {
    const pair = document.createElement("div");
    pair.className = "duplicate-pair";
    for (const [label, member] of [["A", item.left], ["B", item.right]]) {
      const card = document.createElement("article");
      appendDuplicateMemberDetails(card, member, label);
      pair.appendChild(card);
    }
    el.duplicatesPreview.appendChild(pair);

    const metrics = document.createElement("div");
    metrics.className = "duplicate-metrics";
    const values = [
      [t("sharedResources"), item.sharedResourceCount],
      [t("identicalTgiPayloads"), item.identicalTgiPayloadCount],
      [t("changedSameTgi"), item.changedSameTgiCount],
      [t("sharedStructural"), item.sharedStructuralCount],
      [t("changedTextures"), item.changedTextureCount],
      [t("changedCatalog"), item.changedCatalogCount],
    ];
    for (const [label, value] of values) {
      const row = document.createElement("div");
      const span = document.createElement("span");
      span.textContent = label;
      const strong = document.createElement("strong");
      strong.textContent = String(value ?? 0);
      row.append(span, strong);
      metrics.appendChild(row);
    }
    el.duplicatesPreview.appendChild(metrics);

    if (false && ["retexture", "recategorized_variant", "related_variant"].includes(item.kind)) {
      const groupActions = document.createElement("div");
      groupActions.className = "quarantine-actions";
      const keepTogether = document.createElement("button");
      keepTogether.type = "button";
      keepTogether.className = "secondary-btn";
      keepTogether.textContent = t("keepTogetherGroups");
      keepTogether.addEventListener("click", () => {
        for (const member of [item.left, item.right]) {
          const match = state.items.find((candidate) =>
            candidate.path === member?.path || candidate.relativePath === member?.relativePath
          );
          if (match) state.metadataGroupSelected.add(match.path);
        }
        state.tab = "tools";
        state.toolsTab = "metadata";
        persistPreferences();
        render();
      });
      groupActions.appendChild(keepTogether);
      el.duplicatesPreview.appendChild(groupActions);
    }

    if (item.evidence?.length) {
      const evidenceTitle = document.createElement("h4");
      evidenceTitle.textContent = t("evidence");
      const list = document.createElement("ul");
      list.className = "duplicate-evidence";
      for (const evidence of item.evidence) {
        const li = document.createElement("li");
        li.textContent = evidence;
        list.appendChild(li);
      }
      el.duplicatesPreview.append(evidenceTitle, list);
    }
  }
}

function createDuplicateFindingRow(finding) {
  const button = document.createElement("button");
  button.type = "button";
  button.dataset.duplicateId = finding.id;
  button.className =
    "duplicate-row" + (finding.id === state.duplicateSelectedId ? " active" : "");

  const main = document.createElement("div");
  main.className = "duplicate-row-main";
  const kind = document.createElement("strong");
  kind.textContent = duplicateKindLabel(finding.kind);
  const names = document.createElement("span");
  names.textContent = finding.findingType === "group"
    ? (finding.members || []).map((member) => member.name).join(" · ")
    : `${finding.left?.name || "—"} ↔ ${finding.right?.name || "—"}`;
  main.append(kind, names);

  const count = document.createElement("span");
  count.className = `duplicate-kind duplicate-kind-${finding.kind}`;
  count.textContent = finding.findingType === "group"
    ? String(finding.members?.length || 0)
    : String(finding.sharedStructuralCount ?? 0);

  button.append(main, count);
  const exact = finding.findingType === "group" && finding.kind === "exact_duplicate";
  button.addEventListener("click", event => {
    if (exact && (event.shiftKey || event.ctrlKey || event.metaKey)) {
      const selected = !(finding.members || []).some(member => state.quarantineSelected.has(member.path));
      toggleExactGroupFromRow(finding, selected, event);
      return;
    }
    state.duplicateSelectedId = finding.id;
    // Selection remains global even when details of another group are opened.
    updateActiveVirtualRow(el.duplicatesList, "duplicateId", finding.id, "active");
    openDuplicateDetails();
  });
  if (!exact) return button;
  const wrapper = document.createElement("div");
  wrapper.className = "duplicate-row-shell";
  const checkbox = document.createElement("input");
  checkbox.type = "checkbox";
  checkbox.className = "duplicate-row-select";
  const selectedCount = (finding.members || []).filter(m => state.quarantineSelected.has(m.path)).length;
  checkbox.checked = selectedCount > 0;
  checkbox.indeterminate = selectedCount > 0 && selectedCount < finding.members.length - 1;
  wrapper.classList.toggle("selected", selectedCount > 0);
  checkbox.disabled = state.quarantineBusy || state.analysisPipelineBusy || workspaceReadOnly();
  checkbox.setAttribute("aria-label", `${t("selectForQuarantine")}: ${finding.members?.[0]?.name || finding.id}`);
  checkbox.addEventListener("click", event => {
    if (event.shiftKey && state.lastExactGroupAnchor) {
      event.preventDefault();
      toggleExactGroupFromRow(finding, selectedCount === 0, event);
    }
  });
  checkbox.addEventListener("change", event => toggleExactGroupFromRow(finding, event.currentTarget.checked, event));
  wrapper.append(checkbox, button);
  return wrapper;
}

function renderDuplicateVirtualRows(findings, force = false) {
  el.duplicatesList.dataset.fluentCount = String(findings.length);
  const view = virtualViews.duplicates;
  const changed = view.items !== findings;
  if (changed) {
    view.items = findings;
    view.start = -1;
    view.end = -1;
    el.duplicatesList.scrollTop = 0;
  }

  if (!findings.length) {
    const empty = document.createElement("div");
    empty.className = "list-empty";
    empty.textContent = t("noDuplicateFindings");
    el.duplicatesList.replaceChildren(empty);
    return;
  }

  const { start, end } = virtualRange(el.duplicatesList, findings.length, ANALYSIS_ROW_STRIDE);
  if (!force && !changed && start === view.start && end === view.end) return;
  view.start = start;
  view.end = end;

  const fragment = document.createDocumentFragment();
  if (start > 0) fragment.appendChild(virtualSpacer(start * ANALYSIS_ROW_STRIDE));
  for (let index = start; index < end; index += 1) {
    const row = createDuplicateFindingRow(findings[index]);
    row.dataset.fluentIndex = String(index);
    fragment.appendChild(row);
  }
  if (end < findings.length) {
    fragment.appendChild(virtualSpacer((findings.length - end) * ANALYSIS_ROW_STRIDE));
  }
  el.duplicatesList.replaceChildren(fragment);
}

function scheduleDuplicateVirtualRows() {
  const view = virtualViews.duplicates;
  if (view.raf) return;
  view.raf = window.requestAnimationFrame(() => {
    view.raf = 0;
    if (state.tab === "duplicates" && state.duplicatesAnalysis) {
      renderDuplicateVirtualRows(visibleDuplicateFindings());
    }
  });
}

function renderDuplicateBatchControls() {
  if (!el.duplicatesBatchPreview) return;
  const hasAnalysis = !!state.duplicatesAnalysis;
  const busy = state.quarantineBusy || state.analysisPipelineBusy ||
    state.scanning || state.duplicatesBusy || state.conflictsBusy ||
    state.auditBusy || state.executing;
  const count = state.quarantineSelected.size;
  el.duplicatesSelectExactBtn.disabled = !hasAnalysis ||
    !exactDuplicateGroups().length || busy || workspaceReadOnly();
  el.duplicatesSelectCurrentBtn.disabled = !hasAnalysis || busy || workspaceReadOnly() ||
    !exactDuplicateGroups().some(item => item.id === state.duplicateSelectedId);
  el.duplicatesSelectVisibleBtn.disabled = !hasAnalysis || busy || workspaceReadOnly() ||
    !visibleDuplicateFindings().some(item => item.findingType === "group" && item.kind === "exact_duplicate");
  el.duplicatesPreviewAllBtn.disabled = !hasAnalysis || !count || busy ||
    !exactSurvivorIsSafe() || workspaceReadOnly();
  el.duplicatesClearSelectionBtn.disabled = !count || busy;
  el.duplicatesBatchSummary.textContent = hasAnalysis
    ? `${t("quarantineSelectionCount")}: ${count}. ${t("quarantineBatchHint")}`
    : "";
  el.duplicatesBatchPreview.replaceChildren();
  el.duplicatesBatchPreview.classList.toggle("hidden", !state.quarantinePlan || !count);
  if (!state.quarantinePlan || !count) return;

  const title = document.createElement("h4");
  title.textContent = t("quarantinePreviewOnly");
  const destination = document.createElement("code");
  destination.textContent =
    `${t("quarantineRoot")}: ${state.quarantinePlan.quarantineRoot}`;
  const summary = document.createElement("p");
  summary.textContent =
    `${t("quarantineReady")}: ${state.quarantinePlan.stats?.ready || 0} · ${t("blocked")}: ${state.quarantinePlan.stats?.blocked || 0}`;
  const manifest = document.createElement("details");
  const heading = document.createElement("summary");
  heading.textContent = t("manifestPreview");
  const text = document.createElement("pre");
  text.textContent = state.quarantinePlan.manifestPreview || "";
  manifest.append(heading, text);
  el.duplicatesBatchPreview.append(title, destination, summary, manifest);

  if (state.quarantinePlan.canExecute) {
    const execute = document.createElement("button");
    execute.type = "button";
    execute.className = "primary-btn";
    execute.textContent = t("executeQuarantine");
    execute.disabled = busy || workspaceReadOnly() || !exactSurvivorIsSafe() ||
      Number(state.quarantinePlan.stats?.selected) !== count;
    execute.addEventListener("click", () => openConfirm("quarantine"));
    el.duplicatesBatchPreview.appendChild(execute);
  }
}

function renderDuplicates() {
  if (!el.analyzeDuplicatesBtn) return;
  renderDuplicateBatchControls();
  el.analyzeDuplicatesBtn.querySelector("[data-i18n]")?.replaceChildren(
    document.createTextNode(t(state.duplicatesAnalysis ? "reanalyzeDuplicates" : "analyzeDuplicates"))
  );
  el.analyzeDuplicatesBtn.disabled =
    !state.folder ||
    state.duplicatesBusy ||
    state.scanning ||
    state.conflictsBusy ||
    state.structureBusy ||
    state.toolsBusy;

  const stats = state.duplicatesAnalysis?.stats || {};
  el.dupStatPackages.textContent = stats.packagesScanned ?? 0;
  el.dupStatExact.textContent = stats.exactGroups ?? 0;
  el.dupStatContent.textContent = stats.contentGroups ?? 0;
  el.dupStatRetexture.textContent = stats.retextureRelations ?? 0;
  el.dupStatRelated.textContent =
    (stats.recategorizedRelations ?? 0) + (stats.relatedVariantRelations ?? 0);

  if (state.duplicatesBusy) {
    el.duplicatesState.textContent = t("analyzingDuplicates");
    el.duplicatesState.className = "scan-state busy";
  } else if (state.duplicatesError) {
    el.duplicatesState.textContent = `${t("duplicatesFailed")}: ${state.duplicatesError}`;
    el.duplicatesState.className = "scan-state error";
  } else if (state.duplicatesNotice) {
    el.duplicatesState.textContent = state.duplicatesNotice;
    el.duplicatesState.className = "scan-state";
  } else if (state.duplicatesAnalysis) {
    const unreadable = stats.unreadablePackages ?? 0;
    const parts = [t("duplicatesReady")];
    if (unreadable) parts.push(`${t("unreadablePackages")}: ${unreadable}`);
    if (stats.cacheHits) parts.push(`${t("cacheReused")}: ${stats.cacheHits}`);
    if (stats.cacheMisses) parts.push(`${t("cacheUpdated")}: ${stats.cacheMisses}`);
    if (stats.variantAnalysisTruncated) parts.push(t("variantAnalysisTruncated"));
    el.duplicatesState.textContent = parts.join(" · ");
    el.duplicatesState.className = "scan-state success";
  } else {
    el.duplicatesState.textContent = "";
    el.duplicatesState.className = "scan-state";
  }

  el.duplicatesState.dataset.fluentSeverity =
    !state.duplicatesBusy && !state.duplicatesError && state.duplicatesAnalysis &&
    (Number(stats.unreadablePackages || 0) > 0 || !!stats.variantAnalysisTruncated)
      ? "warning" : "";

  const hasAnalysis = !!state.duplicatesAnalysis;
  el.duplicatesEmpty.classList.toggle("hidden", hasAnalysis);
  el.duplicatesResults.classList.toggle("hidden", !hasAnalysis);
  if (!hasAnalysis) {
    virtualViews.duplicates.items = null;
    renderDuplicatesPreview();
    return;
  }

  const findings = visibleDuplicateFindings();
  if (!findings.some((item) => item.id === state.duplicateSelectedId)) {
    state.duplicateSelectedId = findings[0]?.id || "";
  }
  renderDuplicateVirtualRows(findings);
  if (!el.duplicateDetailsModal || el.duplicateDetailsModal.classList.contains("hidden")) {
    return;
  }
  renderDuplicatesPreview();
}

function renderConflictFilter() {
  if (!el.conflictsFilter) return;
  const options = [
    ["attention", t("conflictNeedsReview")],
    ["all", t("allConflictTypes")],
    ["inactive", t("conflictOutsideLoad")],
    ["script_conflict", t("scriptConflict")],
    ["gameplay_override", t("gameplayOverride")],
    ["catalog_override", t("catalogOverride")],
    ["visual_override", t("visualOverride")],
    ["text_override", t("textOverride")],
    ["potential_conflict", t("potentialConflict")],
    ["filename_collision_review", t("filenameCollisionReview")],
    ["cas_morph_overlap", t("casMorphOverlap")],
    ["mixed_override", t("mixedOverride")],
    ["shared_identical", t("sharedIdentical")],
    ["intentional_override", t("intentionalOverride")],
    ["ignored_session", t("ignoredSession")],
  ];
  el.conflictsFilter.innerHTML = "";
  for (const [value, label] of options) {
    const option = document.createElement("option");
    option.value = value;
    option.textContent = label;
    option.selected = state.conflictsFilter === value;
    el.conflictsFilter.appendChild(option);
  }
}

function isInactiveConflictFinding(finding, analysis = state.conflictsAnalysis) {
  return !!analysis?.resourceCfg?.precedenceReliable &&
    ["unmatched", "partially_matched"].includes(finding?.loadOrderStatus);
}

// Prioritization never nominates a file for removal. It only orders
// *active*, differing payload overlaps for manual inspection.
function conflictRiskScore(finding) {
  if (!finding || finding.differentPayloadCount === 0 ||
      isInactiveConflictFinding(finding)) return 0;
  const impacts = new Set([finding.kind, ...(finding.impactKinds || [])]);
  const impactScore = [
    ["script_conflict", 600],
    ["gameplay_override", 500],
    ["catalog_override", 400],
    ["visual_override", 300],
    ["cas_morph_overlap", 250],
    ["text_override", 200],
    ["potential_conflict", 150],
  ].reduce((maximum, [kind, score]) => impacts.has(kind) ? Math.max(maximum, score) : maximum, 0);
  if (!impactScore) return 0;
  const loadUncertainty = finding.loadOrderStatus === "same_priority" ? 35
    : finding.loadOrderStatus === "resolved_by_priority" ? 5 : 20;
  return impactScore + loadUncertainty +
    Math.min(25, Number(finding.differentPayloadCount || 0));
}

function conflictPriorityLabel(finding) {
  const score = conflictRiskScore(finding);
  return score >= 500 ? t("conflictPriorityUrgent")
    : score >= 300 ? t("conflictPriorityHigh")
      : score > 0 ? t("conflictPriorityReview") : t("conflictPriorityInfo");
}

function toggleConflictReviewSelection(id, selected, row) {
  state.lastConflictReviewAnchor = id;
  if (selected) state.conflictReviewSelected.add(id);
  else state.conflictReviewSelected.delete(id);
  // Keep the checkbox focused for keyboard use. Rebuild the virtual list
  // only when the selected-only filter actually changes visible rows.
  if (!state.conflictsSelectedOnly) {
    row.classList.toggle("selected", selected);
    renderConflictBatchControls();
    return;
  }
  if (!state.conflictReviewSelected.size) state.conflictsSelectedOnly = false;
  conflictVisibleMemo.analysis = null;
  renderConflicts();
  renderConflictVirtualRows(visibleConflictFindings(), true);
}

function selectConflictReviewRange(targetId, select = true) {
  const visible = visibleConflictFindings();
  const from = visible.findIndex(f => f.id === state.lastConflictReviewAnchor);
  const to = visible.findIndex(f => f.id === targetId);
  if (from < 0 || to < 0) return false;
  for (const item of visible.slice(Math.min(from, to), Math.max(from, to) + 1)) {
    if (select) state.conflictReviewSelected.add(item.id);
    else state.conflictReviewSelected.delete(item.id);
  }
  state.lastConflictReviewAnchor = targetId;
  if (!state.conflictReviewSelected.size) state.conflictsSelectedOnly = false;
  conflictVisibleMemo.analysis = null;
  renderConflicts();
  renderConflictVirtualRows(visibleConflictFindings(), true);
  return true;
}

function toggleConflictReviewFromRow(finding, event) {
  if (event.shiftKey && selectConflictReviewRange(finding.id)) return;
  const selected = !state.conflictReviewSelected.has(finding.id);
  state.lastConflictReviewAnchor = finding.id;
  if (selected) state.conflictReviewSelected.add(finding.id);
  else state.conflictReviewSelected.delete(finding.id);
  if (!state.conflictReviewSelected.size) state.conflictsSelectedOnly = false;
  conflictVisibleMemo.analysis = null;
  renderConflicts();
  renderConflictVirtualRows(visibleConflictFindings(), true);
}

function selectAllVisibleConflicts() {
  if (!state.conflictsAnalysis || state.reviewBusy || state.conflictsBusy) return;
  // Filter is a view concern; selection is retained across filters.
  for (const finding of visibleConflictFindings()) state.conflictReviewSelected.add(finding.id);
  conflictVisibleMemo.analysis = null;
  renderConflicts();
  renderConflictVirtualRows(visibleConflictFindings(), true);
}

function selectAllDetectedConflicts() {
  if (!state.conflictsAnalysis || state.reviewBusy || state.conflictsBusy) return;
  // Dashboard-inspired Select-by-category: select across the complete scan,
  // not just current search/filter. Exclude identical/shared-only findings:
  // these are not conflicts requiring action.
  for (const finding of state.conflictsAnalysis.findings || []) {
    if (finding.kind !== "shared_identical" && finding.differentPayloadCount > 0) {
      state.conflictReviewSelected.add(finding.id);
    }
  }
  conflictVisibleMemo.analysis = null;
  renderConflicts();
  renderConflictVirtualRows(visibleConflictFindings(), true);
}

function clearConflictReviewSelection() {
  state.conflictReviewSelected.clear();
  state.conflictsSelectedOnly = false;
  conflictVisibleMemo.analysis = null;
  renderConflicts();
  renderConflictVirtualRows(visibleConflictFindings(), true);
}

function prioritizeSelectedConflicts() {
  if (!state.conflictsAnalysis || !state.conflictReviewSelected.size) return;
  // Only filter and rank review findings: never automatically nominate
  // a conflicting package for quarantine or remove it.
  state.conflictsSelectedOnly = true;
  // Prioritize all checked findings, including those selected under another
  // filter. Do not silently omit hidden selections from the review list.
  state.conflictsFilter = "all";
  state.conflictsSearch = "";
  el.conflictsFilter.value = "all";
  el.conflictsSearch.value = "";
  persistPreferences();
  conflictVisibleMemo.analysis = null;
  renderConflicts();
  renderConflictVirtualRows(visibleConflictFindings(), true);
}

function reviewSelectedConflicts() {
  if (!state.conflictsAnalysis || !state.conflictReviewSelected.size ||
      state.reviewBusy || state.conflictsBusy) return;
  prioritizeSelectedConflicts();
  const findings = visibleConflictFindings();
  const selected = findings.find((finding) =>
    state.conflictReviewSelected.has(finding.id)
  ) || findings[0];
  if (!selected) return;
  state.conflictSelectedId = selected.id;
  openConflictDetails();
}

function renderConflictNavigation() {
  if (!el.conflictPreviousBtn || !el.conflictNextBtn) return;
  const findings = visibleConflictFindings();
  const index = findings.findIndex((finding) => finding.id === state.conflictSelectedId);
  const hasCurrent = index >= 0;
  el.conflictPreviousBtn.disabled = !hasCurrent || index <= 0;
  el.conflictNextBtn.disabled = !hasCurrent || index >= findings.length - 1;
}

function navigateConflictDetails(direction) {
  const findings = visibleConflictFindings();
  const index = findings.findIndex((finding) => finding.id === state.conflictSelectedId);
  if (index < 0) return;
  const target = findings[index + direction];
  if (!target) return;
  state.conflictSelectedId = target.id;
  renderConflictVirtualRows(findings, true);
  renderConflictsPreview();
  renderConflictNavigation();
}

async function applySelectedConflictReviewMark(mark) {
  if (!state.folder || !state.conflictsAnalysis || !state.conflictReviewSelected.size ||
      state.reviewBusy || state.quarantineBusy || state.scanning) return;
  const targets = (state.conflictsAnalysis.findings || []).filter(
    finding => state.conflictReviewSelected.has(finding.id)
  );
  if (!targets.length) return;
  if (mark === "reviewed" || mark === "ignored") {
    for (const finding of targets) state.conflictMarks[finding.id] = mark;
    conflictMarksVersion += 1;
    state.conflictsNotice = `${targets.length} · ${t(mark === "reviewed" ? "batchMarkedReviewed" : "batchMarkedIgnored")}`;
    renderConflicts();
    return;
  }
  state.reviewBusy = true;
  renderConflicts();
  try {
    const seen = new Set();
    const changes = [];
    for (const finding of targets) {
      if (mark === "intentional" && isInactiveConflictFinding(finding)) continue;
      if (mark === "clear" && !state.persistentConflictMarks[finding.decisionKey]) continue;
      const left = finding.left, right = finding.right;
      if (!finding.decisionKey || !left?.fileSha256 || !right?.fileSha256 ||
          !left?.relativePath || !right?.relativePath ||
          seen.has(finding.decisionKey)) continue;
      seen.add(finding.decisionKey);
      changes.push({
        decisionKey: finding.decisionKey,
        mark: mark === "intentional" ? "intentional_override" : null,
        leftSha256: left.fileSha256,
        rightSha256: right.fileSha256,
        leftRelativePath: left.relativePath,
        rightRelativePath: right.relativePath,
      });
    }
    if (mark === "intentional" && !changes.length) {
      state.conflictsNotice = t("noEligibleConflictDecisions");
      return;
    }
    if (changes.length) {
      const records = await invoke("set_conflict_decisions_bulk", {
        folder: state.folder, changes,
      });
      applyPersistentDecisionRecords(records);
    }
    for (const finding of targets) delete state.conflictMarks[finding.id];
    conflictMarksVersion += 1;
    state.conflictsNotice = `${mark === "intentional" ? changes.length : targets.length} · ${t(mark === "intentional" ? "batchMarkedIntentional" : "batchMarksCleared")}`;
  } catch (error) {
    state.conflictsError = String(error);
  } finally {
    state.reviewBusy = false;
    renderConflicts();
    renderAuditPanel();
    renderStructure();
  }
}

function conflictSelectionSafe(selection = state.conflictQuarantineSelected) {
  return (state.conflictsAnalysis?.findings || []).every((finding) =>
    !finding.differentPayloadCount ||
    !(selection.has(finding.left?.path) && selection.has(finding.right?.path))
  );
}

async function buildConflictQuarantinePreview() {
  if (!state.folder || !state.conflictsAnalysis ||
      !state.conflictQuarantineSelected.size ||
      state.quarantineBusy || state.scanning || state.analysisPipelineBusy ||
      state.duplicatesBusy || state.conflictsBusy || state.auditBusy) return;
  if (!conflictSelectionSafe()) {
    state.conflictsError = t("conflictBothSelected");
    renderConflicts();
    return;
  }
  const folder = state.folder;
  const selectedPaths = [...state.conflictQuarantineSelected].sort();
  state.quarantineBusy = true;
  state.conflictsError = "";
  state.conflictQuarantinePlan = null;
  renderConflicts();
  try {
    const plan = await invoke("build_quarantine_plan", {
      folder, selectedPaths,
    });
    if (state.folder !== folder ||
        selectedPaths.join("\n") !== [...state.conflictQuarantineSelected].sort().join("\n")) return;
    state.conflictQuarantinePlan = plan;
    if (!state.conflictSelectedId) {
      state.conflictSelectedId = visibleConflictFindings()[0]?.id || "";
    }
    if (state.conflictSelectedId) openConflictDetails();
  } catch (error) {
    state.conflictsError = String(error);
  } finally {
    state.quarantineBusy = false;
    renderConflicts();
  }
}

async function executeConflictQuarantine() {
  if (!state.folder || !state.conflictQuarantinePlan?.canExecute ||
      !state.conflictQuarantineSelected.size || !conflictSelectionSafe() ||
      state.quarantineBusy || state.scanning || state.analysisPipelineBusy ||
      state.duplicatesBusy || state.conflictsBusy ||
      state.conflictQuarantinePlan.stats?.selected !== state.conflictQuarantineSelected.size) return;
  const folder = state.folder;
  state.quarantineBusy = true;
  renderConflicts();
  try {
    const result = await invoke("execute_quarantine", {
      folder,
      selectedPaths: [...state.conflictQuarantineSelected],
      plannedQuarantineRoot: state.conflictQuarantinePlan.quarantineRoot,
    });
    const quarantineNotice = `${t("quarantineComplete")}: ${result.moved}`;
    state.conflictsNotice = quarantineNotice;
    state.postQuarantineNotice = { tab: "conflicts", text: quarantineNotice };
    state.conflictQuarantineSelected.clear();
    state.conflictQuarantinePlan = null;
    state.conflictsAnalysis = null;
    state.conflictSelectedId = "";
    state.conflictReviewSelected.clear();
    state.conflictsSelectedOnly = false;
    closeConflictDetails();
    state.tab = "conflicts";
    persistPreferences();
    await Promise.all([refreshOperationHistory(), refreshCacheInfo()]);
    state.quarantineBusy = false;
    await scanFolder(false, true);
    state.tab = "conflicts";
    persistPreferences();
  } catch (error) {
    state.conflictsError = String(error);
  } finally {
    state.quarantineBusy = false;
    state.pendingAction = "";
    el.confirmModal.classList.add("hidden");
    el.confirmModal.setAttribute("aria-hidden", "true");
    render();
  }
}

function conflictKindLabel(kind) {
  return {
    shared_identical: t("sharedIdentical"),
    visual_override: t("visualOverride"),
    catalog_override: t("catalogOverride"),
    gameplay_override: t("gameplayOverride"),
    script_conflict: t("scriptConflict"),
    text_override: t("textOverride"),
    potential_conflict: t("potentialConflict"),
    filename_collision_review: t("filenameCollisionReview"),
    cas_morph_overlap: t("casMorphOverlap"),
    mixed_override: t("mixedOverride"),
  }[kind] || kind;
}

function loadOrderExplanation(finding) {
  return {
    resolved_by_priority: t("higherPriorityWins"),
    same_priority: t("samePriorityUnknown"),
    partially_matched: t("partialPriorityUnknown"),
    unmatched: t("unmatchedPriorityUnknown"),
    resource_cfg_missing: t("missingResourceCfg"),
    advanced_cfg_unresolved: t("advancedCfgUnknown"),
  }[finding?.loadOrderStatus] || t("samePriorityUnknown");
}

function conflictExplanation(kind) {
  return {
    shared_identical: t("conflictSamePayloadExplanation"),
    visual_override: t("conflictVisualExplanation"),
    catalog_override: t("conflictCatalogExplanation"),
    gameplay_override: t("conflictGameplayExplanation"),
    script_conflict: t("conflictScriptExplanation"),
    text_override: t("conflictTextExplanation"),
    potential_conflict: t("conflictPotentialExplanation"),
    filename_collision_review: t("conflictFilenameCollisionExplanation"),
    cas_morph_overlap: t("conflictCasMorphExplanation"),
    mixed_override: t("conflictMixedExplanation"),
  }[kind] || "";
}

function visibleConflictFindings() {
  const analysis = state.conflictsAnalysis;
  const findings = analysis?.findings || [];
  const search = state.conflictsSearch.trim().toLocaleLowerCase();
  if (
    conflictVisibleMemo.analysis === analysis &&
    conflictVisibleMemo.filter === state.conflictsFilter &&
    conflictVisibleMemo.search === search &&
    conflictVisibleMemo.marksVersion === conflictMarksVersion
  ) {
    return conflictVisibleMemo.result;
  }

  const queryInstance = normalizedInstanceQuery(search);
  const result = findings.filter((item) => {
    const mark = effectiveConflictMark(item);
    if (state.conflictsFilter === "ignored_session") {
      if (mark !== "ignored") return false;
    } else if (state.conflictsFilter === "intentional_override") {
      if (mark !== "intentional") return false;
    } else {
      if (mark === "ignored" &&
          !(state.conflictsSelectedOnly && state.conflictReviewSelected.has(item.id))) return false;
      const outsideLoadRules = isInactiveConflictFinding(item, analysis);
      // A specific conflict category is an actionable view by default.
      // Inactive overlaps remain accessible in All findings / Outside loading rules.
      if (outsideLoadRules &&
          !["all", "inactive"].includes(state.conflictsFilter)) return false;
      if (state.conflictsFilter === "attention" &&
          (item.severity === "info" || mark === "intentional" || mark === "reviewed" ||
            (item.differentPayloadCount === 0 && item.kind !== "filename_collision_review"))) return false;
      if (state.conflictsFilter === "inactive" && !outsideLoadRules) return false;
      if (state.conflictsFilter !== "all" &&
          state.conflictsFilter !== "attention" && state.conflictsFilter !== "inactive") {
        const matchesPrimary = item.kind === state.conflictsFilter;
        const matchesImpact = (item.impactKinds || []).includes(state.conflictsFilter);
        if (!matchesPrimary && !matchesImpact) return false;
      }
    }
    if (!search) return true;

    const values = [
      item.left?.name,
      item.left?.relativePath,
      item.right?.name,
      item.right?.relativePath,
      ...(item.evidence || []).map((evidence) => evidence.instanceHex),
    ].filter(Boolean);
    const haystack = values.join("\n").toLocaleLowerCase();
    if (haystack.includes(search)) return true;
    if (!queryInstance) return false;
    return values.some((value) =>
      /^0x[0-9a-f]+$/i.test(String(value)) &&
      String(value).replace(/^0x/i, "").toUpperCase().padStart(16, "0") === queryInstance
    );
  });

  if (state.conflictsSelectedOnly) {
    for (let i = result.length - 1; i >= 0; i--) {
      if (!state.conflictReviewSelected.has(result[i].id)) result.splice(i, 1);
    }
  }
  if (!["ignored_session", "intentional_override"].includes(state.conflictsFilter)) {
    result.sort((a, b) => conflictRiskScore(b) - conflictRiskScore(a) ||
      (b.differentPayloadCount || 0) - (a.differentPayloadCount || 0) ||
      String(a.id).localeCompare(String(b.id))
    );
  }
  conflictVisibleMemo.analysis = analysis;
  conflictVisibleMemo.filter = state.conflictsFilter;
  conflictVisibleMemo.search = search;
  conflictVisibleMemo.marksVersion = conflictMarksVersion;
  conflictVisibleMemo.result = result;
  return result;
}

function openConflictDetails() {
  if (!state.conflictSelectedId || !el.conflictDetailsModal) return;
  el.conflictDetailsModal.classList.remove("hidden");
  el.conflictDetailsModal.setAttribute("aria-hidden", "false");
  renderConflictsPreview();
  el.conflictDetailsCloseBtn?.focus();
}

function closeConflictDetails() {
  if (!el.conflictDetailsModal) return;
  el.conflictDetailsModal.classList.add("hidden");
  el.conflictDetailsModal.setAttribute("aria-hidden", "true");
}

function renderConflictsPreview() {
  renderConflictNavigation();
  if (!el.conflictsPreview || el.conflictDetailsModal?.classList.contains("hidden")) return;
  const finding = (state.conflictsAnalysis?.findings || []).find(
    (item) => item.id === state.conflictSelectedId
  );

  el.conflictsPreview.innerHTML = "";
  if (!finding) {
    const empty = document.createElement("div");
    empty.className = "preview-empty";
    empty.textContent = t("selectConflictFinding");
    el.conflictsPreview.appendChild(empty);
    return;
  }

  const sessionMark = effectiveConflictMark(finding);
  const isInactive = isInactiveConflictFinding(finding);
  const header = document.createElement("div");
  header.className = "preview-header";
  const title = document.createElement("h3");
  title.textContent = isInactive
    ? t("conflictInactiveOverlap")
    : sessionMark === "intentional" ? t("intentionalOverride") : conflictKindLabel(finding.kind);
  const badge = document.createElement("span");
  badge.className = `conflict-kind conflict-kind-${finding.severity}`;
  badge.textContent = finding.severity?.toUpperCase() || "";
  header.append(title, badge);

  const reason = document.createElement("div");
  reason.className = "conflict-reason";
  const reasonTitle = document.createElement("strong");
  reasonTitle.textContent = t("conflictReason");
  const reasonText = document.createElement("p");
  reasonText.textContent = isInactive
    ? t("conflictInactiveExplanation")
    : conflictExplanation(finding.kind);
  reason.append(reasonTitle, reasonText);
  const priority = document.createElement("p");
  priority.className = "conflict-priority-note";
  priority.textContent = `${conflictPriorityLabel(finding)} · ${t("conflictPriorityReason")}`;
  reason.appendChild(priority);
  // Keep the technical type visible, without suggesting it affects gameplay now.
  if (isInactive) {
    const technicalType = document.createElement("p");
    technicalType.textContent = `${t("impact")}: ${conflictKindLabel(finding.kind)}`;
    reason.appendChild(technicalType);
  }

  if (sessionMark) {
    const markNotice = document.createElement("div");
    markNotice.className = "conflict-session-mark";
    markNotice.textContent =
      sessionMark === "intentional" ? t("savedIntentionalOverride")
        : sessionMark === "reviewed" ? t("reviewedSession") : t("ignoredSession");
    reason.appendChild(markNotice);
  }

  const markActions = document.createElement("div");
  markActions.className = "conflict-mark-actions";
  const intentionalButton = document.createElement("button");
  intentionalButton.type = "button";
  intentionalButton.className = "secondary-btn";
  intentionalButton.textContent = t("markIntentional");
  intentionalButton.disabled = isInactive || sessionMark === "intentional" || state.reviewBusy;
  intentionalButton.addEventListener("click", () => setConflictMark(finding, "intentional"));

  const ignoreButton = document.createElement("button");
  ignoreButton.type = "button";
  ignoreButton.className = "secondary-btn";
  ignoreButton.textContent = t("ignoreSession");
  ignoreButton.disabled = sessionMark === "ignored" || state.reviewBusy;
  ignoreButton.addEventListener("click", () => setConflictMark(finding, "ignored"));

  markActions.append(intentionalButton, ignoreButton);
  if (sessionMark) {
    const clearButton = document.createElement("button");
    clearButton.type = "button";
    clearButton.className = "secondary-btn";
    clearButton.textContent = t("clearMark");
    clearButton.addEventListener("click", () => setConflictMark(finding, null));
    markActions.appendChild(clearButton);
  }
  reason.appendChild(markActions);

  const pair = document.createElement("div");
  pair.className = "conflict-pair";
  for (const [label, member] of [["A", finding.left], ["B", finding.right]]) {
    const card = document.createElement("article");
    const mark = document.createElement("b");
    mark.textContent = label;
    const visual = document.createElement("div");
    visual.className = "conflict-member-visual";
    const preview = state.packagePreviews[member?.path];
    if (preview?.thumbnailBase64) {
      const image = document.createElement("img");
      image.className = "conflict-member-thumb";
      image.alt = member?.name || "";
      image.src = `data:${preview.mimeType || "image/png"};base64,${preview.thumbnailBase64}`;
      visual.appendChild(image);
    } else {
      const fallback = document.createElement("div");
      fallback.className = "conflict-member-thumb conflict-member-fallback";
      fallback.textContent = t("conflictPreviewUnavailable");
      visual.appendChild(fallback);
      if (member?.path && !(member.path in state.packagePreviews) &&
          !state.packagePreviewLoading[member.path]) {
        queueMicrotask(() => loadDuplicateMemberPreview(member));
      }
    }
    const name = document.createElement("strong");
    name.textContent = member?.name || "—";
    const path = document.createElement("code");
    path.textContent = member?.relativePath || "";

    const priority = document.createElement("small");
    priority.textContent =
      `${t("loadPriority")}: ${member?.loadPriority ?? "—"}`;

    const rule = document.createElement("small");
    rule.textContent =
      `${t("matchingRule")}: ${member?.loadRule || "—"}`;

    const open = document.createElement("button");
    open.type = "button";
    open.className = "secondary-btn conflict-member-open";
    open.textContent = t("openLocation");
    open.addEventListener("click", () => revealSafe(member?.path));
    card.append(mark, visual, name, path, priority, rule, open);
    if (!isInactive && finding.differentPayloadCount > 0 && !sessionMark) {
      const pick = document.createElement("button");
      pick.type = "button";
      pick.className = "secondary-btn";
      pick.textContent = t("chooseConflictFile");
      pick.setAttribute("aria-pressed", String(state.conflictQuarantineSelected.has(member?.path)));
      pick.disabled = state.quarantineBusy || state.analysisPipelineBusy || workspaceReadOnly();
      pick.addEventListener("click", () => {
        const candidate = member?.path;
        if (!candidate) return;
        if (state.conflictQuarantineSelected.has(candidate)) {
          state.conflictQuarantineSelected.delete(candidate);
        } else {
          const other = label === "A" ? finding.right?.path : finding.left?.path;
          if (other) state.conflictQuarantineSelected.delete(other);
          state.conflictQuarantineSelected.add(candidate);
          if (!conflictSelectionSafe()) {
            state.conflictQuarantineSelected.delete(candidate);
            state.conflictsError = t("conflictBothSelected");
          }
        }
        state.conflictQuarantinePlan = null;
        renderConflictsPreview();
        renderConflictBatchControls();
      });
      card.appendChild(pick);
    }
    pair.appendChild(card);
  }

  const metrics = document.createElement("div");
  metrics.className = "conflict-metrics";
  for (const [label, value] of [
    [t("sharedCount"), finding.sharedResourceCount],
    [t("samePayloadCount"), finding.identicalPayloadCount],
    [t("differentPayloadCount"), finding.differentPayloadCount],
  ]) {
    const row = document.createElement("div");
    const span = document.createElement("span");
    span.textContent = label;
    const strong = document.createElement("strong");
    strong.textContent = String(value ?? 0);
    row.append(span, strong);
    metrics.appendChild(row);
  }

  el.conflictsPreview.append(header, reason, pair, metrics);

  const loadOrder = document.createElement("div");
  loadOrder.className = "conflict-load-order";
  const loadTitle = document.createElement("strong");
  loadTitle.textContent = t("loadOrder");
  const loadText = document.createElement("p");
  loadText.textContent = loadOrderExplanation(finding);
  loadOrder.append(loadTitle, loadText);

  if (finding.higherPriorityPath) {
    const winner = document.createElement("code");
    winner.textContent = `${t("likelyHigherPriority")}: ${finding.higherPriorityPath}`;
    loadOrder.appendChild(winner);
  }

  if (state.conflictsAnalysis?.resourceCfg?.path) {
    const cfg = document.createElement("code");
    cfg.textContent = `${t("resourceCfg")}: ${state.conflictsAnalysis.resourceCfg.path}`;
    loadOrder.appendChild(cfg);

    const cfgWarnings = state.conflictsAnalysis.resourceCfg.warnings || [];
    if (cfgWarnings.length) {
      const warningTitle = document.createElement("strong");
      warningTitle.textContent = t("resourceCfgWarnings");
      loadOrder.appendChild(warningTitle);

      for (const warning of cfgWarnings) {
        const warningLine = document.createElement("p");
        warningLine.textContent = warning;
        loadOrder.appendChild(warningLine);
      }
    }
  }

  el.conflictsPreview.appendChild(loadOrder);

  const chosen = document.createElement("p");
  chosen.className = "analysis-batch-summary";
  chosen.textContent = `${t("conflictSelectedSummary")}: ${state.conflictQuarantineSelected.size}. ${t("conflictSelectionHint")}`;
  el.conflictsPreview.appendChild(chosen);
  if (state.conflictQuarantinePlan) {
    const plan = document.createElement("div");
    plan.className = "quarantine-preview";
    const heading = document.createElement("h4");
    heading.textContent = t("quarantinePreviewOnly");
    const destination = document.createElement("code");
    destination.textContent = `${t("quarantineRoot")}: ${state.conflictQuarantinePlan.quarantineRoot}`;
    const stats = document.createElement("p");
    stats.textContent = `${t("quarantineReady")}: ${state.conflictQuarantinePlan.stats?.ready ?? 0} · ${t("blocked")}: ${state.conflictQuarantinePlan.stats?.blocked ?? 0}`;
    const manifest = document.createElement("pre");
    manifest.textContent = state.conflictQuarantinePlan.manifestPreview || "";
    plan.append(heading, destination, stats, manifest);
    if (state.conflictQuarantinePlan.canExecute) {
      const execute = document.createElement("button");
      execute.type = "button";
      execute.className = "primary-btn";
      execute.textContent = t("executeQuarantine");
      execute.disabled = state.quarantineBusy || workspaceReadOnly();
      execute.addEventListener("click", () => openConfirm("conflict_quarantine"));
      plan.appendChild(execute);
    }
    el.conflictsPreview.appendChild(plan);
  }

  const evidenceTitle = document.createElement("h4");
  evidenceTitle.textContent = t("evidence");
  el.conflictsPreview.appendChild(evidenceTitle);

  const evidenceList = document.createElement("div");
  evidenceList.className = "conflict-evidence-list";
  const identicalList = document.createElement("div");
  identicalList.className = "conflict-evidence-list";
  const differentHeading = document.createElement("strong");
  differentHeading.className = "conflict-evidence-heading";
  differentHeading.textContent = `${t("conflictDifferentResources")} · ${finding.differentPayloadCount || 0}`;
  el.conflictsPreview.appendChild(differentHeading);

  if (finding.evidenceTruncated) {
    const warning = document.createElement("p");
    warning.className = "conflict-evidence-note";
    warning.textContent = t("conflictEvidenceLimited");
    el.conflictsPreview.appendChild(warning);
  }

  for (const evidence of finding.evidence || []) {
    const card = document.createElement("article");
    card.className = `conflict-evidence ${evidence.samePayload ? "same" : "different"}`;

    const top = document.createElement("div");
    top.className = "conflict-evidence-top";
    const label = document.createElement("strong");
    label.textContent = evidence.resourceLabel;
    const impact = document.createElement("span");
    impact.textContent = conflictKindLabel(evidence.impactKind);
    top.append(label, impact);

    const tgi = document.createElement("code");
    tgi.textContent =
      `${evidence.resourceTypeHex} · ${evidence.groupHex} · ${evidence.instanceHex}`;

    const details = document.createElement("div");
    details.className = "conflict-evidence-details";
    for (const [metaLabel, value] of [
      [t("resourceClass"), evidence.resourceClass],
      [t("payloadA"), evidence.leftPayloadSha256],
      [t("payloadB"), evidence.rightPayloadSha256],
    ]) {
      const row = document.createElement("div");
      const span = document.createElement("span");
      span.textContent = metaLabel;
      const strong = document.createElement("strong");
      strong.textContent =
        typeof value === "string" && value.length > 24 ? `${value.slice(0, 24)}…` : value;
      row.append(span, strong);
      details.appendChild(row);
    }

    card.append(top, tgi, details);
    (evidence.samePayload ? identicalList : evidenceList).appendChild(card);
  }

  if (evidenceList.childElementCount) el.conflictsPreview.appendChild(evidenceList);
  if (identicalList.childElementCount) {
    const identicalSection = document.createElement("details");
    identicalSection.className = "conflict-identical-section";
    const summary = document.createElement("summary");
    summary.textContent = `${t("conflictIdenticalResources")} · ${finding.identicalPayloadCount || 0}`;
    identicalSection.append(summary, identicalList);
    el.conflictsPreview.appendChild(identicalSection);
  }
}

function createConflictFindingRow(finding) {
  const button = document.createElement("button");
  button.type = "button";
  button.title = `${conflictPriorityLabel(finding)} · ${t("conflictPriorityReason")}`;
  button.dataset.conflictId = finding.id;
  const sessionMark = effectiveConflictMark(finding);
  const isInactive = isInactiveConflictFinding(finding);
  button.className =
    "conflict-row" +
    (finding.id === state.conflictSelectedId ? " active" : "") +
    (sessionMark === "intentional" ? " intentional" : "") +
    (sessionMark === "ignored" ? " ignored" : "") +
    (sessionMark === "reviewed" ? " reviewed" : "");

  const main = document.createElement("div");
  main.className = "conflict-row-main";
  const kind = document.createElement("strong");
  kind.textContent = isInactive
    ? t("conflictInactiveOverlap")
    : sessionMark === "intentional"
      ? t("intentionalOverride")
      : sessionMark === "ignored"
        ? t("ignoredSession")
        : conflictKindLabel(finding.kind);
  const names = document.createElement("span");
  names.textContent = `${finding.left?.name || "—"} ↔ ${finding.right?.name || "—"}`;
  main.append(kind, names);

  const side = document.createElement("div");
  side.className = "conflict-row-side";
  const count = document.createElement("span");
  count.textContent = String(finding.differentPayloadCount ?? 0);
  const dot = document.createElement("span");
  dot.className = `conflict-severity conflict-severity-${finding.severity}`;
  count.title = t("differentPayloadCount");
  dot.title = isInactive ? t("conflictInactiveExplanation") : (finding.severity || "");
  side.append(count, dot);

  button.append(main, side);
  button.addEventListener("click", event => {
    if (event.shiftKey || event.ctrlKey || event.metaKey) {
      toggleConflictReviewFromRow(finding, event);
      return;
    }
    state.conflictSelectedId = finding.id;
    updateActiveVirtualRow(el.conflictsList, "conflictId", finding.id, "active");
    openConflictDetails();
  });

  const wrapper = document.createElement("div");
  wrapper.className = "conflict-row-shell" +
    (state.conflictReviewSelected.has(finding.id) ? " selected" : "");
  const checkbox = document.createElement("input");
  checkbox.type = "checkbox";
  checkbox.className = "conflict-row-select";
  checkbox.checked = state.conflictReviewSelected.has(finding.id);
  checkbox.disabled = state.reviewBusy || state.conflictsBusy || state.quarantineBusy;
  checkbox.setAttribute("aria-label", `${t("selectConflictFinding")}: ${finding.left?.name || "A"} / ${finding.right?.name || "B"}`);
  checkbox.addEventListener("click", event => {
    if (event.shiftKey && state.lastConflictReviewAnchor &&
        visibleConflictFindings().some(f => f.id === state.lastConflictReviewAnchor)) {
      event.preventDefault();
      selectConflictReviewRange(finding.id, !state.conflictReviewSelected.has(finding.id));
    }
  });
  checkbox.addEventListener("change", () => toggleConflictReviewSelection(finding.id, checkbox.checked, wrapper));
  wrapper.append(checkbox, button);
  return wrapper;
}

function renderConflictVirtualRows(findings, force = false) {
  el.conflictsList.dataset.fluentCount = String(findings.length);
  const view = virtualViews.conflicts;
  const changed = view.items !== findings;
  if (changed) {
    view.items = findings;
    view.start = -1;
    view.end = -1;
    el.conflictsList.scrollTop = 0;
  }

  if (!findings.length) {
    const empty = document.createElement("div");
    empty.className = "list-empty";
    empty.textContent = t("noConflictFindings");
    el.conflictsList.replaceChildren(empty);
    return;
  }

  const { start, end } = virtualRange(el.conflictsList, findings.length, ANALYSIS_ROW_STRIDE);
  if (!force && !changed && start === view.start && end === view.end) return;
  view.start = start;
  view.end = end;

  const fragment = document.createDocumentFragment();
  if (start > 0) fragment.appendChild(virtualSpacer(start * ANALYSIS_ROW_STRIDE));
  for (let index = start; index < end; index += 1) {
    const row = createConflictFindingRow(findings[index]);
    row.dataset.fluentIndex = String(index);
    // Keep the native button as the Fluent ListView keyboard target.
    // The checkbox is a sibling control and must not intercept navigation.
    const action = row.querySelector(".conflict-row");
    if (action) action.dataset.fluentIndex = String(index);
    fragment.appendChild(row);
  }
  if (end < findings.length) {
    fragment.appendChild(virtualSpacer((findings.length - end) * ANALYSIS_ROW_STRIDE));
  }
  el.conflictsList.replaceChildren(fragment);
}

function scheduleConflictVirtualRows() {
  const view = virtualViews.conflicts;
  if (view.raf) return;
  view.raf = window.requestAnimationFrame(() => {
    view.raf = 0;
    if (state.tab === "conflicts" && state.conflictsAnalysis) {
      renderConflictVirtualRows(visibleConflictFindings());
    }
  });
}

function renderConflictBatchControls() {
  if (!el.conflictsReviewSelectedBtn) return;
  const busy = state.quarantineBusy || state.analysisPipelineBusy ||
    state.scanning || state.conflictsBusy || state.duplicatesBusy ||
    state.auditBusy || state.reviewBusy || state.structureBusy;
  el.conflictsPrioritizeBtn.disabled = !state.conflictsAnalysis || busy;
  el.conflictsSelectAllBtn.disabled = !state.conflictsAnalysis || busy ||
    !visibleConflictFindings().length;
  el.conflictsSelectDetectedBtn.disabled = !state.conflictsAnalysis || busy ||
    !(state.conflictsAnalysis.findings || []).some(finding =>
      finding.kind !== "shared_identical" && finding.differentPayloadCount > 0);
  el.conflictsReviewSelectedBtn.disabled = busy || !state.conflictReviewSelected.size;
  el.conflictsClearReviewBtn.disabled = busy || !state.conflictReviewSelected.size;
  el.conflictsPrioritizeSelectedBtn.disabled = busy || !state.conflictReviewSelected.size;
  for (const button of [el.conflictsMarkIntentionalBtn, el.conflictsMarkReviewedBtn,
                        el.conflictsMarkIgnoredBtn, el.conflictsClearMarksBtn]) {
    button.disabled = busy || !state.conflictReviewSelected.size;
  }
  el.conflictsReviewSelectionCount.textContent = state.conflictsAnalysis
    ? `${t("conflictReviewSelectedCount")}: ${state.conflictReviewSelected.size}`
    : "";
  el.conflictsPreviewQuarantineBtn.disabled =
    !state.conflictsAnalysis || !state.conflictQuarantineSelected.size ||
    !conflictSelectionSafe() || busy || workspaceReadOnly();
  el.conflictsClearQuarantineBtn.disabled =
    !state.conflictQuarantineSelected.size || busy;
  renderConflictNavigation();
}

function renderConflicts() {
  if (!el.analyzeConflictsBtn) return;
  renderConflictBatchControls();
  el.analyzeConflictsBtn.querySelector("[data-i18n]")?.replaceChildren(
    document.createTextNode(t(state.conflictsAnalysis ? "reanalyzeConflicts" : "analyzeConflicts"))
  );
  el.analyzeConflictsBtn.disabled =
    !state.folder ||
    state.conflictsBusy ||
    state.scanning ||
    state.duplicatesBusy ||
    state.structureBusy ||
    state.toolsBusy;

  const analysis = state.conflictsAnalysis;
  const stats = analysis?.stats || {};
  // Resource overlap is not an actionable override when neither (or only
  // one) of the files is loaded under the current Resource.cfg rules.
  const needsReview = (analysis?.findings || []).filter(
    (finding) => finding.kind !== "shared_identical" && finding.severity !== "info"
  );
  el.confStatPairs.textContent = stats.packagePairs ?? 0;
  el.confStatReal.textContent = needsReview.length;
  el.confStatScript.textContent = needsReview.filter(
    (finding) => (finding.impactKinds || []).includes("script_conflict")
  ).length;
  el.confStatPotential.textContent = needsReview.filter(
    (finding) => ["potential_conflict", "cas_morph_overlap"].includes(finding.kind)
  ).length;
  el.confStatShared.textContent = stats.sharedIdentical ?? 0;

  if (state.conflictsBusy) {
    el.conflictsState.textContent = t("analyzingConflicts");
    el.conflictsState.className = "scan-state busy";
  } else if (state.conflictsError) {
    el.conflictsState.textContent = `${t("conflictsFailed")}: ${state.conflictsError}`;
    el.conflictsState.className = "scan-state error";
  } else if (state.conflictsNotice) {
    el.conflictsState.textContent = state.conflictsNotice;
    el.conflictsState.className = "scan-state";
  } else if (analysis) {
    const parts = [t("conflictsReady")];
    if (stats.unreadablePackages) parts.push(`${t("unreadablePackages")}: ${stats.unreadablePackages}`);
    if (stats.cacheHits) parts.push(`${t("cacheReused")}: ${stats.cacheHits}`);
    if (stats.cacheMisses) parts.push(`${t("cacheUpdated")}: ${stats.cacheMisses}`);
    if (stats.analysisTruncated) parts.push(t("conflictAnalysisTruncated"));
    if (stats.inactivePairs) parts.push(`${t("conflictInactiveCount")}: ${stats.inactivePairs}`);
    el.conflictsState.textContent = parts.join(" · ");
    el.conflictsState.className = "scan-state success";
  } else {
    el.conflictsState.textContent = "";
    el.conflictsState.className = "scan-state";
  }

  el.conflictsState.dataset.fluentSeverity =
    !state.conflictsBusy && !state.conflictsError && analysis &&
    (Number(stats.unreadablePackages || 0) > 0 || !!stats.analysisTruncated)
      ? "warning" : "";

  const hasAnalysis = !!analysis;
  el.conflictsEmpty.classList.toggle("hidden", hasAnalysis);
  el.conflictsResults.classList.toggle("hidden", !hasAnalysis);
  if (!hasAnalysis) {
    virtualViews.conflicts.items = null;
    renderConflictsPreview();
    return;
  }

  const findings = visibleConflictFindings();
  if (!findings.some((item) => item.id === state.conflictSelectedId)) {
    // After marking a pair intentional/ignored, it may disappear from the
    // review filter. Close its modal rather than silently jumping to a
    // different pair's details without user action.
    closeConflictDetails();
    state.conflictSelectedId = findings[0]?.id || "";
  }
  renderConflictVirtualRows(findings);
  renderConflictsPreview();
}


function operationBusy(kind) {
  return kind === "scan"
    ? state.scanning
    : kind === "duplicates"
      ? state.duplicatesBusy
      : state.conflictsBusy;
}

function operationUi(kind) {
  return {
    scan: [
      el.scanProgress,
      el.scanProgressBar,
      el.scanProgressText,
      el.scanProgressPercent,
      el.scanProgressCount,
      el.scanCancelBtn,
    ],
    duplicates: [
      el.duplicatesProgress,
      el.duplicatesProgressBar,
      el.duplicatesProgressText,
      el.duplicatesProgressPercent,
      el.duplicatesProgressCount,
      el.duplicatesCancelBtn,
    ],
    conflicts: [
      el.conflictsProgress,
      el.conflictsProgressBar,
      el.conflictsProgressText,
      el.conflictsProgressPercent,
      el.conflictsProgressCount,
      el.conflictsCancelBtn,
    ],
  }[kind];
}

function operationPhaseLabel(phase) {
  return {
    starting: t("progressStarting"),
    scanning: t("progressScanning"),
    fingerprinting: t("progressFingerprinting"),
    indexing: t("progressIndexing"),
    comparing: t("progressComparing"),
    cancelling: t("progressCancelling"),
  }[phase] || t("progressStarting");
}

function renderOperationProgress(kind) {
  const ui = operationUi(kind);
  if (!ui) return;
  const [container, bar, text, percentText, countText, cancelButton] = ui;
  const status = state.operations[kind];
  const visible = operationBusy(kind);
  container.classList.toggle("hidden", !visible);
  if (!visible) return;

  const total = Number(status?.total || 0);
  const processed = Math.min(Number(status?.processed || 0), total || Number.MAX_SAFE_INTEGER);
  const percent = total > 0
    ? Math.max(0, Math.min(100, (processed / total) * 100))
    : 0;
  const roundedPercent = Math.round(percent);

  const phaseLabel = operationPhaseLabel(status?.phase);
  const track = bar.parentElement;
  const determinate = total > 0;
  container.dataset.fluentProgress = determinate ? "determinate" : "indeterminate";
  bar.style.width = determinate ? `${percent}%` : "";
  percentText.textContent = determinate ? `${roundedPercent}%` : "—";
  countText.textContent = determinate
    ? `${integerLabel(processed)} / ${integerLabel(total)}`
    : "—";
  text.textContent = phaseLabel;
  // Native role + accessible value, without exposing file names or polling
  // a second UI model. Unknown totals are genuinely indeterminate.
  track.setAttribute("aria-valuemin", "0");
  track.setAttribute("aria-valuemax", "100");
  if (determinate) {
    track.setAttribute("aria-valuenow", String(roundedPercent));
    track.setAttribute(
      "aria-valuetext",
      `${roundedPercent}% — ${integerLabel(processed)} / ${integerLabel(total)}`
    );
  } else {
    track.removeAttribute("aria-valuenow");
    track.setAttribute("aria-valuetext", phaseLabel);
  }

  // Never show status.current here; it contains package file names.
  cancelButton.disabled = !!status?.cancelRequested;
  cancelButton.textContent = status?.cancelRequested ? t("cancelling") : t("cancelAnalysis");
}

function renderOrganizationProgress() {
  const container = el.organizeProgress;
  if (!container) return;
  const visible = state.executing;
  container.classList.toggle("hidden", !visible);
  if (!visible) return;

  const status = state.organizeProgress || {};
  const total = Number(status.total || 0);
  const processed = Math.max(0, Math.min(Number(status.processed || 0), total));
  const determinate = total > 0;
  const pct = determinate ? Math.round(processed * 100 / total) : 0;
  const labels = {
    planning: t("organizePlanning"),
    preparing: t("organizePreparing"),
    snapshotting: t("organizeSnapshotting"),
    moving: t("organizeMoving"),
    integrity_check: t("loadingPhaseIntegrity"),
    cleaning: t("organizeCleaning"),
    rolling_back: t("organizeRollingBack"),
    complete: t("organizeFinishing"),
  };
  const label = labels[status.phase] || t("organizePlanning");
  el.organizeProgressText.textContent = label;
  el.organizeProgress.dataset.fluentProgress = determinate ? "determinate" : "indeterminate";
  el.organizeProgressBar.style.width = determinate ? `${pct}%` : "";
  el.organizeProgressPercent.textContent = determinate ? `${pct}%` : "—";
  const elapsed = `${t("organizeElapsed")}: ${formatMs(status.elapsedMs || 0)}`;
  el.organizeProgressCount.textContent = determinate
    ? `${integerLabel(processed)} / ${integerLabel(total)} · ${elapsed}` : elapsed;
  const track = el.organizeProgressBar.parentElement;
  if (determinate) track.setAttribute("aria-valuenow", String(pct));
  else track.removeAttribute("aria-valuenow");
  track.setAttribute("aria-valuetext", determinate
    ? `${label} · ${pct}%` : label);
}

function loadingOverlayState() {
  if (state.executing) {
    const status = state.organizeProgress || {};
    const operationKey = `organize:${status.startedUnixMs || "active"}`;
    return { key: `${operationKey}:${status.phase || "planning"}`, operationKey,
      kind: "organize", status,
      phase: ({ moving: "loadingPhaseOrganization", integrity_check: "loadingPhaseIntegrity", cleaning: "loadingPhaseCleanup" }[status.phase] || "loadingPhaseOrganization"),
      blocking: true, cancelable: false, continueable: false };
  }
  if (state.restoreOperationActive) {
    const status = state.restoreOperationStatus || {};
    const operationKey = `restore:${status.startedUnixMs || "active"}`;
    return { key: `${operationKey}:${status.phase || "restoring"}`, operationKey,
      kind: "restore", status, phase: "loadingPhaseRestore",
      blocking: true, cancelable: false, continueable: false };
  }
  if (state.planning) {
    return { key: "planning", operationKey: "planning", kind: "planning", status: {},
      phase: "loadingPhasePlan", blocking: false, cancelable: false, continueable: true };
  }
  if (state.toolsBusy && state.toolsOperation) {
    const kind = state.toolsOperation;
    const status = state.toolsOperationStatus || {};
    const operationKey = `${kind}:${status.startedUnixMs || "active"}`;
    const blocking = ["inbox_import", "sims3pack_convert"].includes(kind);
    return { key: `${operationKey}:${status.phase || "working"}`, operationKey, kind, status,
      phase: kind === "health" ? "loadingPhaseAnalysis" : "loadingPhaseTools",
      blocking, cancelable: !blocking, continueable: !blocking };
  }
  if (state.externalOperation?.active) {
    const { kind, status = {} } = state.externalOperation;
    const operationKey = `${kind}:${status.startedUnixMs || "active"}`;
    const blocking = kind === "sims3pack_convert";
    return { key: `${operationKey}:${status.phase || "working"}`, operationKey, kind, status,
      phase: kind === "sims3pack_inspect" ? "loadingPhaseAnalysis" : "loadingPhaseTools",
      blocking, cancelable: !blocking, continueable: !blocking };
  }
  for (const kind of ["scan", "duplicates", "conflicts"]) {
    // The local owner is the source of truth for the lifecycle.  A terminal
    // poll from an older run must never reopen the overlay after that owner
    // has released the operation.
    if (!operationBusy(kind)) continue;
    const status = state.operations[kind] || {};
    const operationKey = `${kind}:${status.startedUnixMs || "active"}`;
    const scanPhase = kind === "scan"
      ? ({ starting: "loadingPhaseIdentification", scanning: "loadingPhaseAnalysis", fingerprinting: "loadingPhaseAnalysis", indexing: "loadingPhaseAnalysis", comparing: "loadingPhaseDuplicates" }[status.phase] || "loadingPhaseAnalysis")
      : "loadingPhaseDuplicates";
    return {
      key: `${operationKey}:${status.phase || "starting"}`,
      operationKey,
      kind,
      status,
      phase: scanPhase,
      blocking: false,
      cancelable: true,
      continueable: true,
    };
  }
  return null;
}

function renderLoadingOverlay() {
  if (!el.loadingOverlay) return;
  const current = loadingOverlayState();
  const dismissed = current && loadingOverlayDismissedKey === current.operationKey;
  el.loadingOverlay.classList.toggle("hidden", !current || dismissed);
  if (!current || dismissed) {
    el.loadingOverlay.setAttribute("aria-hidden", "true");
    if (!current) {
      loadingOverlayDismissedKey = "";
      loadingPhraseKey = "";
      loadingPhraseAt = 0;
    }
    return;
  }

  el.loadingOverlay.setAttribute("aria-hidden", "false");
  el.loadingOverlay.dataset.blocking = current.blocking ? "true" : "false";
  const status = current.status || {};
  const total = Number(status.total || 0);
  const processed = Math.max(0, Math.min(Number(status.processed || 0), total || Number.MAX_SAFE_INTEGER));
  const determinate = total > 0;
  const percent = determinate ? Math.round(processed * 100 / total) : 0;
  el.loadingOverlay.dataset.fluentProgress = determinate ? "determinate" : "indeterminate";
  el.loadingOverlayPhase.textContent = current.phase ? t(current.phase) : t("loadingTitle");
  el.loadingOverlayPercent.textContent = determinate ? `${percent}%` : "—";
  el.loadingOverlayBar.style.width = determinate ? `${percent}%` : "";
  el.loadingOverlayCount.textContent = determinate
    ? `${integerLabel(processed)} / ${integerLabel(total)}` : "—";
  const track = el.loadingOverlayBar.parentElement;
  if (determinate) {
    track.setAttribute("aria-valuenow", String(percent));
    track.setAttribute("aria-valuetext", `${percent}% — ${integerLabel(processed)} / ${integerLabel(total)}`);
  } else {
    track.removeAttribute("aria-valuenow");
    track.setAttribute("aria-valuetext", el.loadingOverlayPhase.textContent);
  }

  const now = Date.now();
  if (loadingPhraseKey !== current.key || now - loadingPhraseAt > 5200) {
    const phrases = [t("loadingPhrase1"), t("loadingPhrase2"), t("loadingPhrase3"), t("loadingPhrase4")];
    let next = (loadingPhraseIndex + 1) % phrases.length;
    if (phrases.length > 1 && next === loadingPhraseIndex) next = (next + 1) % phrases.length;
    loadingPhraseIndex = next;
    loadingPhraseAt = now;
    loadingPhraseKey = current.key;
    el.loadingOverlayPhrase.textContent = phrases[next];
  }

  const canCancel = current.cancelable && !status.cancelRequested;
  el.loadingOverlayCancel.classList.toggle("hidden", !current.cancelable);
  el.loadingOverlayCancel.disabled = !canCancel;
  el.loadingOverlayCancel.textContent = status.cancelRequested ? t("cancelling") : t("cancelAnalysis");
  if (el.loadingOverlayContinue) {
    const canContinue = current.continueable && !status.cancelRequested;
    el.loadingOverlayContinue.classList.toggle("hidden", !canContinue);
    el.loadingOverlayContinue.disabled = !canContinue;
    el.loadingOverlayContinue.textContent = t("continueInBackground");
  }
}

function continueLoadingInBackground() {
  const current = loadingOverlayState();
  if (!current?.continueable) return;
  loadingOverlayDismissedKey = current.operationKey;
  renderLoadingOverlay();
}

function beginOperationMonitor(kind) {
  const token = `${kind}:${++operationMonitorSequence}`;
  operationMonitorTokens.set(kind, token);
  return token;
}

function operationMonitorIsCurrent(kind, token) {
  return operationMonitorTokens.get(kind) === token;
}

function endOperationMonitor(kind, token) {
  if (operationMonitorIsCurrent(kind, token)) operationMonitorTokens.delete(kind);
}

async function settleOperationStatus(kind, token) {
  if (!operationMonitorIsCurrent(kind, token)) return;
  try {
    const status = await invoke("get_operation_status", { kind });
    if (operationMonitorIsCurrent(kind, token)) state.operations[kind] = status;
  } catch (_) {
    // The command result is authoritative; status polling is presentation only.
  }
}

async function monitorOrganizationProgress() {
  while (state.executing) {
    try {
      state.organizeProgress = await invoke("get_operation_status", { kind: "organize" });
      renderOrganizationProgress();
    } catch (_) {
      // The execution result is authoritative; polling must never fail moves.
    }
    if (!state.executing) break;
    await sleep(180);
  }
}

function renderAllOperationProgress() {
  renderOperationProgress("scan");
  renderOperationProgress("duplicates");
  renderOperationProgress("conflicts");
  renderOrganizationProgress();
  renderLoadingOverlay();
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function monitorOperation(kind, token) {
  while (operationMonitorIsCurrent(kind, token) && operationBusy(kind)) {
    try {
      const status = await invoke("get_operation_status", { kind });
      if (!operationMonitorIsCurrent(kind, token)) return;
      state.operations[kind] = status;
      renderOperationProgress(kind);
      renderLoadingOverlay();
    } catch (_) {
      // The primary operation result remains authoritative.
    }
    if (operationMonitorIsCurrent(kind, token)) await sleep(180);
  }

  if (!operationMonitorIsCurrent(kind, token)) return;
  await settleOperationStatus(kind, token);
  if (!operationMonitorIsCurrent(kind, token)) return;
  renderOperationProgress(kind);
  renderLoadingOverlay();
  renderDiagnostics();
}

function beginToolsOperation(kind) {
  const token = ++toolsOperationSequence;
  state.toolsOperation = kind;
  state.toolsOperationStatus = null;
  renderLoadingOverlay();
  void monitorToolsOperation(kind, token);
}

async function monitorToolsOperation(kind, token) {
  while (token === toolsOperationSequence && state.toolsBusy && state.toolsOperation === kind) {
    try {
      const status = await invoke("get_operation_status", { kind });
      if (token !== toolsOperationSequence || state.toolsOperation !== kind) return;
      state.toolsOperationStatus = status;
      renderTools();
      renderLoadingOverlay();
    } catch (_) {}
    if (token === toolsOperationSequence) await sleep(180);
  }
  if (token === toolsOperationSequence && state.toolsOperation === kind) {
    try { state.toolsOperationStatus = await invoke("get_operation_status", { kind }); } catch (_) {}
    if (token !== toolsOperationSequence || state.toolsOperation !== kind) return;
    renderTools();
    renderLoadingOverlay();
  }
}

async function cancelAnalysis(kind) {
  try {
    await invoke("cancel_operation", { kind });
    state.operations[kind] = await invoke("get_operation_status", { kind });
    renderOperationProgress(kind);
    renderLoadingOverlay();
  } catch (_) {}
}

async function cancelLoadingOperation(kind) {
  if (["scan", "duplicates", "conflicts"].includes(kind)) {
    return cancelAnalysis(kind);
  }
  if (kind === state.toolsOperation) {
    return cancelToolsOperation();
  }
  try {
    await invoke("cancel_operation", { kind });
    if (state.externalOperation?.kind === kind) {
      state.externalOperation.status = await invoke("get_operation_status", { kind });
    }
    renderLoadingOverlay();
  } catch (_) {}
}

async function monitorRestoreProgress(token) {
  while (token === restoreOperationSequence && state.restoreOperationActive) {
    try {
      const status = await invoke("get_operation_status", { kind: "restore" });
      if (token !== restoreOperationSequence) return;
      state.restoreOperationStatus = status;
      renderLoadingOverlay();
    } catch (_) {}
    if (token === restoreOperationSequence) await sleep(180);
  }
  if (token === restoreOperationSequence) {
    state.restoreOperationStatus = null;
    renderLoadingOverlay();
  }
}

async function cancelToolsOperation() {
  const kind = state.toolsOperation;
  if (!kind) return;
  try {
    await invoke("cancel_operation", { kind });
    state.toolsOperationStatus = await invoke("get_operation_status", { kind });
    renderTools();
  } catch (_) {}
}

function bytesLabel(value) {
  const bytes = Number(value || 0);
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function integerLabel(value) {
  const locale = state.language === "pt" ? "pt-BR" : state.language === "es" ? "es-ES" : "en-US";
  return new Intl.NumberFormat(locale).format(Number(value || 0));
}

function renderRestoreHistory() {
  if (!el.restoreHistoryList) return;
  el.restoreHistoryList.innerHTML = "";

  if (state.restoreHistoryLoading) {
    const line = document.createElement("div");
    line.className = "restore-history-empty";
    line.textContent = t("restorePreviewing");
    el.restoreHistoryList.appendChild(line);
    return;
  }

  if (!state.restoreHistory.length) {
    const line = document.createElement("div");
    line.className = "restore-history-empty";
    line.textContent = state.restoreHistoryError || t("noRestoreHistory");
    el.restoreHistoryList.appendChild(line);
    return;
  }

  for (const item of state.restoreHistory) {
    const button = document.createElement("button");
    button.type = "button";
    button.className =
      "restore-history-item" +
      (item.path === state.restoreManifest ? " active" : "") +
      (!item.valid || !item.matchesSelectedRoot ? " unavailable" : "");
    button.disabled = state.restoreBusy || state.executing;

    const main = document.createElement("div");
    const name = document.createElement("strong");
    name.textContent = item.fileName;
    const meta = document.createElement("span");
    meta.textContent = item.valid
      ? `${item.status} · ${item.files} ${t("manifestFiles")}`
      : t("invalidManifest");
    main.append(name, meta);

    const badge = document.createElement("span");
    badge.className = "restore-history-status";
    badge.textContent = item.matchesSelectedRoot ? item.status : t("differentRoot");

    button.append(main, badge);
    button.addEventListener("click", () => {
      if (state.restoreBusy || state.executing) return;
      // Selection is never a file operation. Preview and execution are separate.
      state.restoreManifest = item.path;
      state.restorePlan = null;
      state.restorePreviewManifest = "";
      state.restoreError = "";
      state.restoreNotice = "";
      render();
    });
    el.restoreHistoryList.appendChild(button);
  }
}

async function removeSelectedRestoreHistory() {
  const selectedManifest = state.pendingRemoveManifest;
  if (state.pendingAction !== "remove_restore_history" ||
      !selectedManifest || state.restoreBusy || state.executing ||
      el.confirmModal.classList.contains("hidden")) return;

  state.restoreBusy = true;
  state.restoreError = "";
  state.restoreNotice = "";
  el.confirmActionBtn.disabled = true;
  render();

  try {
    await invoke("remove_restore_history", {
      folder: state.folder,
      manifestPath: selectedManifest,
      confirmed: true,
    });
    if (state.restoreManifest === selectedManifest) {
      state.restoreManifest = "";
      state.restorePlan = null;
      state.restorePreviewManifest = "";
    }
    state.restoreNotice = t("restoreRecordRemoved");
  } catch (error) {
    state.restoreError = String(error);
  } finally {
    state.restoreBusy = false;
    el.confirmActionBtn.disabled = false;
    state.pendingAction = "";
    state.pendingRemoveManifest = "";
    el.confirmModal.classList.add("hidden");
    el.confirmModal.setAttribute("aria-hidden", "true");
    await Promise.all([loadRestoreHistory(), refreshOperationHistory()]);
    render();
  }
}

async function loadRestoreHistory() {
  if (!state.folder) {
    state.restoreHistory = [];
    renderRestoreHistory();
    return;
  }
  state.restoreHistoryLoading = true;
  state.restoreHistoryError = "";
  renderRestoreHistory();
  try {
    state.restoreHistory = await invoke("list_restore_history", { folder: state.folder });
  } catch (error) {
    state.restoreHistory = [];
    state.restoreHistoryError = String(error);
  } finally {
    state.restoreHistoryLoading = false;
    renderRestoreHistory();
  }
}

async function loadTechnicalDetails(item) {
  if (!item || !state.folder || state.technicalDetailsLoading) return;
  state.technicalDetailsOpen.add(item.path);
  state.technicalDetailsLoading = item.path;
  delete state.technicalDetailsErrors[item.path];
  renderPreview();

  try {
    state.technicalDetails[item.path] = await invoke("get_package_technical_details", {
      folder: state.folder,
      packagePath: item.path,
    });
  } catch (error) {
    state.technicalDetailsErrors[item.path] = String(error);
  } finally {
    state.technicalDetailsLoading = "";
    renderPreview();
  }
}

function renderTechnicalDetails(container, item) {
  const isOpen = state.technicalDetailsOpen.has(item.path);
  const button = document.createElement("button");
  button.type = "button";
  button.className = "secondary-btn technical-details-btn";
  button.textContent = isOpen ? t("hideTechnicalDetails") : t("technicalDetails");
  button.disabled =
    state.technicalDetailsLoading === item.path ||
    state.scanning ||
    state.duplicatesBusy ||
    state.conflictsBusy;
  button.addEventListener("click", () => {
    if (state.technicalDetailsOpen.has(item.path)) {
      state.technicalDetailsOpen.delete(item.path);
      renderPreview();
    } else if (state.technicalDetails[item.path]) {
      state.technicalDetailsOpen.add(item.path);
      renderPreview();
    } else {
      loadTechnicalDetails(item);
    }
  });
  container.appendChild(button);

  if (!isOpen && state.technicalDetailsLoading !== item.path) return;

  if (state.technicalDetailsLoading === item.path) {
    const loading = document.createElement("div");
    loading.className = "technical-details-state";
    loading.textContent = t("technicalDetailsLoading");
    container.appendChild(loading);
    return;
  }

  if (state.technicalDetailsErrors[item.path] && !state.technicalDetails[item.path]) {
    const error = document.createElement("div");
    error.className = "technical-details-state error";
    error.textContent = `${t("technicalDetailsError")}: ${state.technicalDetailsErrors[item.path]}`;
    container.appendChild(error);
  }

  const details = state.technicalDetails[item.path];
  if (!details) return;

  if (details.parseError) {
    const parseError = document.createElement("div");
    parseError.className = "technical-details-state error";
    parseError.textContent = details.parseError;
    container.appendChild(parseError);
  }

  const summary = document.createElement("div");
  summary.className = "technical-summary";
  appendMeta(summary, t("fileSha256"), details.fileSha256);
  appendMeta(summary, t("dbpfVersion"), details.dbpfMajor == null ? "—" : `${details.dbpfMajor}.${details.dbpfMinor ?? 0}`);
  appendMeta(summary, t("resources"), details.resourceCount);
  appendMeta(summary, t("cacheReused"), details.cacheHit ? "✓" : "—");
  container.appendChild(summary);

  const mesh = details.mesh;
  const meshSection = document.createElement("section");
  meshSection.className = "mesh-information";
  const meshTitle = document.createElement("h4");
  meshTitle.className = "technical-details-title";
  meshTitle.textContent = t("meshInformation");
  meshSection.appendChild(meshTitle);

  if (!mesh?.hasMeshes) {
    const emptyMesh = document.createElement("div");
    emptyMesh.className = "technical-details-state";
    emptyMesh.textContent = t("meshNoData");
    meshSection.appendChild(emptyMesh);
  } else {
    const meshSummary = document.createElement("div");
    meshSummary.className = "technical-summary";
    appendMeta(
      meshSummary,
      t("meshHighestPolycount"),
      mesh.highestVisibleTriangles == null ? "—" : integerLabel(mesh.highestVisibleTriangles)
    );
    appendMeta(
      meshSummary,
      t("meshHighestVertices"),
      mesh.highestVisibleVertices == null ? "—" : integerLabel(mesh.highestVisibleVertices)
    );
    meshSection.appendChild(meshSummary);

    if ((mesh.lods || []).length) {
      const lodTitle = document.createElement("h5");
      lodTitle.className = "mesh-subtitle";
      lodTitle.textContent = t("meshLods");
      meshSection.appendChild(lodTitle);

      const lodList = document.createElement("div");
      lodList.className = "mesh-lod-list";
      for (const lod of mesh.lods) {
        const row = document.createElement("article");
        row.className = "mesh-lod-row";
        const name = document.createElement("strong");
        name.textContent = lod.lod;
        const counts = document.createElement("span");
        counts.textContent =
          `${t("meshTriangles")}: ${integerLabel(lod.triangles)} · ${t("meshVertices")}: ${integerLabel(lod.vertices)} · ${lod.resourceCount} ${t("resources")}`;
        row.append(name, counts);
        lodList.appendChild(row);
      }
      meshSection.appendChild(lodList);
    }

    const resourceTitle = document.createElement("h5");
    resourceTitle.className = "mesh-subtitle";
    resourceTitle.textContent = t("meshResources");
    meshSection.appendChild(resourceTitle);

    const resourceList = document.createElement("div");
    resourceList.className = "mesh-resource-list";
    for (const resource of mesh.resources || []) {
      const row = document.createElement("article");
      row.className = "mesh-resource-row";
      const top = document.createElement("div");
      const label = document.createElement("strong");
      label.textContent = `${resource.typeLabel} · ${resource.lod}`;
      const shadow = document.createElement("span");
      shadow.textContent = resource.shadow ? t("meshShadow") : resource.groupHex;
      top.append(label, shadow);

      const counts = document.createElement("small");
      counts.textContent =
        `${t("meshTriangles")}: ${resource.triangles == null ? "—" : integerLabel(resource.triangles)} · ${t("meshVertices")}: ${integerLabel(resource.vertices)} · ${t("meshGroups")}: ${resource.groups?.length || 0}`;

      const internal = document.createElement("code");
      internal.textContent = resource.internalName || resource.tgi;
      row.append(top, counts, internal);

      for (const warning of resource.warnings || []) {
        const note = document.createElement("small");
        note.className = "mesh-note";
        note.textContent = warning;
        row.appendChild(note);
      }
      resourceList.appendChild(row);
    }
    meshSection.appendChild(resourceList);

    if ((mesh.warnings || []).length) {
      const warningBox = document.createElement("div");
      warningBox.className = "warning-box mesh-warning-box";
      const warningTitle = document.createElement("strong");
      warningTitle.textContent = t("meshWarnings");
      const list = document.createElement("ul");
      for (const warning of mesh.warnings) {
        const li = document.createElement("li");
        li.textContent = warning;
        list.appendChild(li);
      }
      warningBox.append(warningTitle, list);
      meshSection.appendChild(warningBox);
    }
  }
  container.appendChild(meshSection);

  const title = document.createElement("h4");
  title.className = "technical-details-title";
  title.textContent = t("resourceTechnicalDetails");
  container.appendChild(title);

  const list = document.createElement("div");
  list.className = "technical-resource-list";
  const visibleResources = (details.resources || []).slice(0, 500);
  for (const resource of visibleResources) {
    const row = document.createElement("article");
    row.className = "technical-resource";
    const top = document.createElement("div");
    const label = document.createElement("strong");
    label.textContent = resource.typeLabel;
    const compression = document.createElement("span");
    compression.textContent = resource.compression;
    top.append(label, compression);

    const tgi = document.createElement("code");
    tgi.textContent = resource.tgi;

    const meta = document.createElement("small");
    meta.textContent =
      `${t("diskSize")}: ${bytesLabel(resource.diskSize)} · ${t("memorySize")}: ${bytesLabel(resource.memorySize)}`;

    const hash = document.createElement("code");
    hash.textContent = `${t("payloadHash")}: ${resource.payloadSha256}`;

    row.append(top, tgi, meta, hash);
    list.appendChild(row);
  }
  container.appendChild(list);

  if ((details.resources || []).length > visibleResources.length) {
    const limited = document.createElement("div");
    limited.className = "technical-details-state";
    limited.textContent =
      `${t("showingResourceLimit")}: ${visibleResources.length}/${details.resources.length}`;
    container.appendChild(limited);
  }
}

function structureLocked() {
  return (
    state.structureBusy ||
    state.scanning ||
    state.duplicatesBusy ||
    state.conflictsBusy ||
    state.planning ||
    state.executing ||
    state.restoreBusy ||
    state.auditBusy ||
    state.reviewBusy ||
    state.toolsBusy ||
    !!state.technicalDetailsLoading
  );
}

function selectedStructureEntry() {
  return (state.structureListing?.entries || []).find(
    (entry) => entry.relativePath === state.structureSelectedPath
  ) || null;
}

async function refreshManualOperations() {
  if (!state.folder) {
    state.manualOperations = [];
    return;
  }
  try {
    state.manualOperations = await invoke("list_manual_operations", {
      folder: state.folder,
    });
  } catch (_) {
    state.manualOperations = [];
  }
}

function invalidateAnalysesAfterStructureChange() {
  state.items = [];
  state.stats = null;
  state.selectedId = "";
  state.selectedForPlan.clear();
  state.plan = null;
  state.planError = "";
  state.error = "";
  state.duplicatesAnalysis = null;
  state.duplicatesError = "";
  state.duplicatesNotice = "";
  state.duplicateSelectedId = "";
  state.conflictsAnalysis = null;
  state.conflictsError = "";
  state.conflictsNotice = "";
  state.conflictSelectedId = "";
  state.conflictMarks = {};
  state.technicalDetails = {};
  state.technicalDetailsErrors = {};
  state.technicalDetailsOpen.clear();
  state.packagePreviews = {};
  state.packagePreviewLoading = {};
  state.packagePreviewErrors = {};
  state.restorePlan = null;
  state.quarantineSelected.clear();
  state.quarantinePlan = null;
  state.quarantineExactBatch = false;
  state.conflictQuarantineSelected.clear();
  state.conflictQuarantinePlan = null;
  state.auditReports = { organizer: null, duplicates: null, conflicts: null };
  state.auditErrors = { organizer: "", duplicates: "", conflicts: "" };
}

async function loadStructure(relativePath = state.structureCurrent) {
  if (
    !state.folder ||
    state.structureBusy ||
    state.executing ||
    state.restoreBusy
  ) return;
  state.structureBusy = true;
  state.structureError = "";
  renderStructure();

  try {
    state.structureListing = await invoke("list_structure", {
      folder: state.folder,
      relativePath: relativePath || "",
    });
    state.structureCurrent = state.structureListing?.currentRelativePath || "";
    state.structureSelectedPath = "";
  } catch (error) {
    state.structureError = String(error);
    state.structureListing = null;
    if (relativePath) state.structureCurrent = "";
  } finally {
    state.structureBusy = false;
    // The background Structure refresh also affects shared sidebar controls.
    // Restore their disabled state without requiring a navigation click.
    render();
  }
}

async function loadStructureDirectories() {
  if (!state.folder) {
    state.structureDirectories = [];
    return;
  }
  state.structureDirectories = await invoke("list_structure_directories", {
    folder: state.folder,
  });
}

function renderStructurePreview() {
  if (!el.structurePreview) return;
  const entry = selectedStructureEntry();
  el.structurePreview.innerHTML = "";

  if (!entry) {
    const empty = document.createElement("div");
    empty.className = "preview-empty";
    empty.textContent = t("selectStructureItem");
    el.structurePreview.appendChild(empty);
    return;
  }

  const header = document.createElement("div");
  header.className = "preview-header";
  const name = document.createElement("h3");
  name.textContent = entry.name;
  const badge = document.createElement("span");
  badge.className = "status-badge status-classified";
  badge.textContent = entry.isDirectory ? t("folderType") : t("fileType");
  header.append(name, badge);

  const meta = document.createElement("div");
  meta.className = "preview-meta";
  appendMeta(meta, t("itemType"), entry.isDirectory ? t("folderType") : t("fileType"));
  appendMeta(meta, t("structurePath"), entry.relativePath);
  if (!entry.isDirectory) appendMeta(meta, t("structureSize"), bytesLabel(entry.size));

  const actions = document.createElement("div");
  actions.className = "preview-actions";
  const openButton = document.createElement("button");
  openButton.type = "button";
  openButton.className = "secondary-btn";
  openButton.textContent = t("openStructureLocation");
  openButton.addEventListener("click", () => revealSafe(entry.path));
  actions.appendChild(openButton);

  if (entry.isDirectory) {
    const enterButton = document.createElement("button");
    enterButton.type = "button";
    enterButton.className = "secondary-btn";
    enterButton.textContent = t("currentFolder");
    enterButton.addEventListener("click", () => loadStructure(entry.relativePath));
    actions.appendChild(enterButton);
  }

  el.structurePreview.append(header, meta, actions);
}

function renderStructure() {
  if (!el.structureState) return;

  const hasRoot = !!state.folder;
  const listing = state.structureListing;
  el.structureEmpty.classList.toggle("hidden", hasRoot);
  el.structureResults.classList.toggle("hidden", !hasRoot || !listing);

  el.structureCurrentPath.textContent =
    listing?.currentRelativePath ? `\\${listing.currentRelativePath}` : "\\";
  el.structureCurrentPath.title = el.structureCurrentPath.textContent;

  const locked = structureLocked();
  el.structureUpBtn.disabled =
    !hasRoot || locked || !listing?.parentRelativePath;
  el.structureRefreshBtn.disabled = !hasRoot || locked;
  const readOnly = workspaceReadOnly();
  el.structureCreateBtn.disabled = !hasRoot || locked || readOnly;

  const selected = selectedStructureEntry();
  el.structureMoveBtn.disabled = !selected || locked || readOnly;
  el.structureRenameBtn.disabled =
    !selected || !selected.isDirectory || locked || readOnly;
  el.structureUndoBtn.disabled =
    !hasRoot || locked || readOnly || !state.manualOperations.length;

  if (state.structureBusy) {
    el.structureState.textContent = t("scanning");
    el.structureState.className = "scan-state busy";
  } else if (state.structureError) {
    el.structureState.textContent = `${t("structureFailed")}: ${state.structureError}`;
    el.structureState.className = "scan-state error";
  } else if (state.structureNotice) {
    el.structureState.textContent = state.structureNotice;
    el.structureState.className = "scan-state success";
  } else {
    el.structureState.textContent = "";
    el.structureState.className = "scan-state";
  }

  if (!listing) {
    renderStructurePreview();
    return;
  }

  el.structureList.innerHTML = "";
  if (!(listing.entries || []).length) {
    const empty = document.createElement("div");
    empty.className = "list-empty";
    empty.textContent = t("structureNoItems");
    el.structureList.appendChild(empty);
  } else {
    for (const entry of listing.entries || []) {
      const row = document.createElement("button");
      row.type = "button";
      row.className =
        "structure-row" +
        (entry.relativePath === state.structureSelectedPath ? " active" : "");

      const icon = document.createElement("i");
      icon.className = entry.isDirectory
        ? "fa-solid fa-folder"
        : "fa-solid fa-file";

      const main = document.createElement("div");
      main.className = "structure-row-main";
      const name = document.createElement("strong");
      name.textContent = entry.name;
      const meta = document.createElement("span");
      meta.textContent = entry.isDirectory
        ? t("folderType")
        : `${t("fileType")} · ${bytesLabel(entry.size)}`;
      main.append(name, meta);

      row.append(icon, main);
      row.addEventListener("click", () => {
        state.structureSelectedPath = entry.relativePath;
        renderStructure();
      });
      row.addEventListener("dblclick", () => {
        if (entry.isDirectory) loadStructure(entry.relativePath);
      });
      el.structureList.appendChild(row);
    }
  }

  renderStructurePreview();
}

function closeStructureModal() {
  state.structureModalAction = "";
  el.structureModal.classList.add("hidden");
  el.structureModal.setAttribute("aria-hidden", "true");
  el.structureInput.value = "";
}

async function openStructureModal(action) {
  const selected = selectedStructureEntry();
  if (!state.folder || structureLocked() || workspaceReadOnly()) return;
  if ((action === "move" || action === "rename") && !selected) return;
  if (action === "rename" && !selected?.isDirectory) return;

  state.structureModalAction = action;
  el.structureModalActionBtn.disabled = false;
  el.structureInputWrap.classList.toggle("hidden", action === "move");
  el.structureTargetWrap.classList.toggle("hidden", action !== "move");

  if (action === "create") {
    el.structureModalTitle.textContent = t("createFolderTitle");
    el.structureModalMessage.textContent = `${t("createFolderMessage")} ${t("newFolderPathHint")}`;
    el.structureInputLabel.textContent = t("newFolderPath");
    el.structureInput.value = "";
    el.structureModalActionBtn.textContent = t("createFolder");
  } else if (action === "rename") {
    el.structureModalTitle.textContent = t("renameFolderTitle");
    el.structureModalMessage.textContent = t("renameFolderMessage");
    el.structureInputLabel.textContent = t("newFolderName");
    el.structureInput.value = selected?.name || "";
    el.structureModalActionBtn.textContent = t("renameFolder");
  } else {
    el.structureModalTitle.textContent = t("moveTitle");
    el.structureModalMessage.textContent = t("moveMessage");
    el.structureModalActionBtn.textContent = t("moveSelected");

    try {
      await loadStructureDirectories();
    } catch (error) {
      state.structureError = String(error);
      renderStructure();
      return;
    }

    const source = selected?.relativePath || "";
    const sourcePrefix = source ? `${source}\\` : "";
    const sourceParent =
      source.includes("\\") ? source.slice(0, source.lastIndexOf("\\")) : "";

    el.structureTargetSelect.innerHTML = "";
    for (const directory of state.structureDirectories) {
      if (
        selected?.isDirectory &&
        (directory.relativePath === source ||
          (sourcePrefix && directory.relativePath.startsWith(sourcePrefix)))
      ) {
        continue;
      }
      if (directory.relativePath === sourceParent) continue;

      const option = document.createElement("option");
      option.value = directory.relativePath;
      option.textContent =
        directory.relativePath
          ? `${"  ".repeat(directory.depth)}${directory.relativePath}`
          : "\\";
      el.structureTargetSelect.appendChild(option);
    }

    if (!el.structureTargetSelect.options.length) {
      el.structureModalActionBtn.disabled = true;
    }
  }

  el.structureModal.classList.remove("hidden");
  el.structureModal.setAttribute("aria-hidden", "false");
  if (action !== "move") {
    requestAnimationFrame(() => el.structureInput.focus());
  }
}

async function executeStructureAction() {
  const action = state.structureModalAction;
  const selected = selectedStructureEntry();
  if (!action || structureLocked()) return;

  state.structureBusy = true;
  state.structureError = "";
  el.structureModalActionBtn.disabled = true;

  try {
    let result;
    if (action === "create") {
      result = await invoke("create_structure_folder", {
        folder: state.folder,
        parentRelativePath: state.structureCurrent || "",
        nestedPath: el.structureInput.value,
      });
    } else if (action === "rename") {
      result = await invoke("rename_structure_folder", {
        folder: state.folder,
        sourceRelativePath: selected?.relativePath || "",
        newName: el.structureInput.value,
      });
    } else if (action === "move") {
      result = await invoke("move_structure_path", {
        folder: state.folder,
        sourceRelativePath: selected?.relativePath || "",
        targetParentRelativePath: el.structureTargetSelect.value,
      });
    }

    state.structureNotice =
      `${t("structureComplete")}: ${result?.destinationRelativePath || ""}`;
    closeStructureModal();
    if (action === "create") {
      state.plan = null;
      state.planError = "";
      state.auditReports = { organizer: null, duplicates: null, conflicts: null };
    } else {
      invalidateAnalysesAfterStructureChange();
    }
    await Promise.all([
      refreshManualOperations(),
      refreshCacheInfo(),
      refreshConflictDecisions(),
    ]);
    state.structureBusy = false;
    await loadStructure(state.structureCurrent);
  } catch (error) {
    state.structureError = String(error);
    el.structureModalMessage.textContent =
      `${t("structureFailed")}: ${state.structureError}`;
  } finally {
    state.structureBusy = false;
    el.structureModalActionBtn.disabled = false;
    render();
  }
}

function toolsOperationLocked() {
  return (
    state.toolsBusy ||
    state.scanning ||
    state.duplicatesBusy ||
    state.conflictsBusy ||
    state.structureBusy ||
    state.toolsBusy ||
    state.planning ||
    state.executing ||
    state.restoreBusy ||
    state.quarantineBusy ||
    !!state.technicalDetailsLoading
  );
}

function workspaceReadOnly() {
  return !!state.workspaceStore?.readOnly;
}

function activeWorkspaceProfile() {
  const store = state.workspaceStore;
  if (!store) return null;
  return (store.profiles || []).find((profile) => profile.id === store.activeProfileId)
    || (store.profiles || [])[0]
    || null;
}

async function loadWorkspaceTools() {
  if (!state.folder) {
    state.workspaceStore = null;
    renderTools();
    return;
  }
  try {
    state.workspaceStore = await invoke("load_workspace", { folder: state.folder });
  } catch (error) {
    state.toolsError = String(error);
  }
  renderTools();
}

async function persistWorkspaceStore() {
  if (!state.folder || !state.workspaceStore) return;
  state.toolsBusy = true;
  state.toolsError = "";
  try {
    state.workspaceStore = await invoke("save_workspace", {
      folder: state.folder,
      store: state.workspaceStore,
    });
    state.plan = null;
    state.toolsNotice = t("profileSaved");
  } catch (error) {
    state.toolsError = String(error);
  } finally {
    state.toolsBusy = false;
    renderTools();
    render();
  }
}

function toolListItem(title, detail = "", extraClass = "") {
  const row = document.createElement("div");
  row.className = `tools-list-item ${extraClass}`.trim();
  const strong = document.createElement("strong");
  strong.textContent = title;
  row.appendChild(strong);
  if (detail) {
    const small = document.createElement("small");
    small.textContent = detail;
    row.appendChild(small);
  }
  return row;
}

function renderProfileTools() {
  if (!el.toolsProfileSelect) return;
  const store = state.workspaceStore;
  const disabled = !state.folder || state.toolsBusy;
  el.toolsReadOnly.disabled = !state.folder || state.toolsBusy;
  el.toolsReadOnly.checked = !!store?.readOnly;

  el.toolsProfileSelect.innerHTML = "";
  for (const profile of store?.profiles || []) {
    const option = document.createElement("option");
    option.value = profile.id;
    option.textContent = profile.name;
    option.selected = profile.id === store.activeProfileId;
    el.toolsProfileSelect.appendChild(option);
  }

  const profile = activeWorkspaceProfile();
  el.toolsProfilePrefix.value = profile?.destinationPrefix || "";
  el.toolsProfileCollapse.checked = !!profile?.collapseToCategory;
  el.toolsProfilePrefix.disabled = disabled || !profile;
  el.toolsProfileCollapse.disabled = disabled || !profile;
  el.toolsSaveProfile.disabled = disabled || !profile;
  el.toolsDeleteProfile.disabled = disabled || !profile || profile.id === "default";
  el.toolsAddProfile.disabled = disabled;

  el.toolsProtectedList.innerHTML = "";
  for (const folder of profile?.protectedFolders || []) {
    const row = toolListItem(folder, t("protectedFolders"));
    const button = document.createElement("button");
    button.type = "button";
    button.className = "secondary-btn compact-btn";
    button.textContent = "×";
    button.addEventListener("click", async () => {
      profile.protectedFolders = profile.protectedFolders.filter((value) => value !== folder);
      await persistWorkspaceStore();
    });
    row.appendChild(button);
    el.toolsProtectedList.appendChild(row);
  }

  el.toolsRulesList.innerHTML = "";
  for (const rule of profile?.rules || []) {
    const criteria = [
      rule.category && `category=${rule.category}`,
      rule.subCategory && `sub=${rule.subCategory}`,
      rule.detectedFrom && `resource=${rule.detectedFrom}`,
      rule.nameContains && `name~${rule.nameContains}`,
      rule.pathContains && `path~${rule.pathContains}`,
    ].filter(Boolean).join(" · ");
    const row = toolListItem(rule.name || rule.id, `${criteria || "*"} → ${rule.destination}`);
    const enabled = document.createElement("input");
    enabled.type = "checkbox";
    enabled.checked = rule.enabled !== false;
    enabled.title = rule.enabled !== false ? "Enabled" : "Disabled";
    enabled.addEventListener("change", async () => {
      rule.enabled = enabled.checked;
      await persistWorkspaceStore();
    });
    row.appendChild(enabled);
    const remove = document.createElement("button");
    remove.type = "button";
    remove.className = "secondary-btn compact-btn";
    remove.textContent = "×";
    remove.addEventListener("click", async () => {
      profile.rules = profile.rules.filter((item) => item.id !== rule.id);
      await persistWorkspaceStore();
    });
    row.appendChild(remove);
    el.toolsRulesList.appendChild(row);
  }
}

function visibleHealthFindings() {
  const findings = state.healthReport?.findings || [];
  return findings.filter((item) =>
    (state.healthFilter === "all" || item.category === state.healthFilter) &&
    (state.healthSeverity === "all" || item.severity === state.healthSeverity)
  );
}

function renderHealthTools() {
  const report = state.healthReport;
  el.toolsHealthSummary.innerHTML = "";
  if (report) {
    const stats = report.stats || {};
    for (const [label, value] of [
      [t("packages"), stats.packages],
      [t("readablePackages"), stats.readable],
      [t("invalid"), stats.unreadable],
      [t("needsReview"), state.stats?.needsReview ?? 0],
      [t("emptyFolders"), stats.emptyFolders],
      [t("uncoveredPackages"), stats.resourceCfgUncovered],
      [t("outsidePackages"), stats.packagesOutsideRoot],
      [t("exactGroups"), state.duplicatesAnalysis?.stats?.exactGroups ?? 0],
      [t("packagePairs"), state.conflictsAnalysis?.stats?.packagePairs ?? 0],
      [t("cacheReused"), (() => {
        const source = stats.cacheHits != null
          ? stats
          : state.conflictsAnalysis?.stats || state.duplicatesAnalysis?.stats;
        const hits = source?.cacheHits ?? 0;
        const misses = source?.cacheMisses ?? 0;
        return hits + misses > 0 ? `${Math.round((hits / (hits + misses)) * 100)}%` : "—";
      })()],
    ]) {
      const card = document.createElement("article");
      const b = document.createElement("b"); b.textContent = value ?? 0;
      const span = document.createElement("span"); span.textContent = label;
      card.append(b, span); el.toolsHealthSummary.appendChild(card);
    }
  }

  el.toolsResourcecfg.innerHTML = "";
  if (report?.resourceCfg) {
    const header = toolListItem(report.resourceCfg.path,
      `${report.resourceCfg.rules?.length || 0} rules · ${report.resourceCfg.precedenceReliable ? "priority reliable" : "advanced directives"}`);
    el.toolsResourcecfg.appendChild(header);
    for (const rule of report.resourceCfg.rules || []) {
      el.toolsResourcecfg.appendChild(toolListItem(
        `Priority ${rule.priority}`,
        `PackedFile ${rule.pattern} · line ${rule.sourceLine}`
      ));
    }
    const coverage = [...(report.coverage || [])].sort((a,b) =>
      (b.priority ?? -999999) - (a.priority ?? -999999) ||
      a.relativePath.localeCompare(b.relativePath)
    );
    for (const item of coverage.slice(0, 500)) {
      el.toolsResourcecfg.appendChild(toolListItem(
        item.relativePath,
        !item.coverageKnown
          ? t("coverageUnknown")
          : item.covered
            ? `${t("loadOrderViewer")}: ${item.priority ?? "—"} · ${item.rule || "—"}`
            : t("uncoveredPackages"),
        !item.coverageKnown ? "tools-health-unknown" : item.covered ? "" : "tools-health-bad"
      ));
    }
  } else if (report?.resourceCfgError) {
    el.toolsResourcecfg.appendChild(toolListItem("Resource.cfg", report.resourceCfgError, "tools-health-bad"));
  } else if (report) {
    el.toolsResourcecfg.appendChild(toolListItem("Resource.cfg", t("missingResourceCfg"), "tools-health-bad"));
  }


  const findings = report?.findings || [];
  const known = new Set(findings.map((item) => item.id));
  for (const id of state.healthSelected) if (!known.has(id)) state.healthSelected.delete(id);
  const categories = [...new Set(findings.map((item) => item.category))].sort();
  const categoryOptions = ["all", ...categories];
  if (!categoryOptions.includes(state.healthFilter)) state.healthFilter = "all";
  el.toolsHealthFilter.replaceChildren(...categoryOptions.map((category) => {
    const option = document.createElement("option");
    option.value = category;
    option.textContent = category === "all" ? t("healthAll") : category.replaceAll("_", " ");
    return option;
  }));
  el.toolsHealthFilter.value = state.healthFilter;
  el.toolsHealthSeverity.value = state.healthSeverity;
  const visible = visibleHealthFindings();
  el.toolsHealthFindings.replaceChildren();
  for (const item of visible) {
    const row = document.createElement("label");
    row.className = "tools-list-item health-finding" + (state.healthSelected.has(item.id) ? " selected" : "");
    const selection = document.createElement("input");
    selection.type = "checkbox";
    selection.checked = state.healthSelected.has(item.id);
    selection.setAttribute("aria-label", item.category + ": " + item.relativePath);
    selection.addEventListener("change", () => {
      if (selection.checked) state.healthSelected.add(item.id);
      else state.healthSelected.delete(item.id);
      renderHealthTools();
    });
    const content = document.createElement("span");
    const title = document.createElement("strong");
    title.textContent = item.category.replaceAll("_", " ") + " · " + item.severity;
    const path = document.createElement("code");
    path.textContent = item.relativePath;
    const detail = document.createElement("small");
    detail.textContent = item.detail;
    content.append(title, path, detail);
    row.append(selection, content);
    el.toolsHealthFindings.append(row);
  }
  if (!visible.length) el.toolsHealthFindings.append(toolListItem(t("healthNoFindings"), ""));
  el.toolsHealthSelectionCount.textContent = state.healthSelected.size + " " + t("healthSelectedCount");
  el.toolsHealthSelectVisible.disabled = !visible.length || state.toolsBusy;
  el.toolsHealthClearSelection.disabled = !state.healthSelected.size || state.toolsBusy;
  el.toolsHealthExport.disabled = !state.healthSelected.size || state.toolsBusy || !state.folder;
}

async function exportSelectedHealth() {
  if (!state.healthReport || !state.healthSelected.size || state.toolsBusy || !state.folder) return;
  const sourceRoot = state.folder;
  const selected = state.healthReport.findings.filter((item) => state.healthSelected.has(item.id));
  const snapshot = {
    reportKind: "health",
    generatedAt: new Date().toISOString(),
    root: sourceRoot,
    selectionCount: selected.length,
    stats: state.healthReport.stats,
    findings: selected
  };
  state.toolsBusy = true;
  state.toolsError = "";
  renderTools();
  try {
    const markdown = ["# Health review", "", "Selected findings: " + selected.length, "",
      ...selected.flatMap((item) => ["- [" + item.severity + "] " + item.category +
        " — " + item.relativePath + " — " + item.detail])].join("\n");
    const result = await invoke("save_audit_report", {
      folder: sourceRoot, kind: "health", markdown, jsonContent: JSON.stringify(snapshot, null, 2)
    });
    if (state.folder === sourceRoot) {
      state.healthExported = result;
      state.toolsNotice = t("healthExported") + ": " + result.jsonPath;
    }
  } catch (error) { state.toolsError = String(error); }
  finally { state.toolsBusy = false; renderTools(); }
}

function renderSnapshotDiff(diff) {
  el.toolsSnapshotDiff.innerHTML = "";
  if (!diff) return;
  for (const [label, values, pathKey] of [
    [t("addedFiles"), diff.added || [], "relativePath"],
    [t("removedFiles"), diff.removed || [], "relativePath"],
    [t("modifiedFiles"), diff.modified || [], "relativePath"],
    [t("movedFiles"), diff.moved || [], "newPath"],
  ]) {
    const section = document.createElement("div");
    section.className = "tools-diff-section";
    const h = document.createElement("h4"); h.textContent = `${label}: ${values.length}`;
    section.appendChild(h);
    for (const item of values.slice(0, 250)) {
      section.appendChild(toolListItem(item[pathKey] || "—",
        item.oldPath ? `${item.oldPath} → ${item.newPath}` : item.sha256 || ""));
    }
    el.toolsSnapshotDiff.appendChild(section);
  }
}

function renderSnapshotsTools() {
  el.toolsSnapshotList.innerHTML = "";
  for (const snapshot of state.snapshots || []) {
    const row = toolListItem(snapshot.createdAt, `${snapshot.entries?.length || 0} packages`);
    const compare = document.createElement("button");
    compare.type = "button";
    compare.className = "secondary-btn compact-btn";
    compare.textContent = t("snapshotCompare");
    compare.addEventListener("click", async () => {
      state.toolsBusy = true; renderTools();
      try {
        state.snapshotDiff = await invoke("compare_snapshot_to_current", {
          folder: state.folder, snapshotId: snapshot.id,
        });
      } catch (error) { state.toolsError = String(error); }
      finally { state.toolsBusy = false; renderTools(); }
    });
    row.appendChild(compare);
    el.toolsSnapshotList.appendChild(row);
  }
  el.toolsCompareRoot.value = state.compareRoot;
  renderSnapshotDiff(state.snapshotDiff);
}

function renderInboxTools() {
  el.toolsInboxPath.textContent = state.inboxFolder || t("noInbox");
  el.toolsInboxResults.innerHTML = "";
  for (const item of state.inboxScan?.items || []) {
    const row = toolListItem(item.name,
      `${item.status} · ${item.destinationPath || t("noDestination")}`,
      state.inboxSelected.has(item.path) ? "selected" : "");
    row.addEventListener("click", () => {
      if (state.inboxSelected.has(item.path)) state.inboxSelected.delete(item.path);
      else state.inboxSelected.add(item.path);
      state.inboxPlan = null;
      renderInboxTools();
    });
    el.toolsInboxResults.appendChild(row);
  }
  el.toolsPreviewImport.disabled = !state.inboxFolder || !state.inboxSelected.size || state.toolsBusy;
  el.toolsExecuteImport.disabled =
    !state.inboxPlan?.canExecute || workspaceReadOnly() || state.toolsBusy;

  el.toolsInboxPlan.classList.toggle("hidden", !state.inboxPlan);
  el.toolsInboxPlan.innerHTML = "";
  if (state.inboxPlan) {
    el.toolsInboxPlan.appendChild(toolListItem(
      t("importReady"),
      `${state.inboxPlan.ready} ready · ${state.inboxPlan.blocked} blocked · ${state.inboxPlan.destinationRoot}`
    ));
    for (const item of state.inboxPlan.items || []) {
      el.toolsInboxPlan.appendChild(toolListItem(item.relativePath, item.status));
    }
  }
}

async function loadMetadataSelection() {
  const path = el.toolsMetadataPackage.value;
  if (!path || !state.folder) return;
  try {
    const details = await invoke("get_package_technical_details", {
      folder: state.folder, packagePath: path,
    });
    const meta = state.workspaceStore?.packageMetadata?.[details.fileSha256] || null;
    el.toolsTags.value = (meta?.tags || []).join(", ");
    el.toolsTestStatus.value = meta?.testStatus || "";
    el.toolsFavorite.checked = !!meta?.favorite;
  } catch (_) {
    el.toolsTags.value = ""; el.toolsTestStatus.value = ""; el.toolsFavorite.checked = false;
  }
}

function renderMetadataTools() {
  const currentPackage = el.toolsMetadataPackage.value;
  el.toolsMetadataPackage.innerHTML = "";
  for (const item of state.items || []) {
    const option = document.createElement("option");
    option.value = item.path; option.textContent = item.relativePath || item.name;
    el.toolsMetadataPackage.appendChild(option);
  }
  if ([...el.toolsMetadataPackage.options].some((o) => o.value === currentPackage)) {
    el.toolsMetadataPackage.value = currentPackage;
  }

  el.toolsGroupPackageList.innerHTML = "";
  for (const item of state.items || []) {
    const row = toolListItem(item.name, item.relativePath,
      state.metadataGroupSelected.has(item.path) ? "selected" : "");
    row.addEventListener("click", () => {
      if (state.metadataGroupSelected.has(item.path)) state.metadataGroupSelected.delete(item.path);
      else state.metadataGroupSelected.add(item.path);
      renderMetadataTools();
    });
    el.toolsGroupPackageList.appendChild(row);
  }

  el.toolsGroupsList.innerHTML = "";
  for (const group of state.workspaceStore?.groups || []) {
    const row = toolListItem(group.name, `${group.memberSha256?.length || 0} packages · keep together`);
    const remove = document.createElement("button");
    remove.type = "button"; remove.className = "secondary-btn compact-btn"; remove.textContent = t("deleteGroup");
    remove.addEventListener("click", async () => {
      state.workspaceStore = await invoke("delete_package_group", { folder: state.folder, id: group.id });
      state.plan = null; renderTools();
    });
    row.appendChild(remove); el.toolsGroupsList.appendChild(row);
  }
}

function renderTechnicalTools() {
  el.toolsTechResults.innerHTML = "";
  for (const hit of state.technicalResults || []) {
    const row = toolListItem(hit.relativePath,
      hit.tgi || hit.fileSha256,
      state.technicalSelected.has(hit.packagePath) ? "selected" : "");
    row.addEventListener("click", () => {
      if (state.technicalSelected.has(hit.packagePath)) state.technicalSelected.delete(hit.packagePath);
      else state.technicalSelected.add(hit.packagePath);
      renderTechnicalTools();
    });
    el.toolsTechResults.appendChild(row);
  }

  const currentLeft = el.toolsCompareLeft.value;
  const currentRight = el.toolsCompareRight.value;
  for (const select of [el.toolsCompareLeft, el.toolsCompareRight]) {
    select.innerHTML = "";
    for (const item of state.items || []) {
      const option = document.createElement("option");
      option.value = item.path; option.textContent = item.relativePath || item.name;
      select.appendChild(option);
    }
  }
  if ([...el.toolsCompareLeft.options].some((o) => o.value === currentLeft)) el.toolsCompareLeft.value = currentLeft;
  if ([...el.toolsCompareRight.options].some((o) => o.value === currentRight)) el.toolsCompareRight.value = currentRight;

  el.toolsPackageCompare.innerHTML = "";
  if (state.packageCompare) {
    const result = state.packageCompare;
    el.toolsPackageCompare.appendChild(toolListItem(
      `${result.identicalResources} identical · ${result.changedResources} changed`,
      `only left ${result.onlyLeft} · only right ${result.onlyRight}`
    ));
    for (const resource of (result.resources || []).slice(0, 500)) {
      const row = document.createElement("div");
      row.className = `package-compare-row compare-${resource.relation}`;
      const tgi = document.createElement("code");
      tgi.textContent = resource.tgi;
      const relation = document.createElement("strong");
      relation.textContent = resource.relation;
      const left = document.createElement("code");
      left.textContent = resource.leftPayload || "—";
      left.title = resource.leftPayload || "";
      const right = document.createElement("code");
      right.textContent = resource.rightPayload || "—";
      right.title = resource.rightPayload || "";
      row.append(tgi, relation, left, right);
      el.toolsPackageCompare.appendChild(row);
    }
  }

  el.toolsDependencyResults.innerHTML = "";
  if (state.dependenciesAnalysis) {
    const dependencyNotes = [t("dependenciesConservative")];
    if (state.dependenciesAnalysis.skippedLargeReferenceResources > 0) {
      dependencyNotes.push(
        `${t("dependencySkippedLarge")}: ${state.dependenciesAnalysis.skippedLargeReferenceResources}`
      );
    }
    if (state.dependenciesAnalysis.truncated) dependencyNotes.push(t("dependencyTruncated"));
    const summary = toolListItem(
      `${state.dependenciesAnalysis.findings?.length || 0} potential dependencies`,
      dependencyNotes.join(" · ")
    );
    el.toolsDependencyResults.appendChild(summary);
    for (const [index, group] of (state.dependenciesAnalysis.suggestedGroups || []).entries()) {
      const row = toolListItem(
        `${t("suggestedDependencyGroup")} ${index + 1}`,
        `${group.packagePaths?.length || 0} packages · ${group.evidenceCount || 0} evidence`
      );
      const button = document.createElement("button");
      button.type = "button";
      button.className = "secondary-btn compact-btn";
      button.textContent = t("reviewSuggestedGroup");
      button.addEventListener("click", () => {
        state.metadataGroupSelected.clear();
        for (const relativePath of group.packagePaths || []) {
          const match = state.items.find((item) => item.relativePath === relativePath);
          if (match) state.metadataGroupSelected.add(match.path);
        }
        el.toolsGroupName.value = `${t("suggestedDependencyGroup")} ${index + 1}`;
        state.toolsTab = "metadata";
        renderTools();
      });
      row.appendChild(button);
      el.toolsDependencyResults.appendChild(row);
    }
    for (const finding of (state.dependenciesAnalysis.findings || []).slice(0, 1000)) {
      const row = toolListItem(
        `${finding.sourceRelativePath} → ${finding.targetRelativePath}`,
        `${finding.sourceResource} → ${finding.targetResource} · ${finding.evidence}`
      );
      row.title = t("keepTogetherGroups");
      row.addEventListener("click", () => {
        const source = state.items.find((item) => item.relativePath === finding.sourceRelativePath);
        const target = state.items.find((item) => item.relativePath === finding.targetRelativePath);
        if (source) state.metadataGroupSelected.add(source.path);
        if (target) state.metadataGroupSelected.add(target.path);
        state.toolsTab = "metadata";
        renderTools();
      });
      el.toolsDependencyResults.appendChild(row);
    }
  }
}

async function restoreQuarantineFromHistory(item) {
  if (state.pendingAction !== "restore_quarantine" ||
      state.pendingQuarantineRestore?.destination !== item?.destination ||
      el.confirmModal.classList.contains("hidden") ||
      !state.folder || !item?.destination || workspaceReadOnly() || state.toolsBusy) return;
  state.toolsBusy = true;
  state.toolsError = "";
  renderTools();
  try {
    const result = await invoke("restore_quarantine", {
      folder: state.folder,
      manifestPath: item.destination,
      confirmed: true,
    });
    state.toolsNotice = `${t("quarantineRestored")}: ${result.moved}`;
    state.duplicatesAnalysis = null;
    state.quarantinePlan = null;
    state.quarantineSelected.clear();
    await Promise.all([refreshOperationHistory(), refreshCacheInfo()]);
    state.toolsBusy = false;
    await scanFolder(false, true);
  } catch (error) {
    state.toolsError = String(error);
  } finally {
    state.toolsBusy = false;
    state.pendingAction = "";
    state.pendingQuarantineRestore = null;
    el.confirmModal.classList.add("hidden");
    el.confirmModal.setAttribute("aria-hidden", "true");
    el.confirmActionBtn.disabled = false;
    render();
  }
}

async function recoverQuarantineFromHistory(item) {
  if (!state.folder || !item?.destination || workspaceReadOnly() || state.toolsBusy) return;
  state.toolsBusy = true;
  state.toolsError = "";
  renderTools();
  try {
    const result = await invoke("recover_quarantine", {
      folder: state.folder,
      manifestPath: item.destination,
    });
    state.toolsNotice = `${t("quarantineRecovered")}: ${result.status} · ${result.moved}`;
    state.duplicatesAnalysis = null;
    state.quarantinePlan = null;
    state.quarantineSelected.clear();
    state.toolsBusy = false;
    await Promise.all([refreshOperationHistory(), refreshCacheInfo()]);
    await scanFolder(false, true);
  } catch (error) {
    state.toolsError = String(error);
  } finally {
    state.toolsBusy = false;
    render();
  }
}

function historyRecordCanBeRemoved(item) {
  if (!item?.destination || !state.folder || workspaceReadOnly() ||
      state.toolsBusy || state.restoreBusy || state.quarantineBusy ||
      state.executing) return false;
  if (item.kind === "restore_manifest") {
    const matching = state.restoreHistory.find(record => record.path === item.destination);
    return !!matching && (!matching.valid || matching.matchesSelectedRoot);
  }
  if (item.kind === "quarantine") {
    return item.status === "RESTORED" || item.status === "ROLLED_BACK";
  }
  return false;
}

async function removeOperationHistoryRecord() {
  const item = state.pendingHistoryRemoval;
  if (state.pendingAction !== "remove_operation_history" ||
      !item || !historyRecordCanBeRemoved(item) ||
      el.confirmModal.classList.contains("hidden")) return;

  state.toolsBusy = true;
  state.toolsError = "";
  state.toolsNotice = "";
  el.confirmActionBtn.disabled = true;
  renderTools();
  try {
    if (item.kind === "restore_manifest") {
      await invoke("remove_restore_history", {
        folder: state.folder,
        manifestPath: item.destination,
        confirmed: true,
      });
      if (state.restoreManifest === item.destination) {
        state.restoreManifest = "";
        state.restorePlan = null;
        state.restorePreviewManifest = "";
      }
    } else if (item.kind === "quarantine") {
      await invoke("remove_quarantine_history", {
        folder: state.folder,
        manifestPath: item.destination,
        confirmed: true,
      });
    }
    state.toolsNotice = t("historyRecordRemoved");
  } catch (error) {
    state.toolsError = String(error);
  } finally {
    state.toolsBusy = false;
    state.pendingAction = "";
    state.pendingHistoryRemoval = null;
    el.confirmModal.classList.add("hidden");
    el.confirmModal.setAttribute("aria-hidden", "true");
    el.confirmActionBtn.disabled = false;
    await Promise.all([loadRestoreHistory(), refreshOperationHistory()]);
    render();
  }
}

function renderHistoryTools() {
  el.toolsOperationHistory.innerHTML = "";
  for (const item of (state.operationHistory || []).filter(
    (entry) => ["restore_manifest", "quarantine"].includes(entry.kind)
  )) {
    const row = toolListItem(
      `${item.kind}: ${item.title}`,
      `${item.timestamp} · ${item.source || ""}${item.destination ? " → " + item.destination : ""} · ${item.status}`,
      "history-record-row"
    );
    if (item.destination) {
      const actions = document.createElement("div");
      actions.className = "history-record-actions";

      const openButton = document.createElement("button");
      openButton.type = "button";
      openButton.className = "secondary-btn compact-btn";
      openButton.textContent = t("openStructureLocation");
      openButton.addEventListener("click", () => revealSafe(item.destination));
      actions.appendChild(openButton);

      if (item.kind === "quarantine" &&
          ["PENDING", "ROLLBACK_INCOMPLETE", "RESTORE_PENDING", "RESTORE_INCOMPLETE"].includes(item.status)) {
        const recover = document.createElement("button");
        recover.type = "button";
        recover.className = "secondary-btn compact-btn";
        recover.textContent = t("recoverQuarantine");
        recover.disabled = workspaceReadOnly() || state.toolsBusy;
        recover.addEventListener("click", () => recoverQuarantineFromHistory(item));
        actions.appendChild(recover);
      }

      if (item.kind === "quarantine" && item.status === "COMPLETE") {
        const restore = document.createElement("button");
        restore.type = "button";
        restore.className = "secondary-btn compact-btn";
        restore.textContent = t("restoreQuarantine");
        restore.disabled = workspaceReadOnly() || state.toolsBusy;
        restore.addEventListener("click", () => {
          state.pendingQuarantineRestore = item;
          openConfirm("restore_quarantine");
        });
        actions.appendChild(restore);
      }

      const remove = document.createElement("button");
      remove.type = "button";
      remove.className = "danger-btn compact-btn";
      remove.textContent = t("removeRestoreManifest");
      remove.disabled = !historyRecordCanBeRemoved(item);
      if (item.kind === "quarantine" &&
          !["RESTORED", "ROLLED_BACK"].includes(item.status)) {
        remove.title = t("historyRemovalUnavailable");
      }
      remove.addEventListener("click", () => {
        if (!historyRecordCanBeRemoved(item)) return;
        state.pendingHistoryRemoval = item;
        openConfirm("remove_operation_history");
      });
      actions.appendChild(remove);
      row.appendChild(actions);
    }
    el.toolsOperationHistory.appendChild(row);
  }
}

async function refreshSnapshots() {
  if (!state.folder) return;
  try { state.snapshots = await invoke("list_snapshots", { folder: state.folder }); }
  catch (error) { state.toolsError = String(error); }
  renderTools();
}

async function refreshOperationHistory() {
  if (!state.folder) return;
  try { state.operationHistory = await invoke("get_operation_history", { folder: state.folder }); }
  catch (error) { state.toolsError = String(error); }
  renderTools();
}

async function refreshToolsContext() {
  if (!state.folder) return;
  await Promise.all([
    loadWorkspaceTools(),
    refreshSnapshots(),
    refreshOperationHistory(),
  ]);
}

async function toggleReadOnly() {
  if (!state.folder) return;
  state.toolsBusy = true;
  try {
    state.workspaceStore = await invoke("set_read_only", {
      folder: state.folder,
      readOnly: el.toolsReadOnly.checked,
    });
    state.toolsNotice = el.toolsReadOnly.checked ? t("readOnlyEnabled") : t("readOnlyDisabled");
  } catch (error) { state.toolsError = String(error); }
  finally { state.toolsBusy = false; render(); }
}

async function addWorkspaceProfile() {
  if (!state.workspaceStore) return;
  const name = el.toolsProfileName.value.trim();
  if (!name) return;
  const id = `profile-${Date.now()}`;
  state.workspaceStore.profiles.push({
    id, name, destinationPrefix: "", collapseToCategory: false,
    rules: [], protectedFolders: [],
  });
  state.workspaceStore.activeProfileId = id;
  el.toolsProfileName.value = "";
  await persistWorkspaceStore();
}

async function deleteActiveProfile() {
  const store = state.workspaceStore;
  const profile = activeWorkspaceProfile();
  if (!store || !profile || profile.id === "default" || state.toolsBusy) return;
  store.profiles = store.profiles.filter((item) => item.id !== profile.id);
  store.activeProfileId = "default";
  state.plan = null;
  await persistWorkspaceStore();
}

async function saveActiveProfile() {
  const profile = activeWorkspaceProfile();
  if (!profile) return;
  profile.destinationPrefix = el.toolsProfilePrefix.value.trim();
  profile.collapseToCategory = el.toolsProfileCollapse.checked;
  await persistWorkspaceStore();
}

async function addProtectedFolder() {
  const profile = activeWorkspaceProfile();
  const value = el.toolsProtectedInput.value.trim().replaceAll("/", "\\").replace(/^\\+|\\+$/g, "");
  if (!profile || !value) return;
  if (!(profile.protectedFolders || []).some((folder) => folder.toLowerCase() === value.toLowerCase())) {
    profile.protectedFolders.push(value);
  }
  el.toolsProtectedInput.value = "";
  await persistWorkspaceStore();
}

async function addCustomRule() {
  const profile = activeWorkspaceProfile();
  if (!profile) return;
  const destination = el.ruleDestination.value.trim();
  if (!destination) return;
  profile.rules.push({
    id: `rule-${Date.now()}`,
    name: el.ruleName.value.trim() || `Rule ${profile.rules.length + 1}`,
    enabled: true,
    category: el.ruleCategory.value.trim() || null,
    subCategory: el.ruleSubcategory.value.trim() || null,
    detectedFrom: el.ruleDetected.value.trim() || null,
    nameContains: el.ruleNameContains.value.trim() || null,
    pathContains: el.rulePathContains.value.trim() || null,
    destination,
  });
  for (const input of [el.ruleName, el.ruleCategory, el.ruleSubcategory, el.ruleDetected, el.ruleNameContains, el.rulePathContains, el.ruleDestination]) input.value = "";
  await persistWorkspaceStore();
}

async function analyzeHealth() {
  if (!state.folder || toolsOperationLocked()) return;
  state.toolsBusy = true; state.toolsError = ""; beginToolsOperation("health");
  renderTools();
  try {
    state.healthReport = await invoke("analyze_mods_health", { folder: state.folder });
    state.healthSelected.clear();
  } catch (error) { state.toolsError = String(error); }
  finally { state.toolsBusy = false; state.toolsOperation = null; renderTools(); }
}

async function createSnapshotTool() {
  if (!state.folder || toolsOperationLocked()) return;
  state.toolsBusy = true; beginToolsOperation("snapshot"); renderTools();
  try {
    await invoke("create_snapshot", { folder: state.folder });
    state.toolsNotice = t("snapshotCreated");
    await refreshSnapshots();
    await refreshOperationHistory();
  } catch (error) { state.toolsError = String(error); }
  finally { state.toolsBusy = false; state.toolsOperation = null; renderTools(); }
}

async function chooseCompareRoot() {
  const selected = await open({ directory: true, multiple: false, title: t("compareFolders") });
  if (typeof selected === "string") {
    state.compareRoot = selected;
    renderTools();
  }
}

async function compareRootsTool() {
  if (!state.folder || !state.compareRoot || toolsOperationLocked()) return;
  state.toolsBusy = true; beginToolsOperation("snapshot"); renderTools();
  try {
    state.snapshotDiff = await invoke("compare_mods_roots", {
      leftFolder: state.folder,
      rightFolder: state.compareRoot,
    });
  } catch (error) { state.toolsError = String(error); }
  finally { state.toolsBusy = false; state.toolsOperation = null; renderTools(); }
}

async function chooseInboxFolder() {
  const selected = await open({ directory: true, multiple: false, title: t("chooseInbox") });
  if (typeof selected === "string") {
    state.inboxFolder = selected;
    state.inboxScan = null; state.inboxSelected.clear(); state.inboxPlan = null;
    renderTools();
  }
}

async function scanInboxTool() {
  if (!state.inboxFolder || toolsOperationLocked()) return;
  state.toolsBusy = true; beginToolsOperation("inbox_scan"); renderTools();
  try {
    state.inboxScan = await invoke("scan_inbox", {
      sourceFolder: state.inboxFolder,
      language: state.language,
    });
    state.inboxSelected.clear(); state.inboxPlan = null;
  } catch (error) { state.toolsError = String(error); }
  finally { state.toolsBusy = false; state.toolsOperation = null; renderTools(); }
}

async function previewInboxImport() {
  if (!state.folder || !state.inboxFolder || !state.inboxSelected.size || toolsOperationLocked()) return;
  state.toolsBusy = true; beginToolsOperation("inbox_plan"); renderTools();
  try {
    state.inboxPlan = await invoke("build_inbox_import_plan", {
      folder: state.folder,
      sourceFolder: state.inboxFolder,
      selectedPaths: [...state.inboxSelected],
    });
  } catch (error) { state.toolsError = String(error); }
  finally { state.toolsBusy = false; state.toolsOperation = null; renderTools(); }
}

async function executeInboxImport() {
  if (!state.inboxPlan?.canExecute || workspaceReadOnly() || toolsOperationLocked()) return;
  state.toolsBusy = true; beginToolsOperation("inbox_import"); renderTools();
  try {
    const result = await invoke("execute_inbox_import", {
      folder: state.folder,
      sourceFolder: state.inboxFolder,
      selectedPaths: [...state.inboxSelected],
    });
    state.toolsNotice = [
      `${result.imported} imported → ${result.destinationRoot}`,
      ...(result.warnings || []),
    ].join(" · ");
    state.inboxSelected.clear(); state.inboxPlan = null;
    await Promise.all([scanFolder(false, true), refreshOperationHistory(), refreshCacheInfo()]);
  } catch (error) { state.toolsError = String(error); }
  finally { state.toolsBusy = false; state.toolsOperation = null; render(); }
}

async function savePackageMetadataTool() {
  const packagePath = el.toolsMetadataPackage.value;
  if (!state.folder || !packagePath || toolsOperationLocked()) return;
  state.toolsBusy = true; renderTools();
  try {
    state.workspaceStore = await invoke("set_package_metadata", {
      folder: state.folder,
      packagePath,
      tags: el.toolsTags.value.split(",").map((value) => value.trim()).filter(Boolean),
      testStatus: el.toolsTestStatus.value,
      favorite: el.toolsFavorite.checked,
    });
    state.toolsNotice = t("metadataSaved");
  } catch (error) { state.toolsError = String(error); }
  finally { state.toolsBusy = false; renderTools(); }
}

async function saveGroupTool() {
  if (!state.folder || !state.metadataGroupSelected.size || toolsOperationLocked()) return;
  state.toolsBusy = true; renderTools();
  try {
    state.workspaceStore = await invoke("save_package_group", {
      folder: state.folder,
      id: "",
      name: el.toolsGroupName.value.trim() || `Group ${Date.now()}`,
      keepTogether: true,
      packagePaths: [...state.metadataGroupSelected],
    });
    state.metadataGroupSelected.clear(); el.toolsGroupName.value = "";
    state.plan = null; state.toolsNotice = t("groupSaved");
  } catch (error) { state.toolsError = String(error); }
  finally { state.toolsBusy = false; renderTools(); }
}

async function runTechnicalSearch() {
  if (!state.folder || !el.toolsTechQuery.value.trim() || toolsOperationLocked()) return;
  state.toolsBusy = true; beginToolsOperation("technical_search"); renderTools();
  try {
    state.technicalResults = await invoke("technical_search", {
      folder: state.folder, query: el.toolsTechQuery.value,
    });
    state.technicalSelected.clear();
  } catch (error) { state.toolsError = String(error); }
  finally { state.toolsBusy = false; state.toolsOperation = null; renderTools(); }
}

function technicalExportContent(format) {
  let selected = state.technicalSelected.size
    ? state.technicalResults.filter((hit) => state.technicalSelected.has(hit.packagePath))
    : state.technicalResults;
  if (!selected.length && state.selectedForPlan.size) {
    selected = state.items
      .filter((item) => state.selectedForPlan.has(item.id))
      .map((item) => ({
        packagePath: item.path,
        relativePath: item.relativePath,
        fileSha256: "",
        tgi: "",
        payloadSha256: "",
      }));
  }
  if (format === "json") return JSON.stringify(selected, null, 2);
  if (format === "csv") {
    const esc = (value) => `"${String(value ?? "").replaceAll('"', '""')}"`;
    return [
      ["package","relative_path","file_sha256","tgi","payload_sha256"].join(","),
      ...selected.map((hit) => [hit.packagePath,hit.relativePath,hit.fileSha256,hit.tgi,hit.payloadSha256].map(esc).join(",")),
    ].join("\n");
  }
  return selected.map((hit) => [hit.relativePath, hit.tgi || "", hit.fileSha256].filter(Boolean).join(" | ")).join("\n");
}

async function exportTechnicalSelection(format) {
  if (!state.folder || (!state.technicalResults.length && !state.selectedForPlan.size)) return;
  try {
    const result = await invoke("save_selection_export", {
      folder: state.folder, format, content: technicalExportContent(format),
    });
    state.toolsNotice = `${t("exportSaved")}: ${result.path}`;
  } catch (error) { state.toolsError = String(error); }
  renderTools();
}

async function comparePackagesTool() {
  const leftPath = el.toolsCompareLeft.value;
  const rightPath = el.toolsCompareRight.value;
  if (!leftPath || !rightPath || leftPath === rightPath || toolsOperationLocked()) return;
  state.toolsBusy = true; beginToolsOperation("package_compare"); renderTools();
  try {
    state.packageCompare = await invoke("compare_packages", {
      folder: state.folder, leftPath, rightPath,
    });
  } catch (error) { state.toolsError = String(error); }
  finally { state.toolsBusy = false; state.toolsOperation = null; renderTools(); }
}

async function analyzeDependenciesTool() {
  if (!state.folder || toolsOperationLocked()) return;
  state.toolsBusy = true; beginToolsOperation("dependencies"); renderTools();
  try {
    state.dependenciesAnalysis = await invoke("analyze_dependencies", { folder: state.folder });
  } catch (error) { state.toolsError = String(error); }
  finally { state.toolsBusy = false; state.toolsOperation = null; renderTools(); }
}

async function undoManualOperation() {
  if (!state.folder || workspaceReadOnly() || structureLocked()) return;
  state.structureBusy = true; renderStructure();
  try {
    const result = await invoke("undo_last_manual_operation", { folder: state.folder });
    state.structureNotice = `${t("undoLast")}: ${result.destinationRelativePath || ""}`;
    invalidateAnalysesAfterStructureChange();
    await Promise.all([
      refreshManualOperations(), refreshOperationHistory(), refreshCacheInfo(),
      refreshConflictDecisions(),
    ]);
    state.structureBusy = false;
    await loadStructure(state.structureCurrent || "");
  } catch (error) {
    state.structureError = String(error);
  } finally {
    state.structureBusy = false; render();
  }
}

function renderTools() {
  if (!el.toolsState) return;
  for (const button of el.toolsSubtabs) {
    button.classList.toggle("active", button.dataset.toolsTab === state.toolsTab);
  }
  for (const panel of el.toolsPanels) {
    panel.classList.toggle("hidden", panel.id !== `tools-${state.toolsTab}`);
  }

  if (state.toolsBusy) {
    const operation = state.toolsOperationStatus;
    const progress = operation?.total
      ? ` · ${operation.processed || 0}/${operation.total}`
      : "";
    el.toolsState.textContent = operation?.phase
      ? `${operation.phase}${progress}`
      : t("scanning");
    el.toolsState.className = "scan-state busy";
  } else if (state.toolsError) {
    el.toolsState.textContent = state.toolsError;
    el.toolsState.className = "scan-state error";
  } else if (state.toolsNotice) {
    el.toolsState.textContent = state.toolsNotice;
    el.toolsState.className = "scan-state success";
  } else {
    el.toolsState.textContent = "";
    el.toolsState.className = "scan-state";
  }
  if (el.toolsCancel) {
    const active = !!state.toolsBusy && !!state.toolsOperation;
    const transactional = ["inbox_import", "sims3pack_convert"].includes(state.toolsOperation);
    el.toolsCancel.classList.toggle("hidden", !active || transactional);
    el.toolsCancel.disabled = !!state.toolsOperationStatus?.cancelRequested || transactional;
    el.toolsCancel.textContent = state.toolsOperationStatus?.cancelRequested
      ? t("cancelling")
      : t("cancel");
  }

  renderProfileTools();
  renderHealthTools();
  renderSnapshotsTools();
  renderInboxTools();
  renderMetadataTools();
  renderTechnicalTools();
  renderHistoryTools();
  renderFixIndexPanel();
}

function renderTabs() {
  for (const button of el.tabs) {
    const active = button.dataset.tab === state.tab;
    button.classList.toggle("active", active);
    button.disabled = false;
    button.setAttribute("aria-selected", String(active));
  }
  for (const page of el.pages) {
    page.classList.toggle("hidden", page.id !== `page-${state.tab}`);
  }
}

function renderStats() {
  const stats = state.stats || {};
  el.statPackages.textContent = stats.packages ?? 0;
  el.statClassified.textContent = stats.classified ?? 0;
  el.statMixed.textContent = stats.mixed ?? 0;
  el.statReview.textContent = stats.needsReview ?? 0;
  el.statUnknown.textContent = stats.unknown ?? 0;
  el.statInvalid.textContent = stats.invalid ?? 0;
  el.statDisabled.textContent = stats.disabled ?? 0;
}

function metadataForItem(item) {
  if (!item || !state.workspaceStore?.packageMetadata) return null;
  const details = state.technicalDetails[item.path];
  if (details?.fileSha256 && state.workspaceStore.packageMetadata[details.fileSha256]) {
    return state.workspaceStore.packageMetadata[details.fileSha256];
  }
  const target = String(item.relativePath || "").replaceAll("/", "\\").toLowerCase();
  return Object.values(state.workspaceStore.packageMetadata).find(
    (meta) => String(meta.lastPath || "").replaceAll("/", "\\").toLowerCase() === target
  ) || null;
}

function visibleItems() {
  const search = state.search.trim().toLocaleLowerCase();
  if (
    managerVisibleMemo.items === state.items &&
    managerVisibleMemo.status === state.status &&
    managerVisibleMemo.search === search
  ) {
    return managerVisibleMemo.result;
  }

  const result = state.items.filter((item) => {
    if (state.status !== "all" && item.status !== state.status) return false;
    return itemMatchesManagerSearch(item, search);
  });

  managerVisibleMemo.items = state.items;
  managerVisibleMemo.status = state.status;
  managerVisibleMemo.search = search;
  managerVisibleMemo.result = result;
  return result;
}

function renderSelectionSummary() {
  const count = state.selectedForPlan.size;
  el.selectionSummary.textContent = `${count} ${t("selectedEligible")}`;
  el.planBtn.disabled =
    count === 0 ||
    state.scanning ||
    state.planning ||
    state.executing ||
    state.structureBusy;
}

function appendMeta(container, label, value) {
  if (value == null || value === "" || (Array.isArray(value) && !value.length)) return;

  const row = document.createElement("div");
  row.className = "meta-row";
  const dt = document.createElement("span");
  dt.className = "meta-label";
  dt.textContent = label;
  const dd = document.createElement("strong");
  dd.textContent = Array.isArray(value) ? value.join(", ") : String(value);
  row.append(dt, dd);
  container.appendChild(row);
}

async function loadPackagePreview(item) {
  if (!item || !state.folder || state.packagePreviewLoading[item.path]) return;
  state.packagePreviewLoading[item.path] = true;
  delete state.packagePreviewErrors[item.path];
  renderPreview();
  try {
    state.packagePreviews[item.path] = await invoke("get_package_preview", {
      folder: state.folder,
      packagePath: item.path,
    });
  } catch (error) {
    state.packagePreviewErrors[item.path] = String(error);
  } finally {
    delete state.packagePreviewLoading[item.path];
    renderPreview();
  }
}

function manualClassificationForItem(item) {
  const values = Object.values(state.workspaceStore?.manualClassifications || {});
  const target = String(item?.relativePath || "").replaceAll("/", "\\").toLowerCase();
  return values.find(
    (entry) => String(entry.lastPath || "").replaceAll("/", "\\").toLowerCase() === target
  ) || null;
}

async function saveManualReview(item, destination) {
  if (!state.folder || !item || state.reviewBusy) return;
  state.reviewBusy = true;
  state.error = "";
  renderPreview();
  try {
    state.workspaceStore = await invoke("set_manual_classification", {
      folder: state.folder,
      packagePath: item.path,
      destination,
    });
    state.notice = t("manualReviewSaved");
    state.plan = null;
    const selectedPath = item.path;
    state.reviewBusy = false;
    await scanFolder(true, true);
    const refreshed = state.items.find((candidate) => candidate.path === selectedPath);
    if (refreshed) state.selectedId = refreshed.id;
  } catch (error) {
    state.error = String(error);
  } finally {
    state.reviewBusy = false;
    render();
  }
}

function renderPreview() {
  const item = state.items.find((candidate) => candidate.id === state.selectedId);
  el.previewCard.innerHTML = "";

  if (!item) {
    const empty = document.createElement("div");
    empty.className = "preview-empty";
    empty.textContent = t("selectPackage");
    el.previewCard.appendChild(empty);
    return;
  }

  const header = document.createElement("div");
  header.className = "preview-header";
  const name = document.createElement("h3");
  name.textContent = item.name;
  const badge = document.createElement("span");
  badge.className = `status-badge status-${item.status}`;
  badge.textContent = statusLabel(item.status);
  header.append(name, badge);

  const meta = document.createElement("div");
  meta.className = "preview-meta";
  appendMeta(meta, t("detectedFrom"), item.detectedFrom);
  appendMeta(meta, t("classificationConfidence"), confidenceLabel(item.classificationConfidence));
  appendMeta(meta, t("creator"), item.creator);
  appendMeta(meta, t("modName"), item.modName);
  appendMeta(meta, t("gameplayCategory"), item.gameplayCategory);
  appendMeta(meta, t("category"), item.category);
  appendMeta(meta, t("subCategory"), item.subCategory);
  appendMeta(meta, t("gender"), item.gender);
  appendMeta(meta, t("age"), item.age);
  appendMeta(meta, t("species"), item.species);
  appendMeta(meta, t("usageCategories"), item.usageCategories);
  appendMeta(meta, t("resources"), `${item.resourceCount} · ${(item.resourceTypes || []).join(", ")}`);
  appendMeta(meta, t("originalPath"), item.relativePath);
  appendMeta(meta, t("suggestedDestination"), item.destinationPath || t("noDestination"));
  appendMeta(meta, t("classificationReason"), item.classificationReason);
  const localMetadata = metadataForItem(item);
  appendMeta(meta, t("tags"), localMetadata?.tags);
  appendMeta(meta, t("testStatus"), localMetadata?.testStatus);
  if (localMetadata?.favorite) appendMeta(meta, t("favorite"), "★");

  if (!item.destinationPath && item.candidateDestinations?.length) {
    appendMeta(meta, t("possibleDestinations"), item.candidateDestinations);
  }
  if (!eligibleForPlan(item)) {
    appendMeta(meta, t("organizationPlan"), t("notEligible"));
  }

  const thumbWrap = document.createElement("button");
  thumbWrap.type = "button";
  thumbWrap.className = "package-preview-thumb-wrap";
  thumbWrap.title = t("previewOpenLocation");
  thumbWrap.addEventListener("click", () => revealSafe(item.path));
  const preview = state.packagePreviews[item.path];
  if (preview?.thumbnailBase64) {
    const image = document.createElement("img");
    image.className = "package-preview-thumb";
    image.alt = item.name;
    image.src = `data:${preview.mimeType || "image/png"};base64,${preview.thumbnailBase64}`;
    thumbWrap.appendChild(image);
  } else {
    const empty = document.createElement("span");
    empty.className = "package-preview-thumb-empty";
    empty.innerHTML = '<i class="fa-regular fa-image" aria-hidden="true"></i>';
    const label = document.createElement("small");
    label.textContent = state.packagePreviewLoading[item.path]
      ? t("previewLoading")
      : t("previewUnavailable");
    empty.appendChild(label);
    thumbWrap.appendChild(empty);
  }

  el.previewCard.append(thumbWrap, header, meta);

  if (!(item.path in state.packagePreviews)
      && !state.packagePreviewLoading[item.path]
      && !state.packagePreviewErrors[item.path]) {
    queueMicrotask(() => loadPackagePreview(item));
  }

  const existingManual = manualClassificationForItem(item);
  if (item.status !== "invalid" && (!eligibleForPlan(item) || existingManual || (item.detectedFrom || []).includes("ManualReview"))) {
    const review = document.createElement("div");
    review.className = "manual-review-box";
    const title = document.createElement("strong");
    title.textContent = t("manualReview");
    const hint = document.createElement("small");
    hint.textContent = t("manualReviewHint");
    const input = document.createElement("input");
    input.type = "text";
    input.className = "text-input";
    input.placeholder = t("manualDestination");
    input.value = existingManual?.destination || (item.destinationPath || "");
    const actions = document.createElement("div");
    actions.className = "preview-actions";
    const save = document.createElement("button");
    save.type = "button";
    save.className = "primary-btn";
    save.textContent = t("saveManualReview");
    save.disabled = state.reviewBusy;
    save.addEventListener("click", () => saveManualReview(item, input.value.trim()));
    actions.appendChild(save);
    if (existingManual || (item.detectedFrom || []).includes("ManualReview")) {
      const clear = document.createElement("button");
      clear.type = "button";
      clear.className = "secondary-btn";
      clear.textContent = t("clearManualReview");
      clear.disabled = state.reviewBusy;
      clear.addEventListener("click", () => saveManualReview(item, ""));
      actions.appendChild(clear);
    }
    review.append(title, hint, input, actions);
    el.previewCard.appendChild(review);
  }

  if (item.warnings?.length) {
    const warnings = document.createElement("div");
    warnings.className = "warning-box";
    const title = document.createElement("strong");
    title.textContent = t("warnings");
    const list = document.createElement("ul");
    for (const warning of item.warnings) {
      const li = document.createElement("li");
      li.textContent = warning;
      list.appendChild(li);
    }
    warnings.append(title, list);
    el.previewCard.appendChild(warnings);
  }

  const technical = document.createElement("div");
  technical.className = "technical-details";
  renderTechnicalDetails(technical, item);
  el.previewCard.appendChild(technical);
}

function setPlanSelection(id, checked) {
  if (checked) state.selectedForPlan.add(id);
  else state.selectedForPlan.delete(id);
  state.plan = null;
  state.planError = "";
  renderSelectionSummary();
}

function createPackageRow(item) {
  const row = document.createElement("div");
  row.className =
    "package-row" +
    (item.id === state.selectedId ? " active" : "") +
    (state.selectedForPlan.has(item.id) ? " plan-selected" : "");
  row.title = item.path;
  row.dataset.packageId = item.id;
  row.tabIndex = 0;
  row.setAttribute("role", "button");

  const selection = document.createElement("label");
  selection.className = "package-check";
  selection.title = eligibleForPlan(item) ? t("selected") : t("notEligible");

  const checkbox = document.createElement("input");
  checkbox.type = "checkbox";
  checkbox.checked = state.selectedForPlan.has(item.id);
  checkbox.disabled = !eligibleForPlan(item);
  checkbox.addEventListener("click", (event) => event.stopPropagation());
  checkbox.addEventListener("change", (event) => {
    event.stopPropagation();
    setPlanSelection(item.id, checkbox.checked);
    row.classList.toggle("plan-selected", checkbox.checked);
  });
  selection.appendChild(checkbox);

  const main = document.createElement("div");
  main.className = "package-main";
  const name = document.createElement("strong");
  const localMetadata = metadataForItem(item);
  name.textContent = `${localMetadata?.favorite ? "★ " : ""}${item.name}`;
  const details = document.createElement("span");
  details.textContent =
    [item.category, item.subCategory, item.gender, item.age]
      .filter(Boolean)
      .join(" › ") || statusLabel(item.status);
  main.append(name, details);

  const side = document.createElement("div");
  side.className = "package-side";
  const source = document.createElement("span");
  source.textContent = (item.detectedFrom || []).join(" + ") || "—";
  const badge = document.createElement("span");
  badge.className = `status-dot status-${item.status}`;
  badge.setAttribute("aria-label", statusLabel(item.status));
  side.append(source, badge);

  row.append(selection, main, side);

  const choose = () => {
    if (state.selectedId === item.id) return;
    state.selectedId = item.id;
    updateActiveVirtualRow(el.packageList, "packageId", item.id, "active");
    renderPreview();
  };
  row.addEventListener("click", choose);
  row.addEventListener("keydown", (event) => {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      choose();
    }
  });

  return row;
}

function renderManagerVirtualRows(items, force = false) {
  el.packageList.dataset.fluentCount = String(items.length);
  const view = virtualViews.manager;
  const changed = view.items !== items;
  if (changed) {
    view.items = items;
    view.start = -1;
    view.end = -1;
    el.packageList.scrollTop = 0;
  }

  if (!items.length) {
    view.start = 0;
    view.end = 0;
    const empty = document.createElement("div");
    empty.className = "list-empty";
    empty.textContent = t("noResults");
    el.packageList.replaceChildren(empty);
    return;
  }

  const { start, end } = virtualRange(el.packageList, items.length, MANAGER_ROW_STRIDE);
  if (!force && !changed && start === view.start && end === view.end) return;
  view.start = start;
  view.end = end;

  const fragment = document.createDocumentFragment();
  if (start > 0) fragment.appendChild(virtualSpacer(start * MANAGER_ROW_STRIDE));
  for (let index = start; index < end; index += 1) {
    const row = createPackageRow(items[index]);
    row.dataset.fluentIndex = String(index);
    fragment.appendChild(row);
  }
  if (end < items.length) {
    fragment.appendChild(virtualSpacer((items.length - end) * MANAGER_ROW_STRIDE));
  }
  el.packageList.replaceChildren(fragment);
}

function scheduleManagerVirtualRows() {
  const view = virtualViews.manager;
  if (view.raf) return;
  view.raf = window.requestAnimationFrame(() => {
    view.raf = 0;
    if (state.tab === "organizer" && state.stats) {
      renderManagerVirtualRows(visibleItems());
    }
  });
}

function renderResults(forceRows = false) {
  const hasScan = !!state.stats;
  el.emptyState.classList.toggle("hidden", hasScan);
  el.resultsState.classList.toggle("hidden", !hasScan);

  if (!hasScan) {
    virtualViews.manager.items = null;
    renderPreview();
    renderSelectionSummary();
    return;
  }

  const items = visibleItems();
  if (!items.some((item) => item.id === state.selectedId)) {
    state.selectedId = items[0]?.id || "";
  }

  renderManagerVirtualRows(items, forceRows);
  renderPreview();
  renderSelectionSummary();
}

function renderOrganizationReview() {
  const review = state.organizationReview;
  const visible = !!review && ((review.moved ?? 0) > 0 || (review.duplicates ?? 0) > 0 || (review.collisions ?? 0) > 0);

  el.organizationReviewPanel?.classList.toggle("hidden", !visible);
  if (!visible) return;
  el.openOrganizedResultBtn.disabled = !state.folder || state.executing;

  const summaryParts = [
    `${review.moved ?? 0} ${t("organizedPackages")}`,
    `${review.duplicates ?? 0} ${t("duplicatesPending")}`,
    `${review.collisions ?? 0} ${t("collisionsPending")}`,
  ];
  if (review.oldFoldersRemoved > 0) {
    summaryParts.push(`${review.oldFoldersRemoved} ${t("emptyOldFoldersRemoved")}`);
  }
  if (review.oldFoldersRetained > 0) {
    summaryParts.push(`${review.oldFoldersRetained} ${t("oldFoldersStillOccupied")}`);
  }
  if (review.remainingLegacyFiles > 0) {
    summaryParts.push(`${review.remainingLegacyFiles} ${t("remainingLegacyFiles")}`);
  }
  el.organizationReviewSummary.textContent = summaryParts.join(" · ");
  const leftovers = review.remainingLegacyExamples || [];
  el.organizationReviewLeftovers?.classList.toggle("hidden", leftovers.length === 0);
  if (el.organizationReviewLeftoverPaths) {
    el.organizationReviewLeftoverPaths.textContent = leftovers.join("\n");
  }

  el.reviewDuplicatesBtn.classList.toggle("hidden", !(review.duplicates > 0));
  el.reviewCollisionsBtn.classList.toggle("hidden", !(review.collisions > 0));
}

function renderCollisionReview() {
  if (!el.collisionReviewList) return;
  el.collisionReviewList.innerHTML = "";

  for (const item of state.organizationCollisionItems || []) {
    const card = document.createElement("article");
    card.className = "collision-review-item";

    const title = document.createElement("strong");
    title.textContent = item.name || item.sourceRelativePath || "package";

    const paths = document.createElement("div");
    paths.className = "collision-review-paths";
    for (const [label, value] of [
      [t("current"), item.sourceRelativePath],
      [t("proposed"), item.destinationRelativePath || t("noDestination")],
    ]) {
      const line = document.createElement("div");
      const key = document.createElement("span");
      key.textContent = label;
      const code = document.createElement("code");
      code.textContent = value || "—";
      line.append(key, code);
      paths.appendChild(line);
    }

    card.append(title, paths);

    if (item.warnings?.length) {
      const note = document.createElement("div");
      note.className = "collision-review-warning";
      note.textContent = item.warnings.join(" ");
      card.appendChild(note);
    }

    el.collisionReviewList.appendChild(card);
  }
}

function openCollisionReview() {
  if (!state.organizationCollisionItems?.length) return;
  renderCollisionReview();
  el.collisionReviewModal.classList.remove("hidden");
  el.collisionReviewModal.setAttribute("aria-hidden", "false");
}

function closeCollisionReview() {
  el.collisionReviewModal.classList.add("hidden");
  el.collisionReviewModal.setAttribute("aria-hidden", "true");
}

function renderPlan() {
  const plan = state.plan;
  if (!plan) return;

  const stats = plan.stats || {};
  const collisions =
    (stats.collisionSameContent ?? 0) + (stats.collisionDifferentContent ?? 0);

  el.planStatSelected.textContent = stats.selected ?? 0;
  const duplicateReady = Number(stats.duplicateQuarantineReady || 0);
  const executableReady = Number(stats.ready || 0);
  el.planStatReady.textContent = Math.max(0, executableReady - duplicateReady);
  if (el.planStatDuplicatesReady) el.planStatDuplicatesReady.textContent = duplicateReady;
  if (el.planStatOrganized) el.planStatOrganized.textContent = stats.alreadyOrganized ?? 0;
  el.planStatKept.textContent = stats.keptUncategorized ?? 0;
  el.planStatSkipped.textContent = stats.duplicateSkipped ?? 0;
  el.planStatCollisions.textContent = collisions;
  el.planStatBlocked.textContent = stats.blocked ?? 0;
  el.planStatFolders.textContent = stats.directoriesToCreate ?? 0;
  if (el.planStatFoldersRemove) el.planStatFoldersRemove.textContent = stats.emptyFoldersToClean ?? 0;
  if (el.planStatFoldersRetained) el.planStatFoldersRetained.textContent = stats.legacyFoldersRetained ?? 0;
  el.planExecuteBtn.disabled = !planCanExecute(plan) || state.executing;
  const cfg = plan.resourceCfgUpdate;
  const cfgSupported = supportsResourceCfgUpdate(state.folder);
  el.planCfgRow.classList.toggle("hidden", !cfgSupported);
  el.planCfgCheckbox.checked = cfgSupported && state.updateResourceCfg;
  el.planCfgCheckbox.disabled = !cfgSupported || state.planning || state.executing;
  el.planCfgSummary.textContent = cfg && state.updateResourceCfg
    ? `${cfg.path}\n${cfg.addedRules.length} ${t("resourceCfgNewRules")}\n${cfg.addedRules.join("\n") || t("resourceCfgAlreadyCovered")}`
    : t("resourceCfgNoUpdate");
  if (el.planPartialNotice) {
    const excluded = Number(stats.blocked || 0);
    el.planPartialNotice.textContent = excluded && Number(stats.ready || 0)
      ? t("organizePartialHint").replace("{ready}", String(stats.ready))
          .replace("{blocked}", String(excluded))
      : "";
    el.planPartialNotice.classList.toggle("hidden", !excluded || !Number(stats.ready || 0));
  }

  // Keep the rendered preview in memory while the plan object is unchanged.
  // Closing and reopening the modal must not rebuild thousands of DOM nodes.
  if (el.planModal.__renderedPlan === plan) return;
  el.planModal.__renderedPlan = plan;

  el.planItems.innerHTML = "";
  for (const item of plan.items || []) {
    const card = document.createElement("article");
    card.className = `plan-item plan-${item.planStatus}`;

    const top = document.createElement("div");
    top.className = "plan-item-top";
    const name = document.createElement("strong");
    name.textContent = item.name;
    const badge = document.createElement("span");
    badge.className = `plan-status plan-status-${item.planStatus}`;
    badge.textContent = planStatusLabel(item.planStatus);
    top.append(name, badge);

    const paths = document.createElement("div");
    paths.className = "plan-paths";
    for (const [label, path] of [
      [t("current"), item.sourceRelativePath],
      [t("proposed"), item.destinationRelativePath || t("noDestination")],
    ]) {
      const line = document.createElement("div");
      const span = document.createElement("span");
      span.textContent = label;
      const code = document.createElement("code");
      code.textContent = path;
      line.append(span, code);
      paths.appendChild(line);
    }

    card.append(top, paths);

    if (item.classificationReason) {
      const reason = document.createElement("div");
      reason.className = "plan-reason";
      const label = document.createElement("span");
      label.textContent = t("classificationReason");
      const code = document.createElement("code");
      code.textContent = item.classificationReason;
      reason.append(label, code);
      card.appendChild(reason);
    }

    if (item.warnings?.length) {
      const note = document.createElement("div");
      note.className = "plan-warning";
      note.textContent = item.warnings.join(" ");
      card.appendChild(note);
    }
    el.planItems.appendChild(card);
  }

  el.planDirectories.innerHTML = "";
  if (!plan.directoriesToCreate?.length) {
    const empty = document.createElement("div");
    empty.className = "planner-empty";
    empty.textContent = t("noFoldersNeeded");
    el.planDirectories.appendChild(empty);
  } else {
    for (const path of plan.directoriesToCreate) {
      const line = document.createElement("code");
      line.textContent = path;
      el.planDirectories.appendChild(line);
    }
  }

  el.planTree.innerHTML = "";
  for (const item of plan.items || []) {
    const row = document.createElement("div");
    row.className = "plan-tree-row";
    const before = document.createElement("code");
    before.textContent = item.sourceRelativePath || "—";
    const arrow = document.createElement("span");
    arrow.textContent = "→";
    const after = document.createElement("code");
    after.textContent = item.destinationRelativePath || "—";
    row.append(before, arrow, after);
    el.planTree.appendChild(row);
  }

  el.manifestPreviewText.textContent = plan.manifestPreview || "";
}

function renderRestore() {
  const plan = state.restorePlan;
  const hasManifest = !!state.restoreManifest;
  const canExecuteRestore = !!plan?.canExecute &&
    state.restorePreviewManifest === state.restoreManifest;

  el.restoreManifestPath.textContent = state.restoreManifest || t("noManifest");
  el.restoreManifestPath.title = state.restoreManifest;

  el.previewRestoreBtn.classList.toggle("hidden", !hasManifest);
  el.previewRestoreBtn.disabled = state.restoreBusy || state.structureBusy || state.executing;

  el.openManifestFolderBtn.classList.toggle("hidden", !hasManifest);
  el.openManifestFolderBtn.disabled = state.restoreBusy;

  const selectedHistory = state.restoreHistory.find(
    (entry) => entry.path === state.restoreManifest
  );
  el.removeRestoreHistoryBtn.disabled =
    !selectedHistory || (selectedHistory.valid && !selectedHistory.matchesSelectedRoot) ||
    state.restoreBusy || state.executing || state.structureBusy || workspaceReadOnly();

  el.executeRestoreBtn.classList.toggle("hidden", !canExecuteRestore);
  el.executeRestoreBtn.disabled =
    state.restoreBusy || state.structureBusy || workspaceReadOnly() ||
    state.restorePreviewManifest !== state.restoreManifest;

  if (plan) {
    el.restoreRootCheck.classList.remove("hidden", "match", "mismatch");
    el.restoreRootCheck.classList.add(plan.rootMatchesSelected ? "match" : "mismatch");
    el.restoreManifestRoot.textContent = plan.root || "—";
    el.restoreManifestRoot.title = plan.root || "";
    el.restoreSelectedRoot.textContent = plan.selectedRoot || state.folder || "—";
    el.restoreSelectedRoot.title = plan.selectedRoot || state.folder || "";
    el.restoreRootStatus.textContent = plan.rootMatchesSelected ? t("rootMatch") : t("rootMismatch");
  } else {
    el.restoreRootCheck.classList.add("hidden");
    el.restoreRootCheck.classList.remove("match", "mismatch");
  }

  const stats = plan?.stats || {};
  el.restoreStatTracked.textContent = stats.tracked ?? 0;
  el.restoreStatReady.textContent = stats.readyRestore ?? 0;
  el.restoreStatNew.textContent = stats.newFiles ?? 0;
  el.restoreStatCollisions.textContent = stats.collisions ?? 0;
  el.restoreStatBlocked.textContent = stats.blocked ?? 0;

  if (state.restoreBusy) {
    el.restoreState.textContent = state.pendingAction === "restore"
      ? t("restoring")
      : state.pendingAction === "remove_restore_history"
        ? t("removingRestoreRecord")
        : t("restorePreviewing");
    el.restoreState.className = "scan-state busy";
  } else if (state.restoreError) {
    el.restoreState.textContent = `${t("restoreFailed")}: ${state.restoreError}`;
    el.restoreState.className = "scan-state error";
  } else if (state.restoreNotice) {
    el.restoreState.textContent = state.restoreNotice;
    el.restoreState.className = "scan-state success";
  } else if (plan) {
    el.restoreState.textContent = t("restoreReady");
    el.restoreState.className = "scan-state success";
  } else if (hasManifest) {
    el.restoreState.textContent = t("restoreSelectedHint");
    el.restoreState.className = "scan-state";
  } else {
    el.restoreState.textContent = "";
    el.restoreState.className = "scan-state";
  }

  el.restoreState.dataset.fluentSeverity =
    !state.restoreBusy && !state.restoreError && plan &&
    (Number(stats.blocked || 0) > 0 || Number(stats.collisions || 0) > 0)
      ? "warning" : "";

  el.restoreEmpty.classList.toggle("hidden", !!plan);
  el.restoreResults.classList.toggle("hidden", !plan);
  el.restoreItems.innerHTML = "";

  if (!plan) return;

  for (const item of plan.items || []) {
    const card = document.createElement("article");
    card.className = `plan-item restore-${item.status}`;

    const top = document.createElement("div");
    top.className = "plan-item-top";
    const name = document.createElement("strong");
    name.textContent =
      item.sourceRelativePath ||
      item.destinationRelativePath ||
      item.sha256.slice(0, 16);
    const badge = document.createElement("span");
    badge.className = `plan-status restore-status-${item.status}`;
    badge.textContent = restoreStatusLabel(item.status);
    top.append(name, badge);

    const paths = document.createElement("div");
    paths.className = "plan-paths";
    if (item.sourceRelativePath) {
      const line = document.createElement("div");
      const label = document.createElement("span");
      label.textContent = t("current");
      const code = document.createElement("code");
      code.textContent = item.sourceRelativePath;
      line.append(label, code);
      paths.appendChild(line);
    }
    if (item.destinationRelativePath) {
      const line = document.createElement("div");
      const label = document.createElement("span");
      label.textContent = t("proposed");
      const code = document.createElement("code");
      code.textContent = item.destinationRelativePath;
      line.append(label, code);
      paths.appendChild(line);
    }

    card.append(top, paths);

    if (item.warnings?.length) {
      const note = document.createElement("div");
      note.className = "plan-warning";
      note.textContent = item.warnings.join(" ");
      card.appendChild(note);
    }

    el.restoreItems.appendChild(card);
  }
}

function openPlanModal() {
  renderPlan();
  el.planModal.classList.remove("hidden");
  el.planModal.setAttribute("aria-hidden", "false");
}

function closePlanModal() {
  el.planModal.classList.add("hidden");
  el.planModal.setAttribute("aria-hidden", "true");
}

function openConfirm(action) {
  if (action === "restore") {
    if (!state.restorePlan?.canExecute ||
        state.restorePreviewManifest !== state.restoreManifest ||
        state.restoreBusy || workspaceReadOnly()) return;
    state.pendingRestoreManifest = state.restoreManifest;
  } else if (action === "remove_restore_history") {
    const selected = state.restoreHistory.find(
      (item) => item.path === state.restoreManifest
    );
    if (!selected || (selected.valid && !selected.matchesSelectedRoot) ||
        state.restoreBusy || workspaceReadOnly()) return;
    state.pendingRemoveManifest = selected.path;
  } else if (action === "restore_quarantine") {
    if (!state.pendingQuarantineRestore || state.toolsBusy || workspaceReadOnly()) return;
  } else if (action === "remove_operation_history") {
    if (!historyRecordCanBeRemoved(state.pendingHistoryRemoval)) return;
  }

  state.pendingAction = action;
  if (action === "organize") {
    el.confirmTitle.textContent = t("confirmOrganizeTitle");
    const readyCount = state.plan?.stats?.ready || 0;
    const blockedCount = state.plan?.stats?.blocked || 0;
    el.confirmMessage.textContent = blockedCount
      ? `${t("confirmOrganizeMessage")}\n${t("organizePartialHint")
          .replace("{ready}", String(readyCount))
          .replace("{blocked}", String(blockedCount))}`
      : t("confirmOrganizeMessage");
    if (state.updateResourceCfg && state.plan?.resourceCfgUpdate) el.confirmMessage.textContent += `\n\n${t("resourceCfgConfirm")}\n${state.plan.resourceCfgUpdate.path}\n${state.plan.resourceCfgUpdate.addedRules.join("\n")}`;
    el.confirmActionBtn.textContent = t("organizeSelected");
  } else if (action === "clear_cache") {
    el.confirmTitle.textContent = t("confirmClearCacheTitle");
    el.confirmMessage.textContent = t("confirmClearCacheMessage");
    el.confirmActionBtn.textContent = t("clearCache");
  } else if (action === "remove_empty_folder") {
    el.confirmTitle.textContent = t("confirmRemoveEmptyTitle");
    el.confirmMessage.textContent = `${t("confirmRemoveEmptyMessage")}\n${state.pendingEmptyFolder}`;
    el.confirmActionBtn.textContent = t("removeEmptyFolderAction");
  } else if (action === "conflict_quarantine") {
    if (!state.conflictQuarantinePlan?.canExecute ||
        state.conflictQuarantinePlan.stats?.selected !== state.conflictQuarantineSelected.size ||
        !conflictSelectionSafe() || state.quarantineBusy || workspaceReadOnly()) {
      state.pendingAction = "";
      return;
    }
    el.confirmTitle.textContent = t("conflictQuarantineTitle");
    el.confirmMessage.textContent = t("conflictQuarantineMessage");
    el.confirmActionBtn.textContent = t("executeQuarantine");
  } else if (action === "quarantine") {
    el.confirmTitle.textContent = t("confirmQuarantineTitle");
    el.confirmMessage.textContent = t("confirmQuarantineMessage");
    el.confirmActionBtn.textContent = t("executeQuarantine");
  } else if (action === "restore") {
    el.confirmTitle.textContent = t("confirmRestoreTitle");
    el.confirmMessage.textContent = `${t("confirmRestoreMessage")}\n${state.restoreManifest}`;
    el.confirmActionBtn.textContent = t("executeRestore");
  } else if (action === "remove_restore_history") {
    el.confirmTitle.textContent = t("confirmRemoveRestoreTitle");
    el.confirmMessage.textContent = `${t("confirmRemoveRestoreMessage")}\n${state.pendingRemoveManifest}`;
    el.confirmActionBtn.textContent = t("removeRestoreManifest");
  } else if (action === "restore_quarantine") {
    el.confirmTitle.textContent = t("confirmQuarantineRestoreTitle");
    el.confirmMessage.textContent = `${t("confirmQuarantineRestoreMessage")}\n${state.pendingQuarantineRestore.destination}`;
    el.confirmActionBtn.textContent = t("restoreQuarantine");
  } else if (action === "remove_operation_history") {
    const item = state.pendingHistoryRemoval;
    el.confirmTitle.textContent = item.kind === "quarantine"
      ? t("confirmRemoveQuarantineHistoryTitle") : t("confirmRemoveRestoreTitle");
    el.confirmMessage.textContent = `${item.kind === "quarantine"
      ? t("confirmRemoveQuarantineHistoryMessage") : t("confirmRemoveRestoreMessage")}\n${item.destination}`;
    el.confirmActionBtn.textContent = t("removeRestoreManifest");
  } else {
    state.pendingAction = "";
    return;
  }
  el.confirmModal.classList.remove("hidden");
  el.confirmModal.setAttribute("aria-hidden", "false");
  el.confirmActionBtn.focus();
}

function closeConfirm() {
  if (state.executing || state.restoreBusy || state.quarantineBusy ||
      (state.toolsBusy && ["restore_quarantine", "remove_operation_history"].includes(state.pendingAction))) return;
  state.pendingAction = "";
  state.pendingRemoveManifest = "";
  state.pendingRestoreManifest = "";
  state.pendingQuarantineRestore = null;
  state.pendingHistoryRemoval = null;
  el.confirmModal.classList.add("hidden");
  el.confirmModal.setAttribute("aria-hidden", "true");
}

function render() {
  renderLanguage();
  renderTabs();
  if (state.tab === "organizer") renderStats();
  if (state.tab === "restore") {
    renderRestore();
    renderRestoreHistory();
  }
  if (state.tab === "duplicates") renderDuplicates();
  if (state.tab === "conflicts") renderConflicts();
  if (state.tab === "tools") renderTools();
  renderAllOperationProgress();
  renderCachePanel();
  renderDiagnostics();
  renderAuditPanel();
  renderStructure();
  renderOrganizationReview();

  el.folderPath.textContent = state.folder || t("noFolder");
  el.folderPath.title = state.folder;
  el.openOrganizedFolderBtn.disabled = !state.folder || state.executing;
  el.scanBtn.disabled =
    !state.folder ||
    analysisReadsBusy() ||
    state.auditBusy || state.structureBusy || state.toolsBusy ||
    state.planning || state.executing;
  const hasLoadedLibrary =
    state.items.length > 0 ||
    !!state.duplicatesAnalysis ||
    !!state.conflictsAnalysis;
  el.clearListBtn.classList.toggle("hidden", !hasLoadedLibrary);
  el.clearListBtn.disabled =
    state.scanning ||
    state.duplicatesBusy ||
    state.conflictsBusy ||
    state.structureBusy ||
    state.planning ||
    state.executing;

  el.chooseFolderBtn.disabled =
    state.scanning ||
    state.duplicatesBusy ||
    state.conflictsBusy ||
    state.structureBusy ||
    state.planning ||
    state.executing;

  if (state.executing) {
    el.scanState.textContent = t("executing");
    el.scanState.className = "scan-state busy";
  } else if (state.planError) {
    el.scanState.textContent = `${t("planFailed")}: ${state.planError}`;
    el.scanState.className = "scan-state error";
  } else if (state.scanning) {
    el.scanState.textContent = t("scanning");
    el.scanState.className = "scan-state busy";
  } else if (state.duplicatesBusy) {
    el.scanState.textContent = `${t("scanComplete")} · ${t("analyzingDuplicates")}`;
    el.scanState.className = "scan-state busy";
  } else if (state.conflictsBusy) {
    el.scanState.textContent = `${t("scanComplete")} · ${t("analyzingConflicts")}`;
    el.scanState.className = "scan-state busy";
  } else if (state.error) {
    el.scanState.textContent = `${t("scanFailed")}: ${state.error}`;
    el.scanState.className = "scan-state error";
  } else if (state.notice) {
    el.scanState.textContent = state.notice;
    el.scanState.className = "scan-state success";
  } else if (state.stats) {
    el.scanState.textContent = t("scanComplete");
    el.scanState.className = "scan-state success";
  } else {
    el.scanState.textContent = "";
    el.scanState.className = "scan-state";
  }

  if (state.tab === "organizer") renderResults(true);
  if (state.plan && !el.planModal.classList.contains("hidden")) renderPlan();
}

function clearDuplicateList() {
  if (state.analysisPipelineBusy) return;
  if (state.scanning || state.duplicatesBusy || state.conflictsBusy ||
      state.quarantineBusy || state.auditBusy) return;
  state.analysisStatus.duplicates = "cleared";
  closeDuplicateDetails();
  state.duplicatesAnalysis = null;
  state.duplicatesError = "";
  state.duplicatesNotice = "";
  state.duplicatesSearch = "";
  state.duplicatesFilter = "all";
  state.duplicateSelectedId = "";
  state.quarantineSelected.clear();
  state.quarantinePlan = null;
  state.quarantineExactBatch = false;
  state.conflictQuarantineSelected.clear();
  state.conflictQuarantinePlan = null;
  state.operations.duplicates = null;
  virtualViews.duplicates.items = null;
  el.duplicatesSearch.value = "";
  persistPreferences();
  render();
}

function clearConflictList() {
  if (state.analysisPipelineBusy) return;
  if (state.scanning || state.duplicatesBusy || state.conflictsBusy ||
      state.reviewBusy || state.auditBusy || state.quarantineBusy) return;
  state.analysisStatus.conflicts = "cleared";
  state.conflictQuarantineSelected.clear();
  state.conflictQuarantinePlan = null;
  state.conflictReviewSelected.clear();
  state.conflictsSelectedOnly = false;
  conflictVisibleMemo.analysis = null;
  closeConflictDetails();
  state.conflictsAnalysis = null;
  state.conflictsError = "";
  state.conflictsNotice = "";
  state.conflictsSearch = "";
  state.conflictsFilter = "attention";
  state.conflictSelectedId = "";
  state.conflictMarks = {};
  state.operations.conflicts = null;
  virtualViews.conflicts.items = null;
  el.conflictsSearch.value = "";
  persistPreferences();
  render();
}

function clearLoadedLibrary() {
  if (
    state.scanning ||
    state.planning ||
    state.executing ||
    state.duplicatesBusy ||
    state.conflictsBusy ||
    state.restoreBusy ||
    state.structureBusy
  ) return;

  state.items = [];
  state.stats = null;
  state.selectedId = "";
  state.selectedForPlan.clear();
  state.search = "";
  state.status = "all";
  state.plan = null;
  state.planError = "";
  state.notice = "";
  state.error = "";
  state.organizationReview = null;
  state.organizationCollisionItems = [];

  state.duplicatesAnalysis = null;
  state.duplicatesError = "";
  state.duplicatesNotice = "";
  state.duplicatesSearch = "";
  state.duplicatesFilter = "all";
  state.duplicateSelectedId = "";

  state.conflictsAnalysis = null;
  state.conflictsError = "";
  state.conflictsNotice = "";
  state.conflictsSearch = "";
  state.conflictsFilter = "attention";
  state.conflictSelectedId = "";
  state.conflictMarks = {};

  state.quarantineSelected.clear();
  state.quarantinePlan = null;
  state.quarantineExactBatch = false;
  state.conflictQuarantineSelected.clear();
  state.conflictQuarantinePlan = null;
  state.conflictReviewSelected.clear();
  state.conflictsSelectedOnly = false;
  conflictVisibleMemo.analysis = null;
  state.packagePreviews = {};
  state.packagePreviewLoading = {};
  state.packagePreviewErrors = {};
  state.technicalDetails = {};
  state.technicalDetailsErrors = {};
  state.technicalDetailsOpen.clear();

  state.operations = { scan: null, duplicates: null, conflicts: null };
  closePlanModal();
  closeDuplicateDetails();
  closeConflictDetails();
  closeCollisionReview();
  persistPreferences();
  render();
}

async function chooseFolder() {
  if (state.scanning || state.analysisPipelineBusy || state.duplicatesBusy ||
      state.conflictsBusy || state.quarantineBusy || state.executing ||
      state.planning || state.restoreBusy || state.structureBusy || state.auditBusy) return;
  const selected = await open({
    directory: true,
    multiple: false,
    title: t("chooseModsFolder"),
  });
  // The chooser is asynchronous: a scan or quarantine may have started
  // while its native window was open. Do not mix in-flight folder sessions.
  if (!selected || Array.isArray(selected) ||
      state.scanning || state.analysisPipelineBusy || state.duplicatesBusy ||
      state.conflictsBusy || state.quarantineBusy || state.executing ||
      state.planning || state.restoreBusy || state.structureBusy || state.auditBusy) return;
  state.analysisRunId += 1;
  state.analysisStatus = { manager: "not_run", duplicates: "not_run", conflicts: "not_run" };
  state.folder = selected;
  state.lastOrganizedDirectory = "";
  state.postQuarantineNotice = null;
  state.lastExactGroupAnchor = null;
  state.lastConflictReviewAnchor = null;
  state.conflictReviewSelected.clear();
  state.conflictsSelectedOnly = false;
  conflictVisibleMemo.analysis = null;
  state.updateResourceCfg = false;
  persistPreferences();
  window.dispatchEvent(new CustomEvent("s3cc-folder-changed", { detail: state.folder }));
  state.items = [];
  state.stats = null;
  state.selectedId = "";
  state.selectedForPlan.clear();
  state.error = "";
  state.planError = "";
  state.notice = "";
  state.plan = null;
  state.organizationReview = null;
  state.organizationCollisionItems = [];
  state.duplicatesAnalysis = null;
  state.duplicatesError = "";
  state.duplicatesNotice = "";
  state.duplicateSelectedId = "";
  state.conflictsAnalysis = null;
  state.conflictsError = "";
  state.conflictsNotice = "";
  state.conflictSelectedId = "";
  state.conflictMarks = {};
  applyPersistentDecisionRecords([]);
  state.quarantineSelected.clear();
  state.quarantinePlan = null;
  state.quarantineExactBatch = false;
  state.conflictQuarantineSelected.clear();
  state.conflictQuarantinePlan = null;
  state.technicalDetails = {};
  state.technicalDetailsLoading = "";
  state.technicalDetailsErrors = {};
  state.restoreHistory = [];
  state.auditErrors = { organizer: "", duplicates: "", conflicts: "" };
  state.auditReports = { organizer: null, duplicates: null, conflicts: null };
  state.structureListing = null;
  state.structureCurrent = "";
  state.structureSelectedPath = "";
  state.structureDirectories = [];
  state.structureError = "";
  state.structureNotice = "";
  state.manualOperations = [];
  closePlanModal();
  render();
  await Promise.all([
    loadRestoreHistory(),
    refreshCacheInfo(),
    refreshConflictDecisions(),
    refreshManualOperations(),
    ...(state.tab === "structure" ? [loadStructure("")] : []),
    refreshToolsContext(),
  ]);
}

async function scanFolder(preserveSelection = false, preserveNotice = false) {
  if (!state.folder || analysisReadsBusy() || state.auditBusy ||
      state.planning || state.executing || state.restoreBusy ||
      state.quarantineBusy || state.structureBusy || state.toolsBusy) return;

  const folder = state.folder;
  const runId = ++state.analysisRunId;
  state.analysisRefreshQueued = false;
  const oldStatus = { ...state.analysisStatus };
  const oldDuplicates = state.duplicatesAnalysis;
  const oldConflicts = state.conflictsAnalysis;
  const previousSelection = new Set(state.selectedForPlan);
  const previousNotice = state.notice;
  const previousItems = state.items;
  const previousStats = state.stats;
  const previousSelectedId = state.selectedId;

  state.scanning = true;
  state.analysisStatus.manager = "running";
  state.operations.scan = null;
  const operationToken = beginOperationMonitor("scan");
  void monitorOperation("scan", operationToken);
  state.error = "";
  state.planError = "";
  state.plan = null;
  if (!preserveNotice) {
    state.notice = "";
    state.organizationReview = null;
    state.organizationCollisionItems = [];
  }
  closePlanModal();
  render();

  let scanSucceeded = false;
  try {
    const result = await invoke("scan_packages", { folder, language: state.language });
    if (!analysisSessionMatches(folder, runId)) return;
    state.items = result.items || [];
    state.stats = result.stats || null;
    virtualViews.manager.items = null;

    const eligibleIds = new Set(
      state.items.filter(eligibleForPlan).map((item) => item.id)
    );

    state.selectedForPlan.clear();
    if (preserveSelection) {
      for (const id of previousSelection) {
        if (eligibleIds.has(id)) state.selectedForPlan.add(id);
      }
    } else {
      for (const id of eligibleIds) state.selectedForPlan.add(id);
    }

    state.selectedId = visibleItems()[0]?.id || "";
    if (preserveNotice) state.notice = previousNotice;
    state.analysisStatus.manager = "completed";
    // A cache-hit scan has already validated the package set, language and
    // classification context. Keep matching duplicate/conflict results so a
    // second click does not immediately reopen every package; a fresh scan
    // still invalidates both analyses exactly as before.
    const reusedScan = result.cacheHit === true;
    const keepDuplicates = reusedScan && !!oldDuplicates;
    const keepConflicts = reusedScan && !!oldConflicts;
    state.duplicatesAnalysis = keepDuplicates ? oldDuplicates : null;
    state.conflictsAnalysis = keepConflicts ? oldConflicts : null;
    state.conflictReviewSelected.clear();
    state.conflictsSelectedOnly = false;
    conflictVisibleMemo.analysis = null;
    state.duplicatesError = "";
    state.conflictsError = "";
    state.duplicatesNotice = "";
    state.conflictsNotice = "";
    state.duplicateSelectedId = "";
    state.conflictSelectedId = "";
    state.quarantineSelected.clear();
    state.quarantinePlan = null;
    closeDuplicateDetails();
    closeConflictDetails();
    state.analysisStatus.duplicates = keepDuplicates ? "completed" : "queued";
    state.analysisStatus.conflicts = keepConflicts ? "completed" : "queued";
    scanSucceeded = true;
  } catch (error) {
    const message = String(error);
    if (message.includes("__S3CC_OPERATION_CANCELLED__")) {
      state.notice = t("cancelled");
      state.error = "";
      state.items = previousItems;
      state.stats = previousStats;
      state.selectedId = previousSelectedId;
      state.selectedForPlan = new Set(previousSelection);
      state.analysisStatus = oldStatus;
      state.duplicatesAnalysis = oldDuplicates;
      state.conflictsAnalysis = oldConflicts;
    } else {
      state.error = message;
      state.analysisStatus.manager = "failed";
      state.analysisStatus.duplicates = "not_run";
      state.analysisStatus.conflicts = "not_run";
      state.duplicatesAnalysis = null;
      state.conflictsAnalysis = null;
      state.items = [];
      state.stats = null;
      state.selectedForPlan.clear();
    }
  } finally {
    await settleOperationStatus("scan", operationToken);
    state.scanning = false;
    endOperationMonitor("scan", operationToken);
    render();
  }
  if (scanSucceeded && analysisSessionMatches(folder, runId)) {
    void runAutomaticAnalyses(folder, runId);
  }
}

// Both analyses persist the same DBPF fingerprint cache. Never run them in parallel.
async function runAutomaticAnalyses(folder, runId) {
  if (!analysisSessionMatches(folder, runId) || state.analysisPipelineBusy) return;
  state.analysisPipelineBusy = true;
  try {
    if (state.analysisStatus.duplicates === "queued") {
      await analyzeDuplicates({ automated: true, folder, runId });
    }
    if (analysisSessionMatches(folder, runId) &&
        state.analysisStatus.conflicts === "queued") {
      await analyzeConflicts({ automated: true, folder, runId });
    }
  } finally {
    // Release the lock even if moving packages invalidated this run.
    // Previously a stale run could leave analysisPipelineBusy locked forever.
    state.analysisPipelineBusy = false;
    if (analysisSessionMatches(folder, runId) && state.postQuarantineNotice) {
      const { tab, text } = state.postQuarantineNotice;
      state.tab = tab;
      if (tab === "duplicates") state.duplicatesNotice = text;
      else if (tab === "conflicts") state.conflictsNotice = text;
      state.postQuarantineNotice = null;
    }
    if (state.analysisRefreshQueued && state.folder) {
      state.analysisRefreshQueued = false;
      const currentFolder = state.folder, currentRun = state.analysisRunId;
      // Rebuild Duplicates and Conflicts after the verified file moves,
      // never using stale quarantine paths from the old analysis.
      void runAutomaticAnalyses(currentFolder, currentRun);
    }
    render();
  }
}

async function buildPlan() {
  if (
    !state.folder ||
    !state.selectedForPlan.size ||
    state.planning ||
    state.structureBusy
  ) return;

  // If nothing changed since the last preview, reopening is instantaneous.
  if (state.plan && !state.planError) {
    openPlanModal();
    return;
  }

  state.planning = true;
  state.planError = "";
  state.notice = "";
  if (!supportsResourceCfgUpdate(state.folder)) state.updateResourceCfg = false;
  render();

  try {
    state.plan = await invoke("build_organization_plan_with_cfg", {
      folder: state.folder,
      language: state.language,
      selectedPaths: [...state.selectedForPlan],
      updateResourceCfg: state.updateResourceCfg,
    });
  } catch (error) {
    state.plan = null;
    state.planError = String(error);
  } finally {
    state.planning = false;
    render();
    if (state.plan) openPlanModal();
  }
}

// Rust returns the verified source/destination pair for every completed
// rename. Use that transaction record instead of a complete DBPF rescan,
// which previously re-read the entire library and started both analyzers.
function reconcileSuccessfulOrganization(result) {
  const moved = result.movedFiles || [];
  if (!Array.isArray(moved) || !moved.length) return;

  const key = path => String(path || "").replaceAll("\\", "/").toLocaleLowerCase();
  const bySource = new Map(moved.map(move => [key(move.sourcePath), move]));
  const priorSelection = state.selectedId;
  const selectedMove = bySource.get(key(priorSelection));
  state.items = state.items.map(item => {
    const move = bySource.get(key(item.path));
    if (!move) return item;
    return {
      ...item,
      id: move.destinationPath,
      path: move.destinationPath,
      relativePath: move.destinationRelativePath,
    };
  });
  if (selectedMove) state.selectedId = selectedMove.destinationPath;
  state.selectedForPlan.clear();
  state.plan = null;
  state.planError = "";
  virtualViews.manager.items = null;
  managerVisibleMemo.items = null;

  // Pre-move absolute paths must never leak into previews or future actions.
  state.packagePreviews = {};
  state.packagePreviewLoading = {};
  state.packagePreviewErrors = {};
  state.technicalDetails = {};
  state.technicalDetailsLoading = "";
  state.technicalDetailsErrors = {};
  state.technicalDetailsOpen.clear();

  // The old conflict/duplicate paths are no longer safe for quarantine.
  // Keep these analyzers available, but do not automatically run them.
  state.analysisRunId += 1;
  state.duplicatesAnalysis = null;
  state.conflictsAnalysis = null;
  state.analysisStatus.manager = "completed";
  state.analysisStatus.duplicates = "not_run";
  state.analysisStatus.conflicts = "not_run";
  state.duplicatesError = "";
  state.conflictsError = "";
  state.duplicatesNotice = t("analysisNeedsRefresh");
  state.conflictsNotice = t("analysisNeedsRefresh");
  state.duplicateSelectedId = "";
  state.conflictSelectedId = "";
  state.quarantineSelected.clear();
  state.quarantinePlan = null;
  state.conflictQuarantineSelected.clear();
  state.conflictQuarantinePlan = null;
  state.conflictReviewSelected.clear();
  state.conflictsSelectedOnly = false;
  closeDuplicateDetails();
  closeConflictDetails();
  conflictVisibleMemo.analysis = null;
  virtualViews.duplicates.items = null;
  virtualViews.conflicts.items = null;
}

async function executeOrganization() {
  if (!planCanExecute(state.plan) || state.executing || state.structureBusy) return;

  const completedPlan = state.plan;
  let refreshAnalyses = false;
  const completedStats = completedPlan?.stats || {};
  const collisionItems = (completedPlan?.items || []).filter((item) =>
    String(item.planStatus || "").includes("collision")
  );

  state.organizationReview = null;
  state.organizationCollisionItems = [];
  closeCollisionReview();

  state.executing = true;
  state.organizeProgress = {
    running: true, phase: "planning", processed: 0, total: 0,
  };
  state.planError = "";
  // The confirmation dialog must not obscure live progress. After confirmation
  // the operation continues transactionally with its rollback protection.
  el.confirmModal.classList.add("hidden");
  el.confirmModal.setAttribute("aria-hidden", "true");
  closePlanModal();
  el.confirmActionBtn.disabled = true;
  render();
  void monitorOrganizationProgress();

  try {
    const result = await invoke("execute_organization_plan_with_cfg", {
      plan: completedPlan,
      language: state.language,
      updateResourceCfg: state.updateResourceCfg,
      expectedCfgHash: completedPlan.resourceCfgUpdate?.originalHash ?? null,
      expectedCfgRules: completedPlan.resourceCfgUpdate?.addedRules ?? [],
    });

    if (result.status === "COMPLETE" || result.status === "NO_CHANGES") {
      state.lastOrganizedDirectory = result.organizedDirectory || state.folder;
      if (result.status === "COMPLETE") {
        reconcileSuccessfulOrganization(result);
        refreshAnalyses = (result.moved ?? 0) > 0;
      }
      state.organizationReview = {
        moved: result.moved ?? 0,
        duplicates: completedStats.duplicateSkipped ?? 0,
        collisions:
          (completedStats.collisionSameContent ?? 0) +
          (completedStats.collisionDifferentContent ?? 0),
        oldFoldersRemoved: result.oldFoldersRemoved ?? 0,
        oldFoldersRetained: result.oldFoldersRetained ?? 0,
        remainingLegacyFiles: result.remainingLegacyFiles ?? 0,
        remainingLegacyExamples: result.remainingLegacyExamples ?? [],
        remainingLegacyDetails: result.remainingLegacyDetails ?? [],
        analysisBefore: result.analysisBefore ?? completedPlan.analysisBefore ?? null,
        execution: result.execution ?? null,
        analysisCurrent: result.analysisCurrent ?? null,
      };
      state.organizationCollisionItems = collisionItems;

      state.notice = result.status === "COMPLETE"
        ? `${t("executionComplete")}: ${result.moved}`
        : t("executionNoChanges");
      // File moves have succeeded, but a permission problem may have kept an
      // empty folder. Surface cleanup warnings instead of silently hiding it.
      if (result.errors?.length) {
        state.notice += `\n${result.errors.join("\n")}`;
      }
    } else {
      state.planError = `${t("executionRolledBack")}: ${(result.errors || []).join(" ")}`;
    }

    state.pendingAction = "";
    el.confirmModal.classList.add("hidden");
    closePlanModal();
  } catch (error) {
    state.planError = String(error);
  } finally {
    state.executing = false;
    state.organizeProgress = null;
    el.confirmActionBtn.disabled = false;
    render();
  }

  if (!state.planError) {
    await loadRestoreHistory();
    if (refreshAnalyses && state.folder) {
      state.analysisStatus.duplicates = "queued";
      state.analysisStatus.conflicts = "queued";
      state.analysisRefreshQueued = true;
      if (!state.analysisPipelineBusy) {
        state.analysisRefreshQueued = false;
        void runAutomaticAnalyses(state.folder, state.analysisRunId);
      }
      render();
    }
  }
}

async function chooseManifest() {
  const selected = await open({
    multiple: false,
    directory: false,
    title: t("chooseManifest"),
    filters: [{ name: "S3CC Manager", extensions: ["txt"] }],
  });
  if (!selected || Array.isArray(selected)) return;

  state.restoreManifest = selected;
  state.restorePlan = null;
  state.restorePreviewManifest = "";
  state.restoreError = "";
  state.restoreNotice = "";
  render();
}

async function previewRestore() {
  if (!state.restoreManifest || state.restoreBusy || state.structureBusy) return;

  state.restoreBusy = true;
  state.restoreError = "";
  state.restoreNotice = "";
  state.restorePlan = null;
  state.restorePreviewManifest = "";
  render();

  try {
    const selectedManifest = state.restoreManifest;
    const selectedFolder = state.folder;
    const plan = await invoke("preview_restore", {
      manifestPath: selectedManifest,
      currentLanguage: state.language,
      expectedRoot: selectedFolder || null,
    });
    if (state.restoreManifest === selectedManifest && state.folder === selectedFolder) {
      state.restorePlan = plan;
      state.restorePreviewManifest = selectedManifest;
    }
  } catch (error) {
    state.restoreError = String(error);
  } finally {
    state.restoreBusy = false;
    render();
  }
}

async function executeRestore() {
  // Restoring is impossible from selection or preview, without an explicit
  // confirmation for the exact manifest that was previewed.
  if (state.pendingAction !== "restore" ||
      state.pendingRestoreManifest !== state.restoreManifest ||
      state.restorePreviewManifest !== state.restoreManifest ||
      !state.restorePlan?.canExecute || state.restoreBusy ||
      state.structureBusy || workspaceReadOnly() ||
      el.confirmModal.classList.contains("hidden")) return;

  state.restoreBusy = true;
  state.restoreOperationActive = true;
  state.restoreOperationStatus = null;
  const operationToken = ++restoreOperationSequence;
  state.restoreError = "";
  el.confirmActionBtn.disabled = true;
  void monitorRestoreProgress(operationToken);
  render();

  try {
    const result = await invoke("execute_restore", {
      manifestPath: state.restoreManifest,
      currentLanguage: state.language,
      expectedRoot: state.folder || null,
      confirmed: true,
    });

    if (result.status === "RESTORED") {
      state.restoreNotice =
        `${t("restoreComplete")}: ${result.restored} + ${result.newFilesRelocated} ${t("newFiles")}`;
      if (result.warnings?.length) {
        state.restoreNotice += `\n${result.warnings.join("\n")}`;
      }
    } else {
      state.restoreError =
        `${t("executionRolledBack")}: ${(result.errors || []).join(" ")}`;
    }

    state.pendingAction = "";
    state.pendingRestoreManifest = "";
    el.confirmModal.classList.add("hidden");
    el.confirmModal.setAttribute("aria-hidden", "true");
  } catch (error) {
    state.restoreError = String(error);
  } finally {
    state.restoreBusy = false;
    state.restoreOperationActive = false;
    restoreOperationSequence += 1;
    state.pendingAction = "";
    state.pendingRestoreManifest = "";
    el.confirmModal.classList.add("hidden");
    el.confirmModal.setAttribute("aria-hidden", "true");
    el.confirmActionBtn.disabled = false;
    render();
  }

  if (!state.restoreError) {
    await previewRestore();
    if (state.restorePlan) state.restoreNotice = t("restoreComplete");
    await loadRestoreHistory();
    render();
  }
}

async function analyzeDuplicates({ automated = false, folder = state.folder, runId = state.analysisRunId } = {}) {
  if (!folder || !analysisSessionMatches(folder, runId) ||
      state.duplicatesBusy || state.conflictsBusy || state.scanning ||
      (!automated && state.analysisPipelineBusy) ||
      state.auditBusy || state.structureBusy || state.toolsBusy) return;
  const previousAnalysis = state.duplicatesAnalysis;
  const previousSelectedId = state.duplicateSelectedId;
  state.duplicatesBusy = true;
  state.analysisStatus.duplicates = "running";
  state.duplicatesNotice = "";
  state.duplicatesError = "";
  state.duplicatesAnalysis = null;
  state.duplicateSelectedId = "";
  state.operations.duplicates = null;
  const operationToken = beginOperationMonitor("duplicates");
  void monitorOperation("duplicates", operationToken);
  render();
  try {
    const result = await invoke("analyze_duplicates", { folder });
    if (!analysisSessionMatches(folder, runId)) return;
    state.duplicatesAnalysis = result;
    state.analysisStatus.duplicates = "completed";
    state.quarantineSelected.clear();
    state.quarantinePlan = null;
    state.duplicateSelectedId = flattenedDuplicateFindings()[0]?.id || "";
  } catch (e) {
    if (!analysisSessionMatches(folder, runId)) return;
    const message = String(e);
    state.duplicatesAnalysis = previousAnalysis;
    state.duplicateSelectedId = previousSelectedId;
    if (message.includes("__S3CC_OPERATION_CANCELLED__")) {
      state.duplicatesNotice = t("cancelled");
      state.analysisStatus.duplicates = "cancelled";
    } else {
      state.duplicatesError = message;
      state.analysisStatus.duplicates = "failed";
    }
  } finally {
    if (analysisSessionMatches(folder, runId)) {
      await settleOperationStatus("duplicates", operationToken);
    }
    if (operationMonitorIsCurrent("duplicates", operationToken)) {
      state.duplicatesBusy = false;
      endOperationMonitor("duplicates", operationToken);
    }
    if (analysisSessionMatches(folder, runId)) {
      render();
      await refreshCacheInfo();
    }
  }
}

async function analyzeConflicts({ automated = false, folder = state.folder, runId = state.analysisRunId } = {}) {
  if (!folder || !analysisSessionMatches(folder, runId) ||
      state.conflictsBusy || state.duplicatesBusy || state.scanning ||
      (!automated && state.analysisPipelineBusy) ||
      state.auditBusy || state.structureBusy || state.toolsBusy) return;
  const previousAnalysis = state.conflictsAnalysis;
  const previousSelectedId = state.conflictSelectedId;
  state.conflictsBusy = true;
  state.conflictQuarantineSelected.clear();
  state.conflictQuarantinePlan = null;
  state.conflictReviewSelected.clear();
  state.conflictsSelectedOnly = false;
  conflictVisibleMemo.analysis = null;
  state.analysisStatus.conflicts = "running";
  state.conflictsNotice = "";
  state.conflictsError = "";
  state.conflictsAnalysis = null;
  state.conflictSelectedId = "";
  state.operations.conflicts = null;
  const operationToken = beginOperationMonitor("conflicts");
  void monitorOperation("conflicts", operationToken);
  render();
  try {
    const result = await invoke("analyze_conflicts", { folder });
    if (!analysisSessionMatches(folder, runId)) return;
    state.conflictsAnalysis = result;
    state.analysisStatus.conflicts = "completed";
    state.conflictMarks = {};
    state.conflictSelectedId = result?.findings?.[0]?.id || "";
  } catch (e) {
    if (!analysisSessionMatches(folder, runId)) return;
    const message = String(e);
    state.conflictsAnalysis = previousAnalysis;
    state.conflictSelectedId = previousSelectedId;
    if (message.includes("__S3CC_OPERATION_CANCELLED__")) {
      state.conflictsNotice = t("cancelled");
      state.analysisStatus.conflicts = "cancelled";
    } else {
      state.conflictsError = message;
      state.analysisStatus.conflicts = "failed";
    }
  } finally {
    if (analysisSessionMatches(folder, runId)) {
      await settleOperationStatus("conflicts", operationToken);
    }
    if (operationMonitorIsCurrent("conflicts", operationToken)) {
      state.conflictsBusy = false;
      endOperationMonitor("conflicts", operationToken);
    }
    if (analysisSessionMatches(folder, runId)) {
      render();
      await refreshCacheInfo();
    }
  }
}

function selectAllVisible() {
  for (const item of visibleItems()) {
    if (eligibleForPlan(item)) state.selectedForPlan.add(item.id);
  }
  state.plan = null;
  state.planError = "";
  renderResults(true);
}

function selectNoneVisible() {
  for (const item of visibleItems()) {
    state.selectedForPlan.delete(item.id);
  }
  state.plan = null;
  state.planError = "";
  renderResults(true);
}

for (const button of el.tabs) {
  button.addEventListener("click", () => {
    state.tab = button.dataset.tab || "organizer";
    persistPreferences();
    render();
    if (state.tab === "restore" && state.folder) void loadRestoreHistory();
    if (state.tab === "structure" && state.folder && !state.structureListing) {
      void loadStructure(state.structureCurrent || "");
    }
    if (state.tab === "tools" && state.folder) {
      void refreshToolsContext();
    }
  });
}

el.languageButton.addEventListener("click", (event) => {
  event.stopPropagation();
  state.isStatusFilterOpen = false;
  el.languageMenu?.classList.toggle("hidden");
  renderLanguage();
});

for (const button of el.languageMenuItems) {
  button.addEventListener("click", async (event) => {
    event.stopPropagation();
    const nextLanguage = button.dataset.lang;
    if (!LANGUAGE_ORDER.includes(nextLanguage)) return;

    el.languageMenu?.classList.add("hidden");
    state.isStatusFilterOpen = false;
    if (nextLanguage === state.language) {
      renderLanguage();
      return;
    }

    state.language = nextLanguage;
    localStorage.setItem("s3cc-organizer-language", state.language);
    persistPreferences();
    render();

    if (state.folder && state.stats) await scanFolder(true);
    if (state.restoreManifest && state.restorePlan) await previewRestore();
  });
}

document.addEventListener("click", (event) => {
  let changed = false;

  if (!event.target.closest("#lang-dropdown") && !el.languageMenu?.classList.contains("hidden")) {
    el.languageMenu?.classList.add("hidden");
    changed = true;
  }

  if (
    state.isStatusFilterOpen &&
    !el.statusFilterDropdown?.contains(event.target)
  ) {
    state.isStatusFilterOpen = false;
    changed = true;
  }

  if (changed) {
    renderLanguage();
    renderStatusFilter();
  }
});

document.addEventListener("keydown", (event) => {
  if (event.key !== "Escape") return;
  if (!el.duplicateDetailsModal.classList.contains("hidden")) {
    closeDuplicateDetails();
    return;
  }
  if (!el.conflictDetailsModal.classList.contains("hidden")) {
    closeConflictDetails();
    return;
  }
  if (!el.collisionReviewModal.classList.contains("hidden")) {
    closeCollisionReview();
    return;
  }
  if (state.isStatusFilterOpen) {
    state.isStatusFilterOpen = false;
    renderStatusFilter();
    return;
  }
  if (!el.structureModal.classList.contains("hidden")) closeStructureModal();
  else if (!el.confirmModal.classList.contains("hidden")) closeConfirm();
  else if (!el.planModal.classList.contains("hidden")) closePlanModal();
});


el.chooseFolderBtn.addEventListener("click", chooseFolder);
el.scanCancelBtn.addEventListener("click", () => cancelAnalysis("scan"));
el.duplicatesCancelBtn.addEventListener("click", () => cancelAnalysis("duplicates"));
el.conflictsCancelBtn.addEventListener("click", () => cancelAnalysis("conflicts"));
el.loadingOverlayCancel.addEventListener("click", () => {
  const current = loadingOverlayState();
  if (current?.cancelable) void cancelLoadingOperation(current.kind);
});
el.loadingOverlayContinue?.addEventListener("click", continueLoadingInBackground);
window.addEventListener("s3cc-operation-progress", event => {
  const detail = event.detail || {};
  if (!detail.kind) return;
  const operationId = String(detail.operationId || "");
  const currentId = String(state.externalOperation?.operationId || "");
  if (detail.active) {
    // A converter poll from a completed run must not resurrect its overlay.
    if (operationId && completedExternalOperationIds.has(operationId)) return;
    if (operationId && currentId && operationId !== currentId) return;
    if (detail.status && detail.status.running === false) {
      if (operationId) completedExternalOperationIds.add(operationId);
      if (!operationId || !currentId || operationId === currentId) {
        state.externalOperation = null;
        renderLoadingOverlay();
      }
      return;
    }
    state.externalOperation = {
      kind: detail.kind,
      operationId,
      status: detail.status || {},
      active: true,
    };
  } else {
    if (operationId) completedExternalOperationIds.add(operationId);
    if (state.externalOperation &&
        operationId && currentId && operationId !== currentId) return;
    state.externalOperation = null;
  }
  renderLoadingOverlay();
});
el.refreshRestoreHistoryBtn.addEventListener("click", loadRestoreHistory);
el.removeRestoreHistoryBtn.addEventListener("click", () => openConfirm("remove_restore_history"));
el.openManifestFolderBtn.addEventListener("click", () => openDirectorySafe(state.restoreManifest));
el.openCacheBtn.addEventListener("click", () => openDirectorySafe(state.cacheInfo?.path));
el.clearCacheBtn.addEventListener("click", () => openConfirm("clear_cache"));
el.rebuildFixIndexBtn.addEventListener("click", rebuildFixIndex);
for (const button of el.exportAuditButtons) {
  button.addEventListener("click", () => exportAuditReport(button.dataset.exportAudit));
}
for (const button of el.openReportFolderButtons) {
  button.addEventListener("click", () =>
    openDirectorySafe(state.auditReports[button.dataset.openReportFolder]?.directory)
  );
}
el.duplicatesClearListBtn.addEventListener("click", clearDuplicateList);
el.duplicatesSelectExactBtn.addEventListener("click", selectExactCopiesForQuarantine);
el.duplicatesSelectCurrentBtn.addEventListener("click", selectOnlyCurrentExactDuplicateGroup);
el.duplicatesSelectVisibleBtn.addEventListener("click", selectFilteredExactDuplicateGroups);
el.duplicatesList.addEventListener("keydown", event => {
  if ((event.ctrlKey || event.metaKey) && !event.altKey && event.key.toLowerCase() === "a") {
    event.preventDefault();
    selectFilteredExactDuplicateGroups();
  }
});
el.duplicatesPreviewAllBtn.addEventListener("click", buildQuarantinePreview);
el.duplicatesClearSelectionBtn.addEventListener("click", () => {
  state.quarantineSelected.clear();
  state.quarantinePlan = null;
  state.quarantineExactBatch = false;
  renderDuplicates();
  renderDuplicatesPreview();
});
el.conflictsClearListBtn.addEventListener("click", clearConflictList);
el.conflictsSelectAllBtn.addEventListener("click", selectAllVisibleConflicts);
el.conflictsSelectDetectedBtn.addEventListener("click", selectAllDetectedConflicts);
el.conflictsReviewSelectedBtn.addEventListener("click", reviewSelectedConflicts);
el.conflictsClearReviewBtn.addEventListener("click", clearConflictReviewSelection);
el.conflictsPrioritizeSelectedBtn.addEventListener("click", prioritizeSelectedConflicts);
el.conflictsMarkIntentionalBtn.addEventListener("click", () => applySelectedConflictReviewMark("intentional"));
el.conflictsMarkReviewedBtn.addEventListener("click", () => applySelectedConflictReviewMark("reviewed"));
el.conflictsMarkIgnoredBtn.addEventListener("click", () => applySelectedConflictReviewMark("ignored"));
el.conflictsClearMarksBtn.addEventListener("click", () => applySelectedConflictReviewMark("clear"));
el.conflictsList.addEventListener("keydown", event => {
  if ((event.ctrlKey || event.metaKey) && !event.altKey && event.key.toLowerCase() === "a") {
    event.preventDefault();
    selectAllVisibleConflicts();
  }
});
el.conflictsPrioritizeBtn.addEventListener("click", () => {
  state.conflictsSelectedOnly = false;
  conflictVisibleMemo.analysis = null;
  state.conflictsFilter = "attention";
  el.conflictsFilter.value = "attention";
  persistPreferences();
  renderConflicts();
});
el.conflictsPreviewQuarantineBtn.addEventListener("click", buildConflictQuarantinePreview);
el.conflictsClearQuarantineBtn.addEventListener("click", () => {
  state.conflictQuarantineSelected.clear();
  state.conflictQuarantinePlan = null;
  renderConflicts();
});
el.conflictPreviousBtn.addEventListener("click", () => navigateConflictDetails(-1));
el.conflictNextBtn.addEventListener("click", () => navigateConflictDetails(1));
el.structureUpBtn.addEventListener("click", () => {
  const parent = state.structureListing?.parentRelativePath;
  if (parent != null) loadStructure(parent);
});
el.structureRefreshBtn.addEventListener("click", () =>
  loadStructure(state.structureCurrent || "")
);
el.structureCreateBtn.addEventListener("click", () => openStructureModal("create"));
el.structureMoveBtn.addEventListener("click", () => openStructureModal("move"));
el.structureRenameBtn.addEventListener("click", () => openStructureModal("rename"));
el.structureUndoBtn.addEventListener("click", undoManualOperation);
el.structureModalCancelBtn.addEventListener("click", closeStructureModal);
el.structureModalActionBtn.addEventListener("click", executeStructureAction);
el.structureModal.addEventListener("click", (event) => {
  if (event.target === el.structureModal) closeStructureModal();
});
for (const button of el.toolsSubtabs) {
  button.addEventListener("click", () => {
    state.toolsTab = button.dataset.toolsTab || "profiles";
    renderTools();
    if (state.toolsTab === "metadata") void loadMetadataSelection();
  });
}
el.toolsReadOnly.addEventListener("change", toggleReadOnly);
el.toolsProfileSelect.addEventListener("change", async () => {
  if (!state.workspaceStore) return;
  state.workspaceStore.activeProfileId = el.toolsProfileSelect.value;
  await persistWorkspaceStore();
});
el.toolsAddProfile.addEventListener("click", addWorkspaceProfile);
el.toolsSaveProfile.addEventListener("click", saveActiveProfile);
el.toolsDeleteProfile.addEventListener("click", deleteActiveProfile);
el.toolsAddProtected.addEventListener("click", addProtectedFolder);
el.toolsAddRule.addEventListener("click", addCustomRule);
el.toolsHealthRun.addEventListener("click", analyzeHealth);
el.toolsCancel?.addEventListener("click", cancelToolsOperation);
el.toolsHealthFilter.addEventListener("change", () => { state.healthFilter = el.toolsHealthFilter.value; renderHealthTools(); });
el.toolsHealthSeverity.addEventListener("change", () => { state.healthSeverity = el.toolsHealthSeverity.value; renderHealthTools(); });
el.toolsHealthSelectVisible.addEventListener("click", () => {
  for (const item of visibleHealthFindings()) state.healthSelected.add(item.id);
  renderHealthTools();
});
el.toolsHealthClearSelection.addEventListener("click", () => { state.healthSelected.clear(); renderHealthTools(); });
el.toolsHealthExport.addEventListener("click", exportSelectedHealth);
el.toolsCreateSnapshot.addEventListener("click", createSnapshotTool);
el.toolsRefreshSnapshots.addEventListener("click", refreshSnapshots);
el.toolsChooseCompareRoot.addEventListener("click", chooseCompareRoot);
el.toolsCompareRoots.addEventListener("click", compareRootsTool);
el.toolsChooseInbox.addEventListener("click", chooseInboxFolder);
el.toolsScanInbox.addEventListener("click", scanInboxTool);
el.toolsPreviewImport.addEventListener("click", previewInboxImport);
el.toolsExecuteImport.addEventListener("click", executeInboxImport);
el.toolsMetadataPackage.addEventListener("change", loadMetadataSelection);
el.toolsSaveMetadata.addEventListener("click", savePackageMetadataTool);
el.toolsSaveGroup.addEventListener("click", saveGroupTool);
el.toolsTechSearch.addEventListener("click", runTechnicalSearch);
el.toolsExportTxt.addEventListener("click", () => exportTechnicalSelection("txt"));
el.toolsExportCsv.addEventListener("click", () => exportTechnicalSelection("csv"));
el.toolsExportJson.addEventListener("click", () => exportTechnicalSelection("json"));
el.toolsComparePackages.addEventListener("click", comparePackagesTool);
el.toolsDependencies.addEventListener("click", analyzeDependenciesTool);
el.toolsRefreshHistory.addEventListener("click", refreshOperationHistory);

el.scanBtn.addEventListener("click", () => scanFolder(false));
el.openOrganizedFolderBtn.addEventListener("click", () => {
  void openDirectorySafe(state.lastOrganizedDirectory || state.folder);
});
el.openOrganizedResultBtn.addEventListener("click", () => {
  void openDirectorySafe(state.lastOrganizedDirectory || state.folder);
});
el.clearListBtn.addEventListener("click", clearLoadedLibrary);
el.planBtn.addEventListener("click", buildPlan);
el.selectAllBtn.addEventListener("click", selectAllVisible);
el.selectNoneBtn.addEventListener("click", selectNoneVisible);
el.packageList.addEventListener("scroll", scheduleManagerVirtualRows, { passive: true });
el.duplicatesList.addEventListener("scroll", scheduleDuplicateVirtualRows, { passive: true });
el.conflictsList.addEventListener("scroll", scheduleConflictVirtualRows, { passive: true });
el.planCloseBtn.addEventListener("click", closePlanModal);
el.planCloseFooterBtn.addEventListener("click", closePlanModal);
el.planExecuteBtn.addEventListener("click", () => openConfirm("organize"));
el.planCfgCheckbox.addEventListener("change", async event => {
  if (!state.plan || state.planning || state.executing) return;

  const enabled = event.currentTarget.checked && supportsResourceCfgUpdate(state.folder);
  state.updateResourceCfg = enabled;
  state.planError = "";

  // The organization plan already contains the exact, integrity-checked
  // package destinations. Toggling Resource.cfg must only preview rules for
  // those destinations; rebuilding here used to repeat DBPF/CASP/OBJD,
  // duplicate and path analysis for the whole selection.
  if (!enabled) {
    state.plan = { ...state.plan, resourceCfgUpdate: null };
    el.planModal.__renderedPlan = null;
    render();
    return;
  }

  const existingPlan = state.plan;
  state.planning = true;
  render();
  try {
    const resourceCfgUpdate = await invoke("preview_resource_cfg_for_plan", {
      plan: existingPlan,
    });
    state.plan = { ...existingPlan, resourceCfgUpdate };
    el.planModal.__renderedPlan = null;
  } catch (error) {
    state.updateResourceCfg = false;
    state.planError = String(error);
    state.plan = { ...existingPlan, resourceCfgUpdate: null };
    el.planModal.__renderedPlan = null;
    event.currentTarget.checked = false;
  } finally {
    state.planning = false;
    render();
  }
});
el.reviewDuplicatesBtn.addEventListener("click", async () => {
  state.tab = "duplicates";
  persistPreferences();
  render();
  if (state.folder && !state.duplicatesAnalysis &&
      !state.duplicatesBusy && !state.conflictsBusy) await analyzeDuplicates();
});
el.reviewCollisionsBtn.addEventListener("click", openCollisionReview);
el.duplicateDetailsCloseBtn.addEventListener("click", closeDuplicateDetails);
el.duplicateDetailsCloseFooterBtn.addEventListener("click", closeDuplicateDetails);
el.duplicateDetailsModal.addEventListener("click", (event) => {
  if (event.target === el.duplicateDetailsModal) closeDuplicateDetails();
});
el.conflictDetailsCloseBtn.addEventListener("click", closeConflictDetails);
el.conflictDetailsCloseFooterBtn.addEventListener("click", closeConflictDetails);
el.conflictDetailsModal.addEventListener("click", (event) => {
  if (event.target === el.conflictDetailsModal) closeConflictDetails();
});
el.collisionReviewCloseBtn.addEventListener("click", closeCollisionReview);
el.collisionReviewCloseFooterBtn.addEventListener("click", closeCollisionReview);
el.collisionReviewModal.addEventListener("click", (event) => {
  if (event.target === el.collisionReviewModal) closeCollisionReview();
});

el.planModal.addEventListener("click", (event) => {
  if (event.target === el.planModal) closePlanModal();
});

el.analyzeDuplicatesBtn.addEventListener("click", analyzeDuplicates);
el.analyzeConflictsBtn.addEventListener("click", analyzeConflicts);
const applyConflictSearch = debounce((value) => {
  state.conflictsSearch = value;
  persistPreferences();
  renderConflicts();
});
el.conflictsSearch.addEventListener("input", (event) => {
  applyConflictSearch(event.currentTarget.value);
});
el.conflictsFilter.addEventListener("change", (event) => {
  state.conflictsFilter = event.currentTarget.value;
  persistPreferences();
  renderConflicts();
});
const applyDuplicateSearch = debounce((value) => {
  state.duplicatesSearch = value;
  persistPreferences();
  renderDuplicates();
});
el.duplicatesSearch.addEventListener("input", (event) => {
  applyDuplicateSearch(event.currentTarget.value);
});
el.duplicatesFilter.addEventListener("change", (event) => {
  state.duplicatesFilter = event.currentTarget.value;
  persistPreferences();
  renderDuplicates();
});

el.chooseManifestBtn.addEventListener("click", chooseManifest);
el.previewRestoreBtn.addEventListener("click", previewRestore);
el.executeRestoreBtn.addEventListener("click", () => openConfirm("restore"));

el.confirmCancelBtn.addEventListener("click", closeConfirm);
el.confirmActionBtn.addEventListener("click", async () => {
  if (state.pendingAction === "organize") await executeOrganization();
  else if (state.pendingAction === "restore") await executeRestore();
  else if (state.pendingAction === "restore_quarantine") {
    if (state.pendingQuarantineRestore) {
      await restoreQuarantineFromHistory(state.pendingQuarantineRestore);
    }
  }
  else if (state.pendingAction === "remove_restore_history") await removeSelectedRestoreHistory();
  else if (state.pendingAction === "remove_operation_history") await removeOperationHistoryRecord();
  else if (state.pendingAction === "quarantine") await executeQuarantine();
  else if (state.pendingAction === "conflict_quarantine") await executeConflictQuarantine();
  else if (state.pendingAction === "clear_cache") {
    await clearAnalysisCache();
    state.pendingAction = "";
    el.confirmModal.classList.add("hidden");
    render();
  } else if (state.pendingAction === "remove_empty_folder") {
    state.toolsBusy = true;
    try {
      await invoke("remove_empty_folder", {
        folder: state.folder,
        relativePath: state.pendingEmptyFolder,
      });
      state.healthReport = await invoke("analyze_mods_health", { folder: state.folder });
      state.toolsNotice = t("structureComplete");
      await refreshOperationHistory();
    } catch (error) {
      state.toolsError = String(error);
    } finally {
      state.toolsBusy = false;
      state.pendingEmptyFolder = "";
      state.pendingAction = "";
      el.confirmModal.classList.add("hidden");
      render();
    }
  }
});

el.confirmModal.addEventListener("click", (event) => {
  if (event.target === el.confirmModal) closeConfirm();
});

const applyManagerSearch = debounce((value) => {
  state.search = value;
  persistPreferences();
  renderResults();
});
el.searchInput.addEventListener("input", (event) => {
  applyManagerSearch(event.currentTarget.value);
});

el.statusFilterToggleBtn.addEventListener("click", (event) => {
  event.stopPropagation();
  state.isStatusFilterOpen = !state.isStatusFilterOpen;
  el.languageMenu?.classList.add("hidden");
  renderStatusFilter();
});

el.searchInput.value = state.search;
el.duplicatesSearch.value = state.duplicatesSearch;
el.conflictsSearch.value = state.conflictsSearch;

// Render the Manager immediately. Hidden Structure data must not hold the
// shared folder picker and Scan CCs buttons disabled during startup.
render();
void refreshFixIndexStatus();

if (state.folder) {
  void loadRestoreHistory();
  void refreshCacheInfo();
  void refreshConflictDecisions();
  void refreshManualOperations();
  // Structure is loaded on demand when that page is actually opened.
  if (state.tab === "structure") void loadStructure("");
  void refreshToolsContext();
}
