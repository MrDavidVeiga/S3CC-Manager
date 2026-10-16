# Veiga's S3CC Manager v1.0.0 Beta 2

Beta 2 focuses on safer automatic organization, clearer classification evidence, broader The Sims 3 package recognition and a more polished WinUI-inspired interface.

## Compared with Beta 1

Beta 2 introduces the following user-visible changes over the first public beta:

- More resource-based CASP, OBJD, FBLN, script, pose, animation and object-family recognition.
- Safer planning for unknown, mixed, invalid, duplicate and collision cases, with no silent overwrites.
- Reversible organization, quarantine, restore manifests and rollback protections that preserve the active keeper.
- More conservative Resource.cfg handling and clearer Before → After planning evidence.
- Independent Organizer, Duplicates and Conflicts workflows with reusable scan data and incremental cache validation.
- Expanded PT / EN / ES localization, theme selection and a Fluent/WinUI-inspired shell with responsive result lists.
- Improved list virtualization, search and filtering responsiveness, plus clearer operation progress and result diagnostics.
- More complete Sims3Pack conversion naming, thumbnail preservation and no-overwrite output handling.

Relative to Beta 1, automatic moves are deliberately more conservative: uncertain packages and unresolved collisions remain available for review instead of being relocated on weak evidence.

## Classification and organization

- Added dedicated internal-signature detection for NRaas packages.
- Added pose and animation recognition using CLIP and pose-list evidence.
- Expanded Build/Buy object classification using s3pi-aligned function and subcategory flags.
- Preserved Room and RoomSubCategory information as metadata while keeping physical object organization based on functional category.
- Improved gameplay script precedence so S3SA packages are not misfiled as ordinary objects/CAS content.
- Added conservative creator detection using filename candidates only when corroborated by package internals.
- Added script-mod name inference and physical destinations such as `Gameplay\Creator\Mod Name`.
- Exposed creator, mod name, gameplay category and classification confidence as separate scanner metadata.
- Added classification evidence to the Manager preview and audit output.

## Safer Planner behavior

- Unknown, Mixed, Needs Review and Invalid packages now stay physically in their current location.
- Not Categorized is treated as a review state during normal organization rather than an automatic dumping folder.
- Duplicate/collision review cases stay in place instead of being automatically relocated.
- Safe classified packages can still be organized when unrelated unresolved packages remain in the selection.
- Existing files are never silently overwritten.
- Restore manifests contain only executable moves.

## Resource.cfg safety

- The Planner reads supported PackedFile rules from the actual Resource.cfg.
- Required fixed prefixes such as `Packages\` are preserved.
- Deep logical taxonomies are compacted when necessary to keep the resulting package path loadable.
- Unsupported/ambiguous traversal is handled conservatively instead of pretending the destination is safe.

## Sims3Pack to Package

- Cleaned exposed/malformed CDATA wrappers from localized Sims3Pack names.
- Preserved/rebuilt _KEY/NMAP naming information during combined conversion.
- Preserved embedded thumbnails.
- Added best-effort recovery of missing matching thumbnails from The Sims 3 thumbnail caches without replacing thumbnails already present in the target package.

## Interface and performance

- Updated the shell toward a higher-fidelity WinUI 3 / Fluent presentation.
- Added the expanded theme system and refined system/theme behavior.
- Improved list virtualization, search debounce, filtering caches and scan-data reuse.
- Added clearer Preview Plan states, classification evidence and performance diagnostics.
- Audit reports now carry richer classification metadata.

## Validation and future corpus audit

Beta 2 includes automatic GitHub validation for the frontend build and Rust library tests on relevant main-branch changes.

The next classifier audit will use the user's complete local `Mods\Packages` tree as a read-only corpus, preserving its existing directory structure as evidence. That full-corpus pass is intentionally separate from Beta 2's conservative automatic rules.

## Beta notice

This is a testing release. Back up important Mods folders before testing write operations. The Manager is designed to preview and explain changes before execution, but real-world beta feedback remains important for uncommon package combinations.
