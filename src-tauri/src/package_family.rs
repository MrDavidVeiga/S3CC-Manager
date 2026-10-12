use crate::i18n::AppLanguage;
use std::collections::BTreeSet;

pub const TYPE_IMG: u32 = 0x00B2_D882;
pub const TYPE_NMAP: u32 = 0x0166_038C;
pub const TYPE_XML: u32 = 0x0333_406C;
pub const TYPE_SKIN_TONE: u32 = 0x0354_796A;
pub const TYPE_HAIR_TONE: u32 = 0x0355_5BA8;
pub const TYPE_BONE_DELTA: u32 = 0x0355_E0A6;
pub const TYPE_FACE: u32 = 0x0358_B08A;
pub const TYPE_ITUN: u32 = 0x03B3_3DDF;
pub const TYPE_BBLN: u32 = 0x062C_8204;
pub const TYPE_BGEO: u32 = 0x067C_AA11;
pub const TYPE_S3SA: u32 = 0x073F_AA07;
pub const TYPE_STBL: u32 = 0x2205_57DA;
pub const TYPE_MANIFEST: u32 = 0x73E9_3EEB;
pub const TYPE_FBLN: u32 = 0xB52F_5055;
pub const TYPE_PTRN: u32 = 0xD4D9_FBE5;

#[derive(Debug, Clone)]
pub struct PackageFamilyClassification {
    pub main_category: String,
    pub sub_category: Option<String>,
    pub folder_parts: Vec<String>,
    pub detected_from: Vec<String>,
    pub technical_reason: String,
}

#[derive(Debug, Clone)]
pub enum PackageFamilyResult {
    Classified(PackageFamilyClassification),
    Ambiguous(Vec<PackageFamilyClassification>),
    None,
}

fn localized(
    language: AppLanguage,
    en: &'static str,
    pt: &'static str,
    es: &'static str,
) -> &'static str {
    match language {
        AppLanguage::En => en,
        AppLanguage::Pt => pt,
        AppLanguage::Es => es,
    }
}

fn classification(
    main_category: &str,
    sub_category: Option<&str>,
    folder_parts: Vec<&str>,
    detected_from: Vec<&str>,
) -> PackageFamilyClassification {
    let mut folder_parts = folder_parts.into_iter().map(str::to_string).collect::<Vec<_>>();
    // Keep CAS as metadata, but remove it as a physical directory.
    if folder_parts.first().is_some_and(|part| part == "CAS") {
        folder_parts.remove(0);
    }
    let detected_from = detected_from.into_iter().map(str::to_string).collect::<Vec<_>>();
    let technical_reason = format!(
        "Resource family [{}] => {}",
        detected_from.join(", "),
        folder_parts.join("\\")
    );
    PackageFamilyClassification {
        main_category: main_category.to_string(),
        sub_category: sub_category.map(str::to_string),
        folder_parts,
        detected_from,
        technical_reason,
    }
}

fn has_any(types: &BTreeSet<u32>, candidates: &[u32]) -> bool {
    candidates.iter().any(|resource_type| types.contains(resource_type))
}

fn tuning_only_family(types: &BTreeSet<u32>) -> bool {
    if !has_any(types, &[TYPE_ITUN, TYPE_XML]) {
        return false;
    }

    types.iter().all(|resource_type| {
        matches!(
            *resource_type,
            TYPE_ITUN | TYPE_XML | TYPE_STBL | TYPE_NMAP | TYPE_MANIFEST
        )
    })
}

fn localization_only_family(types: &BTreeSet<u32>) -> bool {
    types.contains(&TYPE_STBL)
        && types.iter().all(|resource_type| {
            matches!(*resource_type, TYPE_STBL | TYPE_NMAP | TYPE_MANIFEST)
        })
}

pub fn classify_package_family(
    types: &BTreeSet<u32>,
    language: AppLanguage,
) -> PackageFamilyResult {
    if types.is_empty() {
        return PackageFamilyResult::None;
    }

    // Script packages often include XML/ITUN/STBL alongside the assembly.
    // S3SA is therefore authoritative for this package-level family.
    if types.contains(&TYPE_S3SA) {
        return PackageFamilyResult::Classified(classification(
            localized(language, "Gameplay", "Jogabilidade", "Jugabilidad"),
            Some(localized(language, "Gameplay", "Jogabilidade", "Jugabilidad")),
            vec![
                localized(language, "Scripts", "Scripts", "Scripts"),
                localized(language, "Gameplay", "Jogabilidade", "Jugabilidad"),
                localized(language, "Unknown Author", "Autor Não Identificado", "Autor Desconocido"),
            ],
            vec!["S3SA"],
        ));
    }

    let mut candidates = Vec::new();

    if types.contains(&TYPE_SKIN_TONE) {
        candidates.push(classification(
            "CAS",
            Some(localized(
                language,
                "Skin Tones",
                "Tons de Pele",
                "Tonos de Piel",
            )),
            vec![
                "CAS",
                localized(language, "Genetics", "Genética", "Genética"),
                localized(
                    language,
                    "Skin Tones",
                    "Tons de Pele",
                    "Tonos de Piel",
                ),
            ],
            vec!["SkinTone"],
        ));
    }

    if types.contains(&TYPE_HAIR_TONE) {
        candidates.push(classification(
            "CAS",
            Some(localized(
                language,
                "Hair Tones",
                "Tons de Cabelo",
                "Tonos de Cabello",
            )),
            vec![
                "CAS",
                localized(language, "Genetics", "Genética", "Genética"),
                localized(
                    language,
                    "Hair Tones",
                    "Tons de Cabelo",
                    "Tonos de Cabello",
                ),
            ],
            vec!["HairTone"],
        ));
    }

    // FBLN is the BlendUnit/slider definition. FACE, BGEO, BBLN and BOND are
    // supporting morph resources and are intentionally insufficient on their
    // own: replacements and mesh companions may contain the same resource
    // families without defining a CAS slider.
    let definitive_slider = types.contains(&TYPE_FBLN);

    if definitive_slider {
        let mut detected = Vec::new();
        for (resource_type, label) in [
            (TYPE_BONE_DELTA, "BoneDelta"),
            (TYPE_FACE, "FACE"),
            (TYPE_BBLN, "BBLN"),
            (TYPE_BGEO, "BGEO"),
            (TYPE_FBLN, "FBLN"),
        ] {
            if types.contains(&resource_type) {
                detected.push(label);
            }
        }

        candidates.push(classification(
            "CAS",
            Some("Sliders"),
            vec!["CAS", "Sliders"],
            detected,
        ));
    }

    if types.contains(&TYPE_PTRN) {
        candidates.push(classification(
            localized(language, "Patterns", "Padrões", "Patrones"),
            None,
            vec![localized(language, "Patterns", "Padrões", "Patrones")],
            vec!["PTRN"],
        ));
    }

    if candidates.len() > 1 {
        return PackageFamilyResult::Ambiguous(candidates);
    }
    if let Some(classification) = candidates.pop() {
        return PackageFamilyResult::Classified(classification);
    }

    if tuning_only_family(types) {
        let mut detected = Vec::new();
        if types.contains(&TYPE_ITUN) {
            detected.push("ITUN");
        }
        if types.contains(&TYPE_XML) {
            detected.push("XML");
        }

        return PackageFamilyResult::Classified(classification(
            localized(language, "Gameplay", "Jogabilidade", "Jugabilidad"),
            Some("Tuning"),
            vec![
                localized(language, "Gameplay", "Jogabilidade", "Jugabilidad"),
                "Tuning",
            ],
            detected,
        ));
    }

    if localization_only_family(types) {
        return PackageFamilyResult::Classified(classification(
            localized(language, "Localization", "Localização", "Localización"),
            None,
            vec![localized(
                language,
                "Localization",
                "Localização",
                "Localización",
            )],
            vec!["STBL"],
        ));
    }

    PackageFamilyResult::None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(values: &[u32]) -> BTreeSet<u32> {
        values.iter().copied().collect()
    }

    #[test]
    fn skin_tone_is_classified_from_resource_type() {
        let result = classify_package_family(
            &set(&[TYPE_SKIN_TONE, TYPE_IMG]),
            AppLanguage::Pt,
        );
        let PackageFamilyResult::Classified(value) = result else {
            panic!("expected classified");
        };
        assert_eq!(value.folder_parts, vec!["Genética", "Tons de Pele"]);
    }

    #[test]
    fn geom_bgeo_foot_replacements_are_not_mistaken_for_sliders() {
        let result = classify_package_family(
            &set(&[0x015A_1849, TYPE_BGEO]),
            AppLanguage::En,
        );
        assert!(matches!(result, PackageFamilyResult::None));
    }

    #[test]
    fn eyelash_mesh_with_bbln_bgeo_is_not_a_slider() {
        let result = classify_package_family(
            &set(&[0x015A_1849, TYPE_BGEO, TYPE_BBLN, 0x7368_84F1]),
            AppLanguage::En,
        );
        assert!(matches!(result, PackageFamilyResult::None));
    }

    #[test]
    fn slider_family_requires_the_fbln_blend_unit() {
        let result = classify_package_family(
            &set(&[TYPE_FACE, TYPE_BGEO]),
            AppLanguage::En,
        );
        assert!(matches!(result, PackageFamilyResult::None));

        let result = classify_package_family(&set(&[TYPE_FBLN, TYPE_FACE]), AppLanguage::En);
        let PackageFamilyResult::Classified(value) = result else {
            panic!("expected FBLN classified");
        };
        assert_eq!(value.folder_parts, vec!["Sliders"]);
    }

    #[test]
    fn script_wins_over_embedded_tuning_resources() {
        let result = classify_package_family(
            &set(&[TYPE_S3SA, TYPE_XML, TYPE_ITUN, TYPE_STBL]),
            AppLanguage::En,
        );
        let PackageFamilyResult::Classified(value) = result else {
            panic!("expected classified");
        };
        assert_eq!(value.folder_parts, vec!["Scripts", "Gameplay", "Unknown Author"]);
    }

    #[test]
    fn xml_plus_visual_resources_is_not_assumed_to_be_tuning() {
        let result = classify_package_family(
            &set(&[TYPE_XML, TYPE_IMG]),
            AppLanguage::En,
        );
        assert!(matches!(result, PackageFamilyResult::None));
    }

    #[test]
    fn multiple_authoritative_families_require_review() {
        let result = classify_package_family(
            &set(&[TYPE_SKIN_TONE, TYPE_PTRN]),
            AppLanguage::En,
        );
        assert!(matches!(result, PackageFamilyResult::Ambiguous(_)));
    }
}
