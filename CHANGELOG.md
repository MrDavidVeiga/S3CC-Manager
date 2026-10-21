# Changelog

## v1.0.0 Beta 3

Beta 3 compared with Beta 2:

- More complete reconciliation of legacy package folders into the canonical Manager hierarchy, including safe handling of aliases, old empty directories and preserved loading branches.
- Expanded Store and official-resource evidence, with clearer handling of Sets, Items, Premium Content and partial overlaps without treating incomplete evidence as an automatic move decision.
- Improved recognition of CAS replacements, eyelashes, roofs, gameplay companions, sliders and grouped mod content while preserving review states when evidence remains insufficient.
- Safer exact-duplicate keeper resolution and quarantine planning, including packages already present in quarantine, active keeper protection and clearer blocked-operation reasons.
- Better reuse and invalidation of scanner, classification, Store and analysis cache data after file, rule or classification changes.
- On-demand mesh polycount information for supported GEOM/MLOD/MODL resources, with diagnostics retained when a resource cannot be decoded.
- Refined Organizer, Duplicates and Conflicts presentation, including localized filters, stable selection, clearer status feedback and improved theme/control alignment.
- Improved EA App search contrast and interaction states, semantic result colors, language/theme control geometry and PT / EN / ES interface consistency.

Beta 3 is a compatibility and reliability release over Beta 2. It does not change the safety model: uncertain classifications, unresolved collisions and unsupported evidence remain review-only rather than being moved automatically.

## v1.0.0 Beta 2

Beta 2 compared with Beta 1:

- Safer resource-based organization using CASP, OBJD, FBLN and supported package-family evidence.
- Reversible planning, duplicate quarantine, keeper protection, restore manifests and rollback.
- Conservative handling of unknown packages, mixed evidence and destination collisions.
- Improved Resource.cfg-aware destinations and Before → After plan previews.
- Independent Organizer, Duplicates and Conflicts analysis flows with reusable cache data.
- PT / EN / ES localization and a broader Fluent/WinUI-inspired responsive interface.
- Better search, filtering, virtualization, operation progress and diagnostic reporting.
- Improved Sims3Pack conversion naming, thumbnail preservation and overwrite protection.

Beta 2 intentionally removes automatic relocation when the available evidence is insufficient; those items remain visible for review.

## v1.0.0 Beta 1

See [Beta 1 release notes](docs/RELEASE-NOTES-v1.0.0-BETA1.md).
