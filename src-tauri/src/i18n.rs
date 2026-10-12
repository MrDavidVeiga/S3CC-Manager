use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AppLanguage {
    En,
    Pt,
    Es,
}

impl AppLanguage {
    pub fn not_categorized_folder(self) -> &'static str {
        match self {
            Self::En => "Not Categorized",
            Self::Pt => "Sem Categoria",
            Self::Es => "Sin Categorizar",
        }
    }

    /// Physical fallback folder used by the organizer. The leading marker
    /// keeps this review-only bucket distinct from a real model category.
    pub fn uncategorized_fallback_folder(self) -> &'static str {
        match self {
            Self::En => "#Not Categorized",
            Self::Pt => "#Sem Categoria",
            Self::Es => "#Sin Categorizar",
        }
    }

    pub fn duplicates_folder(self) -> &'static str {
        match self {
            Self::En => "#Duplicates",
            Self::Pt | Self::Es => "#Duplicados",
        }
    }

    pub fn previews_folder(self) -> &'static str {
        match self {
            Self::En => "CC Previews",
            Self::Pt => "Previews de CCs",
            Self::Es => "Vistas previas de CC",
        }
    }

    /// Canonical physical folder for functional fixes. The English model
    /// uses `Fixes`; `Corrections` is not a second physical tree.
    pub fn corrections(self) -> &'static str {
        match self {
            Self::En => "Fixes",
            Self::Pt => "Correções",
            Self::Es => "Correcciones",
        }
    }

    pub fn cas(self) -> &'static str { "CAS" }

    pub fn clothing(self) -> &'static str {
        match self {
            Self::En => "Clothing",
            Self::Pt => "Roupas",
            Self::Es => "Ropa",
        }
    }

    pub fn hair(self) -> &'static str {
        match self {
            Self::En => "Hair",
            Self::Pt => "Cabelos",
            Self::Es => "Cabello",
        }
    }

    pub fn accessories(self) -> &'static str {
        match self {
            Self::En => "Accessories",
            Self::Pt => "Acessórios",
            Self::Es => "Accesorios",
        }
    }

    pub fn makeup(self) -> &'static str {
        match self {
            Self::En => "Makeup",
            Self::Pt => "Maquiagem",
            Self::Es => "Maquillaje",
        }
    }

    pub fn genetics(self) -> &'static str {
        match self {
            Self::En => "Genetics",
            Self::Pt => "Genética",
            Self::Es => "Genética",
        }
    }

    pub fn presets(self) -> &'static str {
        match self {
            Self::En => "Presets",
            Self::Pt => "Predefinições",
            Self::Es => "Preajustes",
        }
    }

    pub fn male(self) -> &'static str {
        match self {
            Self::En => "Male",
            Self::Pt => "Masculino",
            Self::Es => "Masculino",
        }
    }

    pub fn female(self) -> &'static str {
        match self {
            Self::En => "Female",
            Self::Pt => "Feminino",
            Self::Es => "Femenino",
        }
    }

    pub fn unisex(self) -> &'static str {
        match self {
            Self::En => "Unisex",
            Self::Pt => "Unissex",
            Self::Es => "Unisex",
        }
    }

    pub fn unknown(self) -> &'static str {
        match self {
            Self::En => "Unknown",
            Self::Pt => "Desconhecido",
            Self::Es => "Desconocido",
        }
    }

    pub fn baby(self) -> &'static str {
        match self {
            Self::En => "Baby",
            Self::Pt => "Recém-Nascido",
            Self::Es => "Bebé",
        }
    }

    pub fn toddler(self) -> &'static str {
        match self {
            Self::En => "Toddler",
            Self::Pt => "Bebê",
            Self::Es => "Niño Pequeño",
        }
    }

    pub fn child(self) -> &'static str {
        match self {
            Self::En => "Child",
            Self::Pt => "Criança",
            Self::Es => "Niño",
        }
    }

    pub fn teen(self) -> &'static str {
        match self {
            Self::En => "Teen",
            Self::Pt => "Adolescente",
            Self::Es => "Adolescente",
        }
    }

    pub fn young_adult_adult(self) -> &'static str {
        match self {
            Self::En => "YA-A",
            Self::Pt => "Jovem Adulto-Adulto",
            Self::Es => "Adulto Joven-Adulto",
        }
    }

    pub fn elder(self) -> &'static str {
        match self {
            Self::En => "Elder",
            Self::Pt => "Idoso",
            Self::Es => "Anciano",
        }
    }

    pub fn multiple_ages(self) -> &'static str {
        match self {
            Self::En => "Multiple Ages",
            Self::Pt => "Múltiplas Idades",
            Self::Es => "Varias Edades",
        }
    }

    pub fn top(self) -> &'static str {
        match self {
            Self::En => "Top",
            Self::Pt => "Parte de Cima",
            Self::Es => "Parte Superior",
        }
    }

    pub fn bottom(self) -> &'static str {
        match self {
            Self::En => "Bottom",
            Self::Pt => "Parte de Baixo",
            Self::Es => "Parte Inferior",
        }
    }

    pub fn outfit(self) -> &'static str {
        match self {
            Self::En => "Outfit",
            Self::Pt => "Conjunto",
            Self::Es => "Conjunto",
        }
    }

    pub fn shoes(self) -> &'static str {
        match self {
            Self::En => "Shoes",
            Self::Pt => "Calçados",
            Self::Es => "Calzado",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_categorized_is_localized() {
        assert_eq!(AppLanguage::En.not_categorized_folder(), "Not Categorized");
        assert_eq!(AppLanguage::Pt.not_categorized_folder(), "Sem Categoria");
        assert_eq!(AppLanguage::Es.not_categorized_folder(), "Sin Categorizar");
    }

    #[test]
    fn review_folders_are_localized_and_distinct() {
        assert_eq!(AppLanguage::Pt.uncategorized_fallback_folder(), "#Sem Categoria");
        assert_eq!(AppLanguage::Pt.duplicates_folder(), "#Duplicados");
        assert_eq!(AppLanguage::Pt.previews_folder(), "Previews de CCs");
        assert_eq!(AppLanguage::En.uncategorized_fallback_folder(), "#Not Categorized");
        assert_eq!(AppLanguage::En.duplicates_folder(), "#Duplicates");
        assert_eq!(AppLanguage::En.previews_folder(), "CC Previews");
    }

    #[test]
    fn fixes_folder_uses_the_canonical_english_name() {
        assert_eq!(AppLanguage::En.corrections(), "Fixes");
        assert_eq!(AppLanguage::Pt.corrections(), "Correções");
        assert_eq!(AppLanguage::Es.corrections(), "Correcciones");
    }
}
