//! Stable identities for Manager-owned category roots.
//!
//! Folder display names are localized, but their identity is not.  This
//! module is deliberately limited to Manager-owned roots and explicit legacy
//! aliases: a similarly named personal directory is never treated as a
//! category merely because its normalized spelling happens to match.

use crate::i18n::AppLanguage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ManagedCategory {
    DuplicateQuarantine,
    Uncategorized,
    Previews,
    Corrections,
    Replacements,
    Scripts,
    Sliders,
    Poses,
    Cas,
    Construction,
    Objects,
    Store,
}

pub fn normalized_category_key(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('#')
        .chars()
        .flat_map(char::to_lowercase)
        .map(|character| match character {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        })
        .filter(|character| character.is_alphanumeric())
        .collect()
}

pub fn category_identity(value: &str) -> Option<ManagedCategory> {
    match normalized_category_key(value).as_str() {
        "duplicates" | "duplicados" => Some(ManagedCategory::DuplicateQuarantine),
        "notcategorized" | "semcategoria" | "sincategorizar" | "uncategorized" => {
            Some(ManagedCategory::Uncategorized)
        }
        "ccpreviews" | "previewsdeccs" | "vistaspreviasdecc" => Some(ManagedCategory::Previews),
        "fixes" | "corrections" | "correcoes" | "correcciones" => {
            Some(ManagedCategory::Corrections)
        }
        "replacements" | "substituicoes" | "reemplazos" => Some(ManagedCategory::Replacements),
        "scripts" => Some(ManagedCategory::Scripts),
        "sliders" => Some(ManagedCategory::Sliders),
        "poses" | "posesandanimations" | "posesanimacoes" | "posesyanimaciones" => {
            Some(ManagedCategory::Poses)
        }
        "cas" => Some(ManagedCategory::Cas),
        "construction" | "construcao" | "construccion" | "build" => {
            Some(ManagedCategory::Construction)
        }
        "objects" | "objetos" => Some(ManagedCategory::Objects),
        "store" | "sims3store" | "storecontent" => Some(ManagedCategory::Store),
        _ => None,
    }
}

pub fn canonical_folder(category: ManagedCategory, language: AppLanguage) -> &'static str {
    match category {
        ManagedCategory::DuplicateQuarantine => language.duplicates_folder(),
        ManagedCategory::Uncategorized => language.uncategorized_fallback_folder(),
        ManagedCategory::Previews => language.previews_folder(),
        ManagedCategory::Corrections => language.corrections(),
        ManagedCategory::Replacements => match language {
            AppLanguage::En => "Replacements",
            AppLanguage::Pt => "Substituições",
            AppLanguage::Es => "Reemplazos",
        },
        ManagedCategory::Scripts => "Scripts",
        ManagedCategory::Sliders => "Sliders",
        ManagedCategory::Poses => match language {
            AppLanguage::En => "Poses and Animations",
            AppLanguage::Pt => "Poses e Animações",
            AppLanguage::Es => "Poses y Animaciones",
        },
        ManagedCategory::Cas => "CAS",
        ManagedCategory::Construction => match language {
            AppLanguage::En => "Construction",
            AppLanguage::Pt => "Construção",
            AppLanguage::Es => "Construcción",
        },
        ManagedCategory::Objects => match language {
            AppLanguage::En => "Objects",
            AppLanguage::Pt | AppLanguage::Es => "Objetos",
        },
        // Store item/set names are catalog identifiers.  Only the managed
        // root is canonicalized; its catalog-defined descendants are kept.
        ManagedCategory::Store => "Store",
    }
}

pub fn canonicalize_managed_root(value: &str, language: AppLanguage) -> Option<String> {
    category_identity(value).map(|category| canonical_folder(category, language).to_string())
}

/// Rewrites only the first semantic folder.  This is intentionally not fuzzy
/// recursive matching: descendants are canonicalized by the technical
/// classifier/manual model, while arbitrary user project folders stay intact.
pub fn canonicalize_managed_destination_parts(
    parts: &[String],
    language: AppLanguage,
) -> Vec<String> {
    let mut result = parts.to_vec();
    let first_semantic = result.iter().position(|part| {
        !part.eq_ignore_ascii_case("Packages") && !part.eq_ignore_ascii_case("Overrides")
    });
    if let Some(index) = first_semantic {
        if let Some(canonical) = canonicalize_managed_root(&result[index], language) {
            result[index] = canonical;
        }
    }
    result
}

pub fn is_category_alias(value: &str, category: ManagedCategory) -> bool {
    category_identity(value) == Some(category)
}

/// True only for a top-level Manager category root.  Callers must pass the
/// directory component immediately below Packages (or below a standalone
/// Packages root); this intentionally does not inspect arbitrary descendants.
pub fn is_duplicate_quarantine_root(value: &str) -> bool {
    is_category_alias(value, ManagedCategory::DuplicateQuarantine)
}

/// The roots that must exist as stable Manager buckets even when empty.  An
/// alias is never included here: after a language migration it becomes a
/// legacy directory that may be removed once empty.
pub fn stable_empty_roots(language: AppLanguage) -> Vec<String> {
    [
        ManagedCategory::DuplicateQuarantine,
        ManagedCategory::Uncategorized,
        ManagedCategory::Previews,
    ]
    .into_iter()
    .map(|category| canonical_folder(category, language).to_string())
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translated_and_historical_roots_share_one_identity() {
        for value in ["#Duplicates", "#Duplicados", "duplicates", " Duplicados "] {
            assert_eq!(
                category_identity(value),
                Some(ManagedCategory::DuplicateQuarantine)
            );
        }
        for value in [
            "#Sem Categoria",
            "Sem_Categoria",
            "#Not Categorized",
            "Sin Categorizar",
        ] {
            assert_eq!(
                category_identity(value),
                Some(ManagedCategory::Uncategorized)
            );
        }
        assert_eq!(
            canonicalize_managed_root("#Duplicates", AppLanguage::Pt).as_deref(),
            Some("#Duplicados")
        );
        assert_eq!(
            canonicalize_managed_root("#Duplicados", AppLanguage::En).as_deref(),
            Some("#Duplicates")
        );
    }

    #[test]
    fn canonicalization_is_explicit_not_a_fuzzy_personal_folder_match() {
        assert_eq!(category_identity("Anthon"), None);
        assert_eq!(category_identity("My Object Collection"), None);
        assert_eq!(
            canonicalize_managed_destination_parts(
                &["Packages".into(), "Fixes".into(), "Author".into()],
                AppLanguage::Pt,
            ),
            vec!["Packages", "Correções", "Author"],
        );
    }
}
