# Veiga's S3CC Manager — v1.0.0 Beta 2

**Release date:** October 8, 2026  
**Status:** Public beta / pre-release  
**Platforms:** Windows x64, Linux x64, macOS Intel and macOS Apple Silicon (portable ZIP builds)

> **Release scope:** The sections **Added**, **Fixed**, **Improved**, **Changed**, and **Removed / no longer automatic** below describe the published v1.0.0 Beta 2 release. Work carried out after October 8 is listed separately under **Post-release development** and is **not included in the currently published Beta 2 downloads**. This document does not change the tagged binaries.

## Added

### Package detection and classification
- Internal-signature recognition for **NRaas** packages, rather than relying on names alone.
- Recognition of pose and animation content using **CLIP** and pose-list evidence.
- More detailed Build/Buy classification using s3pi-aligned **OBJD function and subcategory flags**.
- Creator candidates and script-mod name detection, with corroboration from internal package data before confident assignments.
- Separate scanner metadata for creator, mod name, gameplay category and **classification confidence**.
- Classification evidence in the Organizer preview and exported audit reports.

### Package conversion
- Best-effort recovery of missing matching thumbnails from The Sims 3 thumbnail caches during **Sims3Pack → Package** conversion.
- Preservation of thumbnails already embedded in the converted package.

### Interface and diagnostics
- Expanded appearance/theme support inspired by **WinUI 3 / Fluent**.
- More informative organization-plan preview statuses and diagnostic details.
- Richer classification metadata in audit output.

## Fixed

- Cleaned malformed or exposed **CDATA** wrappers in localized Sims3Pack names.
- Corrected preservation/reconstruction of **_KEY / NMAP** naming information during combined conversion.
- Improved classification precedence for **S3SA** gameplay scripts to reduce incorrect assignment as ordinary CAS or object content.
- Addressed cases where missing thumbnail-cache references prevented best-effort thumbnail recovery.
- Prevented the Organizer from silently overwriting an existing destination file.

## Improved

### Organizer and classification
- Improved category decisions based on real package resources, rather than unsupported filename-only assumptions.
- Better preservation of **Room** and **RoomSubCategory** as metadata while using functional categories for physical Build/Buy organization.
- More conservative creator and script-mod classification when evidence is incomplete.
- Clearer reasons for classification and review decisions.

### Planning, Resource.cfg and safety
- Better support for **partial organization**: safe, classified packages may proceed while unrelated uncertain packages remain in place.
- The Planner reads supported `PackedFile` rules from the actual **Resource.cfg**.
- Preserves required path prefixes, including `Packages\`, when validating loading coverage.
- Compacts excessively deep logical taxonomies where necessary to respect game-loading depth.
- Treats unsupported or ambiguous loading rules conservatively instead of assuming they are safe.
- Restore manifests record **executable file moves**, not speculative changes.

### Performance and usability
- Improved virtualized result lists and search debouncing.
- Improved filtering caches and reuse of available scan data.
- Refined system-theme behavior and WinUI-inspired appearance.
- More useful performance and classification diagnostics.

## Changed

- **Unknown**, **Mixed**, **Needs Review** and **Invalid** packages remain in their original locations during ordinary automatic organization.
- **Not Categorized** is a review outcome in the normal planning flow; it is not a mandatory automatic destination for every unresolved file.
- Potential duplicates and content collisions are **reviewed rather than automatically relocated**.
- Organization previews and confirmations communicate which selected files are executable and which require review.

## Removed / no longer automatic

- Automatic routing of uncertain package classifications into an arbitrary fallback folder during regular organization.
- Automatic relocation of duplicate/collision review cases without an explicit, safe resolution.
- Any assumption that a matching filename alone proves file identity or author/creator.

**No existing CC files are intentionally deleted by this release.** These changes remove unsafe *automatic behaviors*, not users' installed content.

## Security, integrity and rollback

- Existing destinations are protected against silent overwrite.
- Ambiguous files remain in place for manual review.
- Planner decisions are previewed before write operations.
- Restore and organization preserve executable-move records for reversibility.
- Unsupported Resource.cfg cases are reported conservatively.

## Validation and distribution

- GitHub workflows validate the frontend build and Rust library tests for relevant changes on the main branch.
- Portable packages are published for Windows x64, Linux x64, macOS Intel and macOS Apple Silicon, with accompanying SHA-256 checksum files.
- As a beta release, real-world mod combinations may still reveal edge cases. Back up important Mods directories before using write operations.

## Post-release development — NOT included in the published Beta 2 binaries

The following work was developed or validated **after** the published October 8 release. It is documented here for transparency, **not** as functionality already present in the downloadable v1.0.0 Beta 2 assets.

### Locally developed and tested, pending integration and a new published build
- More precise **FBLN-based slider detection**, including packages containing multiple FBLN resources, validated against a 299-package sample.
- Further **CASP / OBJD** classification and destination improvements based on S3PI definitions and a read-only manual model audit.
- **Body Hair** classification by CASP type and supported gender, without invented age or anatomical subfolders.
- Safer localized category paths and reconciliation of partly organized libraries.
- Flat **#Duplicados** quarantine layout with collision-safe naming and tracked SHA-256 identities.
- Targeted improvements to duplicate-keeper verification, filename-collision review, rollback and legacy-folder cleanup.
- Targeted Rust and JavaScript regression coverage, including real DBPF fixtures when available.

### Known follow-up work / not yet claimed as fixed
- Further investigation of **Resource.cfg** checkbox performance and exclusion of non-package preview files.
- Investigation of destination collisions discovered during real organization, including the `Destination appeared after preflight` safeguard.
- Final modal alignment and responsiveness checks in the new executable.
- Broader background-processing and shared-cache improvements across the application.
- Planned Duplicates/Conflicts action-layout changes inspired by The Sims 3 Dashboard.
- The 52 manual-classification review cases, eight Body Hair filename-collision reviews and a separate TMZ restoration decision remain subject to explicit review.

Advanced **Health**, **Structure**, **Snapshots**, **Inbox**, **Metadata** and **Technical** capabilities follow the project's staged public release roadmap; their existence in development source does not mean they are all exposed in the published Beta 2 build.

## Feedback

This is a testing release. Please report reproducible issues with the affected package or Sims3Pack, the expected classification or destination, the observed result, and relevant diagnostic output.

**Published release:** [v1.0.0 Beta 2](https://github.com/MrDavidVeiga/S3CCOrganizer/releases/tag/v1.0.0-beta.2)  
**Release roadmap:** [docs/RELEASE-ROADMAP.md](RELEASE-ROADMAP.md)
