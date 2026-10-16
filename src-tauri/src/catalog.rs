use crate::i18n::AppLanguage;
use byteorder::{LittleEndian, ReadBytesExt};
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Read};

pub const TYPE_CASP: u32 = 0x034A_EECB;
pub const TYPE_OBJD: u32 = 0x319E_4F1D;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogClassification {
    pub source: String,
    pub kind: String,
    pub main_category: String,
    pub sub_category: Option<String>,
    pub gender: Option<String>,
    pub age: Option<String>,
    pub species: Option<String>,
    pub usage_categories: Vec<String>,
    pub folder_parts: Vec<String>,
    pub candidate_folder_parts: Vec<Vec<String>>,
    pub ambiguous: bool,
    pub technical_reason: String,
}

#[derive(Debug, Clone, Copy)]
struct CaspCore {
    clothing_type: u32,
    type_flags: u32,
    age_species_gender: u32,
    clothing_category: u32,
}

#[derive(Debug, Clone, Copy)]
struct ObjdCatalogFlags {
    object_type_flags: u32,
    room_flags: u32,
    room_sub_category_flags: u64,
    function_category_flags: u32,
    sub_category1_flags: u64,
    sub_category2_flags: u64,
    build_category_flags: u32,
}

pub fn classify_resource(
    resource_type: u32,
    data: &[u8],
    language: AppLanguage,
) -> Option<CatalogClassification> {
    match resource_type {
        TYPE_CASP => classify_casp(data, language),
        TYPE_OBJD => classify_objd(data, language),
        _ => None,
    }
}

fn tr(language: AppLanguage, key: &str) -> &'static str {
    match (language, key) {
        // Root and CAS
        (_, "CAS") => "CAS",
        (AppLanguage::En, "Clothing") => "Clothing",
        (AppLanguage::Pt, "Clothing") => "Roupas",
        (AppLanguage::Es, "Clothing") => "Ropa",
        (AppLanguage::En, "Hair") => "Hair",
        (AppLanguage::Pt, "Hair") => "Cabelos",
        (AppLanguage::Es, "Hair") => "Cabello",
        (AppLanguage::En, "Accessories") => "Accessories",
        (AppLanguage::Pt, "Accessories") => "Acessórios",
        (AppLanguage::Es, "Accessories") => "Accesorios",
        (AppLanguage::En, "Makeup") => "Makeup",
        (AppLanguage::Pt, "Makeup") => "Maquiagem",
        (AppLanguage::Es, "Makeup") => "Maquillaje",
        (AppLanguage::En, "Genetics") => "Genetics",
        (AppLanguage::Pt, "Genetics") => "Genética",
        (AppLanguage::Es, "Genetics") => "Genética",
        (AppLanguage::En, "Pets") => "Pets",
        (AppLanguage::Pt, "Pets") => "Animais",
        (AppLanguage::Es, "Pets") => "Mascotas",

        // CAS subcategories
        (AppLanguage::En, "Top") => "Top",
        (AppLanguage::Pt, "Top") => "Parte de Cima",
        (AppLanguage::Es, "Top") => "Parte Superior",
        (AppLanguage::En, "Bottom") => "Bottom",
        (AppLanguage::Pt, "Bottom") => "Parte de Baixo",
        (AppLanguage::Es, "Bottom") => "Parte Inferior",
        (AppLanguage::En, "Outfit") => "Outfit",
        (AppLanguage::Pt, "Outfit") => "Conjunto",
        (AppLanguage::Es, "Outfit") => "Conjunto",
        (AppLanguage::En, "Shoes") => "Shoes",
        (AppLanguage::Pt, "Shoes") => "Calçados",
        (AppLanguage::Es, "Shoes") => "Calzado",
        (AppLanguage::En, "Scalp") => "Scalp",
        (AppLanguage::Pt, "Scalp") => "Couro Cabeludo",
        (AppLanguage::Es, "Scalp") => "Cuero Cabelludo",
        (AppLanguage::En, "Face") => "Face",
        (AppLanguage::Pt, "Face") => "Rosto",
        (AppLanguage::Es, "Face") => "Rostro",
        (AppLanguage::En, "Facial Hair") => "Facial Hair",
        (AppLanguage::Pt, "Facial Hair") => "Pelos Faciais",
        (AppLanguage::Es, "Facial Hair") => "Vello Facial",
        (AppLanguage::En, "Eyebrows") => "Eyebrows",
        (AppLanguage::Pt, "Eyebrows") => "Sobrancelhas",
        (AppLanguage::Es, "Eyebrows") => "Cejas",
        (AppLanguage::En, "Eyes") => "Eyes",
        (AppLanguage::Pt, "Eyes") => "Olhos",
        (AppLanguage::Es, "Eyes") => "Ojos",
        (AppLanguage::En, "Body Hair") => "Body Hair",
        (AppLanguage::Pt, "Body Hair") => "Pelos Corporais",
        (AppLanguage::Es, "Body Hair") => "Vello Corporal",
        (AppLanguage::En, "Skin Details") => "Skin Details",
        (AppLanguage::Pt, "Skin Details") => "Detalhes da Pele",
        (AppLanguage::Es, "Skin Details") => "Detalles de la Piel",
        (AppLanguage::En, "Tattoos") => "Tattoos",
        (AppLanguage::Pt, "Tattoos") => "Tatuagens",
        (AppLanguage::Es, "Tattoos") => "Tatuajes",
        (AppLanguage::En, "Teeth") => "Teeth",
        (AppLanguage::Pt, "Teeth") => "Dentes",
        (AppLanguage::Es, "Teeth") => "Dientes",
        (AppLanguage::En, "Other") => "Other",
        (AppLanguage::Pt, "Other") => "Outros",
        (AppLanguage::Es, "Other") => "Otros",

        // Accessories / makeup specifics
        (AppLanguage::En, "Necklace") => "Necklace",
        (AppLanguage::Pt, "Necklace") => "Colar",
        (AppLanguage::Es, "Necklace") => "Collar",
        (AppLanguage::En, "Nose Ring") => "Nose Ring",
        (AppLanguage::Pt, "Nose Ring") => "Piercing de Nariz",
        (AppLanguage::Es, "Nose Ring") => "Piercing de Nariz",
        (AppLanguage::En, "Earrings") => "Earrings",
        (AppLanguage::Pt, "Earrings") => "Brincos",
        (AppLanguage::Es, "Earrings") => "Pendientes",
        (AppLanguage::En, "Glasses") => "Glasses",
        (AppLanguage::Pt, "Glasses") => "Óculos",
        (AppLanguage::Es, "Glasses") => "Gafas",
        (AppLanguage::En, "Bracelets") => "Bracelets",
        (AppLanguage::Pt, "Bracelets") => "Pulseiras",
        (AppLanguage::Es, "Bracelets") => "Pulseras",
        (AppLanguage::En, "Rings") => "Rings",
        (AppLanguage::Pt, "Rings") => "Anéis",
        (AppLanguage::Es, "Rings") => "Anillos",
        (AppLanguage::En, "Gloves") => "Gloves",
        (AppLanguage::Pt, "Gloves") => "Luvas",
        (AppLanguage::Es, "Gloves") => "Guantes",
        (AppLanguage::En, "Socks") => "Socks",
        (AppLanguage::Pt, "Socks") => "Meias",
        (AppLanguage::Es, "Socks") => "Calcetines",
        (AppLanguage::En, "Arm Band") => "Arm Band",
        (AppLanguage::Pt, "Arm Band") => "Braçadeira",
        (AppLanguage::Es, "Arm Band") => "Brazalete",
        (AppLanguage::En, "Garters") => "Garters",
        (AppLanguage::Pt, "Garters") => "Ligas",
        (AppLanguage::Es, "Garters") => "Ligas",
        (AppLanguage::En, "Lipstick") => "Lipstick",
        (AppLanguage::Pt, "Lipstick") => "Batom",
        (AppLanguage::Es, "Lipstick") => "Labial",
        (AppLanguage::En, "Eyeshadow") => "Eyeshadow",
        (AppLanguage::Pt, "Eyeshadow") => "Sombra",
        (AppLanguage::Es, "Eyeshadow") => "Sombra de Ojos",
        (AppLanguage::En, "Eyeliner") => "Eyeliner",
        (AppLanguage::Pt, "Eyeliner") => "Delineador",
        (AppLanguage::Es, "Eyeliner") => "Delineador",
        (AppLanguage::En, "Blush") => "Blush",
        (AppLanguage::Pt, "Blush") => "Blush",
        (AppLanguage::Es, "Blush") => "Rubor",
        (AppLanguage::En, "Mascara") => "Mascara",
        (AppLanguage::Pt, "Mascara") => "Rímel",
        (AppLanguage::Es, "Mascara") => "Máscara de Pestañas",

        // Gender/species
        (AppLanguage::En, "Male") => "Male",
        (AppLanguage::Pt, "Male") => "Masculino",
        (AppLanguage::Es, "Male") => "Masculino",
        (AppLanguage::En, "Female") => "Female",
        (AppLanguage::Pt, "Female") => "Feminino",
        (AppLanguage::Es, "Female") => "Femenino",
        (AppLanguage::En, "Unisex") => "Unisex",
        (AppLanguage::Pt, "Unisex") => "Unissex",
        (AppLanguage::Es, "Unisex") => "Unisex",
        (AppLanguage::En, "Unknown") => "Unknown",
        (AppLanguage::Pt, "Unknown") => "Desconhecido",
        (AppLanguage::Es, "Unknown") => "Desconocido",
        (AppLanguage::En, "Human") => "Human",
        (AppLanguage::Pt, "Human") => "Humano",
        (AppLanguage::Es, "Human") => "Humano",
        (AppLanguage::En, "Horse") => "Horse",
        (AppLanguage::Pt, "Horse") => "Cavalo",
        (AppLanguage::Es, "Horse") => "Caballo",
        (AppLanguage::En, "Cat") => "Cat",
        (AppLanguage::Pt, "Cat") => "Gato",
        (AppLanguage::Es, "Cat") => "Gato",
        (AppLanguage::En, "Dog") => "Dog",
        (AppLanguage::Pt, "Dog") => "Cachorro",
        (AppLanguage::Es, "Dog") => "Perro",
        (AppLanguage::En, "Little Dog") => "Little Dog",
        (AppLanguage::Pt, "Little Dog") => "Cachorro Pequeno",
        (AppLanguage::Es, "Little Dog") => "Perro Pequeño",
        (AppLanguage::En, "Deer") => "Deer",
        (AppLanguage::Pt, "Deer") => "Cervo",
        (AppLanguage::Es, "Deer") => "Ciervo",
        (AppLanguage::En, "Raccoon") => "Raccoon",
        (AppLanguage::Pt, "Raccoon") => "Guaxinim",
        (AppLanguage::Es, "Raccoon") => "Mapache",

        // Buy/build roots and main categories
        (AppLanguage::En, "Buy") => "Buy",
        (AppLanguage::Pt, "Buy") => "Compra",
        (AppLanguage::Es, "Buy") => "Compra",
        (AppLanguage::En, "Objects") => "Objects",
        (AppLanguage::Pt, "Objects") => "Objetos",
        (AppLanguage::Es, "Objects") => "Objetos",
        (AppLanguage::En, "Build") => "Build",
        (AppLanguage::Pt, "Build") => "Construção",
        (AppLanguage::Es, "Build") => "Construcción",
        (AppLanguage::En, "Appliances") => "Appliances",
        (AppLanguage::Pt, "Appliances") => "Eletrodomésticos",
        (AppLanguage::Es, "Appliances") => "Electrodomésticos",
        (AppLanguage::En, "Electronics") => "Electronics",
        (AppLanguage::Pt, "Electronics") => "Eletrônicos",
        (AppLanguage::Es, "Electronics") => "Electrónica",
        (AppLanguage::En, "Entertainment") => "Entertainment",
        (AppLanguage::Pt, "Entertainment") => "Entretenimento",
        (AppLanguage::Es, "Entertainment") => "Entretenimiento",
        (AppLanguage::En, "Lighting") => "Lighting",
        (AppLanguage::Pt, "Lighting") => "Iluminação",
        (AppLanguage::Es, "Lighting") => "Iluminación",
        (AppLanguage::En, "Plumbing") => "Plumbing",
        (AppLanguage::Pt, "Plumbing") => "Encanamento",
        (AppLanguage::Es, "Plumbing") => "Fontanería",
        (AppLanguage::En, "Decor") => "Decor",
        (AppLanguage::Pt, "Decor") => "Decoração",
        (AppLanguage::Es, "Decor") => "Decoración",
        (AppLanguage::En, "Children") => "Children",
        (AppLanguage::Pt, "Children") => "Infantil",
        (AppLanguage::Es, "Children") => "Infantil",
        (AppLanguage::En, "Storage") => "Storage",
        (AppLanguage::Pt, "Storage") => "Armazenamento",
        (AppLanguage::Es, "Storage") => "Almacenamiento",
        (AppLanguage::En, "Comfort") => "Comfort",
        (AppLanguage::Pt, "Comfort") => "Conforto",
        (AppLanguage::Es, "Comfort") => "Comodidad",
        (AppLanguage::En, "Surfaces") => "Surfaces",
        (AppLanguage::Pt, "Surfaces") => "Superfícies",
        (AppLanguage::Es, "Surfaces") => "Superficies",
        (AppLanguage::En, "Underwater Objects") => "Underwater Objects",
        (AppLanguage::Pt, "Underwater Objects") => "Objetos Subaquáticos",
        (AppLanguage::Es, "Underwater Objects") => "Objetos Subacuáticos",
        (AppLanguage::En, "Boats") => "Boats",
        (AppLanguage::Pt, "Boats") => "Barcos",
        (AppLanguage::Es, "Boats") => "Barcos",
        (AppLanguage::En, "Props") => "Props",
        (AppLanguage::Pt, "Props") => "Acessórios de Palco",
        (AppLanguage::Es, "Props") => "Accesorios de Escenario",
        (AppLanguage::En, "Vehicles") => "Vehicles",
        (AppLanguage::Pt, "Vehicles") => "Veículos",
        (AppLanguage::Es, "Vehicles") => "Vehículos",
        (AppLanguage::En, "Show Stage") => "Show Stage",
        (AppLanguage::Pt, "Show Stage") => "Palco de Show",
        (AppLanguage::Es, "Show Stage") => "Escenario",
        (_, "Resort") => "Resort",
        (_, "Debug") => "Debug",

        // Buy subcategories
        (AppLanguage::En, "Miscellaneous") => "Miscellaneous",
        (AppLanguage::Pt, "Miscellaneous") => "Diversos",
        (AppLanguage::Es, "Miscellaneous") => "Varios",
        (AppLanguage::En, "Small Appliances") => "Small Appliances",
        (AppLanguage::Pt, "Small Appliances") => "Pequenos Eletrodomésticos",
        (AppLanguage::Es, "Small Appliances") => "Pequeños Electrodomésticos",
        (AppLanguage::En, "Large Appliances") => "Large Appliances",
        (AppLanguage::Pt, "Large Appliances") => "Grandes Eletrodomésticos",
        (AppLanguage::Es, "Large Appliances") => "Grandes Electrodomésticos",
        (_, "TVs") => "TVs",
        (AppLanguage::En, "Audio") => "Audio",
        (AppLanguage::Pt, "Audio") => "Áudio",
        (AppLanguage::Es, "Audio") => "Audio",
        (AppLanguage::En, "Computers") => "Computers",
        (AppLanguage::Pt, "Computers") => "Computadores",
        (AppLanguage::Es, "Computers") => "Ordenadores",
        (AppLanguage::En, "Hobbies & Skills") => "Hobbies & Skills",
        (AppLanguage::Pt, "Hobbies & Skills") => "Hobbies e Habilidades",
        (AppLanguage::Es, "Hobbies & Skills") => "Aficiones y Habilidades",
        (AppLanguage::En, "Sports") => "Sports",
        (AppLanguage::Pt, "Sports") => "Esportes",
        (AppLanguage::Es, "Sports") => "Deportes",
        (AppLanguage::En, "Parties") => "Parties",
        (AppLanguage::Pt, "Parties") => "Festas",
        (AppLanguage::Es, "Parties") => "Fiestas",
        (AppLanguage::En, "Living Chairs") => "Living Chairs",
        (AppLanguage::Pt, "Living Chairs") => "Poltronas",
        (AppLanguage::Es, "Living Chairs") => "Sillones",
        (AppLanguage::En, "Lounge Chairs") => "Lounge Chairs",
        (AppLanguage::Pt, "Lounge Chairs") => "Espreguiçadeiras",
        (AppLanguage::Es, "Lounge Chairs") => "Tumbonas",
        (AppLanguage::En, "Dining Chairs") => "Dining Chairs",
        (AppLanguage::Pt, "Dining Chairs") => "Cadeiras de Jantar",
        (AppLanguage::Es, "Dining Chairs") => "Sillas de Comedor",
        (AppLanguage::En, "Sofas & Loveseats") => "Sofas & Loveseats",
        (AppLanguage::Pt, "Sofas & Loveseats") => "Sofás",
        (AppLanguage::Es, "Sofas & Loveseats") => "Sofás",
        (AppLanguage::En, "Ceiling Lights") => "Ceiling Lights",
        (AppLanguage::Pt, "Ceiling Lights") => "Luzes de Teto",
        (AppLanguage::Es, "Ceiling Lights") => "Luces de Techo",
        (AppLanguage::En, "Floor Lamps") => "Floor Lamps",
        (AppLanguage::Pt, "Floor Lamps") => "Luminárias de Piso",
        (AppLanguage::Es, "Floor Lamps") => "Lámparas de Pie",
        (AppLanguage::En, "Table Lamps") => "Table Lamps",
        (AppLanguage::Pt, "Table Lamps") => "Luminárias de Mesa",
        (AppLanguage::Es, "Table Lamps") => "Lámparas de Mesa",
        (AppLanguage::En, "Wall Lamps") => "Wall Lamps",
        (AppLanguage::Pt, "Wall Lamps") => "Luzes de Parede",
        (AppLanguage::Es, "Wall Lamps") => "Luces de Pared",
        (AppLanguage::En, "Outdoor Lights") => "Outdoor Lights",
        (AppLanguage::Pt, "Outdoor Lights") => "Iluminação Externa",
        (AppLanguage::Es, "Outdoor Lights") => "Iluminación Exterior",
        (AppLanguage::En, "Horses") => "Horses",
        (AppLanguage::Pt, "Horses") => "Cavalos",
        (AppLanguage::Es, "Horses") => "Caballos",
        (AppLanguage::En, "Dogs") => "Dogs",
        (AppLanguage::Pt, "Dogs") => "Cachorros",
        (AppLanguage::Es, "Dogs") => "Perros",
        (AppLanguage::En, "Cats") => "Cats",
        (AppLanguage::Pt, "Cats") => "Gatos",
        (AppLanguage::Es, "Cats") => "Gatos",
        (AppLanguage::En, "Beds") => "Beds",
        (AppLanguage::Pt, "Beds") => "Camas",
        (AppLanguage::Es, "Beds") => "Camas",
        (AppLanguage::En, "Sinks") => "Sinks",
        (AppLanguage::Pt, "Sinks") => "Pias",
        (AppLanguage::Es, "Sinks") => "Lavabos",
        (AppLanguage::En, "Toilets") => "Toilets",
        (AppLanguage::Pt, "Toilets") => "Vasos Sanitários",
        (AppLanguage::Es, "Toilets") => "Inodoros",
        (AppLanguage::En, "Showers & Tubs") => "Showers & Tubs",
        (AppLanguage::Pt, "Showers & Tubs") => "Chuveiros e Banheiras",
        (AppLanguage::Es, "Showers & Tubs") => "Duchas y Bañeras",
        (AppLanguage::En, "Sculptures") => "Sculptures",
        (AppLanguage::Pt, "Sculptures") => "Esculturas",
        (AppLanguage::Es, "Sculptures") => "Esculturas",
        (AppLanguage::En, "Paintings & Posters") => "Paintings & Posters",
        (AppLanguage::Pt, "Paintings & Posters") => "Quadros e Pôsteres",
        (AppLanguage::Es, "Paintings & Posters") => "Cuadros y Pósteres",
        (AppLanguage::En, "Plants") => "Plants",
        (AppLanguage::Pt, "Plants") => "Plantas",
        (AppLanguage::Es, "Plants") => "Plantas",
        (AppLanguage::En, "Mirrors") => "Mirrors",
        (AppLanguage::Pt, "Mirrors") => "Espelhos",
        (AppLanguage::Es, "Mirrors") => "Espejos",
        (AppLanguage::En, "Bookshelves") => "Bookshelves",
        (AppLanguage::Pt, "Bookshelves") => "Estantes de Livros",
        (AppLanguage::Es, "Bookshelves") => "Librerías",
        (AppLanguage::En, "Dressers") => "Dressers",
        (AppLanguage::Pt, "Dressers") => "Cômodas",
        (AppLanguage::Es, "Dressers") => "Cómodas",
        (AppLanguage::En, "Displays") => "Displays",
        (AppLanguage::Pt, "Displays") => "Expositores",
        (AppLanguage::Es, "Displays") => "Expositores",
        (AppLanguage::En, "Coffee Tables") => "Coffee Tables",
        (AppLanguage::Pt, "Coffee Tables") => "Mesas de Centro",
        (AppLanguage::Es, "Coffee Tables") => "Mesas de Centro",
        (AppLanguage::En, "Counters") => "Counters",
        (AppLanguage::Pt, "Counters") => "Balcões",
        (AppLanguage::Es, "Counters") => "Encimeras",
        (AppLanguage::En, "Desks") => "Desks",
        (AppLanguage::Pt, "Desks") => "Escrivaninhas",
        (AppLanguage::Es, "Desks") => "Escritorios",
        (AppLanguage::En, "End Tables") => "End Tables",
        (AppLanguage::Pt, "End Tables") => "Mesas Laterais",
        (AppLanguage::Es, "End Tables") => "Mesas Auxiliares",
        (AppLanguage::En, "Dining Tables") => "Dining Tables",
        (AppLanguage::Pt, "Dining Tables") => "Mesas de Jantar",
        (AppLanguage::Es, "Dining Tables") => "Mesas de Comedor",
        (AppLanguage::En, "Cabinets") => "Cabinets",
        (AppLanguage::Pt, "Cabinets") => "Armários",
        (AppLanguage::Es, "Cabinets") => "Armarios",
        (AppLanguage::En, "Furniture") => "Furniture",
        (AppLanguage::Pt, "Furniture") => "Móveis",
        (AppLanguage::Es, "Furniture") => "Muebles",
        (AppLanguage::En, "Toys") => "Toys",
        (AppLanguage::Pt, "Toys") => "Brinquedos",
        (AppLanguage::Es, "Toys") => "Juguetes",
        (AppLanguage::En, "Cars") => "Cars",
        (AppLanguage::Pt, "Cars") => "Carros",
        (AppLanguage::Es, "Cars") => "Coches",
        (AppLanguage::En, "Bicycles") => "Bicycles",
        (AppLanguage::Pt, "Bicycles") => "Bicicletas",
        (AppLanguage::Es, "Bicycles") => "Bicicletas",
        (AppLanguage::En, "Rugs") => "Rugs",
        (AppLanguage::Pt, "Rugs") => "Tapetes",
        (AppLanguage::Es, "Rugs") => "Alfombras",
        (AppLanguage::En, "Roof Decorations") => "Roof Decorations",
        (AppLanguage::Pt, "Roof Decorations") => "Decorações de Telhado",
        (AppLanguage::Es, "Roof Decorations") => "Decoraciones de Tejado",
        (AppLanguage::En, "Curtains & Blinds") => "Curtains & Blinds",
        (AppLanguage::Pt, "Curtains & Blinds") => "Cortinas e Persianas",
        (AppLanguage::Es, "Curtains & Blinds") => "Cortinas y Persianas",
        (AppLanguage::En, "Tomb Objects") => "Tomb Objects",
        (AppLanguage::Pt, "Tomb Objects") => "Objetos de Tumba",
        (AppLanguage::Es, "Tomb Objects") => "Objetos de Tumba",
        (AppLanguage::En, "Fish Spawners") => "Fish Spawners",
        (AppLanguage::Pt, "Fish Spawners") => "Geradores de Peixes",
        (AppLanguage::Es, "Fish Spawners") => "Generadores de Peces",
        (AppLanguage::En, "Plant & Seed Spawners") => "Plant & Seed Spawners",
        (AppLanguage::Pt, "Plant & Seed Spawners") => "Geradores de Plantas e Sementes",
        (AppLanguage::Es, "Plant & Seed Spawners") => "Generadores de Plantas y Semillas",
        (AppLanguage::En, "Rock, Gem & Metal Spawners") => "Rock, Gem & Metal Spawners",
        (AppLanguage::Pt, "Rock, Gem & Metal Spawners") => "Geradores de Rochas, Gemas e Metais",
        (AppLanguage::Es, "Rock, Gem & Metal Spawners") => "Generadores de Rocas, Gemas y Metales",
        (AppLanguage::En, "Insect Spawners") => "Insect Spawners",
        (AppLanguage::Pt, "Insect Spawners") => "Geradores de Insetos",
        (AppLanguage::Es, "Insect Spawners") => "Generadores de Insectos",

        // Build
        (AppLanguage::En, "Doors") => "Doors",
        (AppLanguage::Pt, "Doors") => "Portas",
        (AppLanguage::Es, "Doors") => "Puertas",
        (AppLanguage::En, "Windows") => "Windows",
        (AppLanguage::Pt, "Windows") => "Janelas",
        (AppLanguage::Es, "Windows") => "Ventanas",
        (AppLanguage::En, "Gates") => "Gates",
        (AppLanguage::Pt, "Gates") => "Portões",
        (AppLanguage::Es, "Gates") => "Portones",
        (AppLanguage::En, "Columns") => "Columns",
        (AppLanguage::Pt, "Columns") => "Colunas",
        (AppLanguage::Es, "Columns") => "Columnas",
        (AppLanguage::En, "Rabbit Holes") => "Rabbit Holes",
        (AppLanguage::Pt, "Rabbit Holes") => "Rabbit Holes",
        (AppLanguage::Es, "Rabbit Holes") => "Rabbit Holes",
        (AppLanguage::En, "Fireplaces") => "Fireplaces",
        (AppLanguage::Pt, "Fireplaces") => "Lareiras",
        (AppLanguage::Es, "Fireplaces") => "Chimeneas",
        (AppLanguage::En, "Chimneys") => "Chimneys",
        (AppLanguage::Pt, "Chimneys") => "Chaminés",
        (AppLanguage::Es, "Chimneys") => "Chimeneas",
        (AppLanguage::En, "Arches") => "Arches",
        (AppLanguage::Pt, "Arches") => "Arcos",
        (AppLanguage::Es, "Arches") => "Arcos",
        (AppLanguage::En, "Flowers") => "Flowers",
        (AppLanguage::Pt, "Flowers") => "Flores",
        (AppLanguage::Es, "Flowers") => "Flores",
        (AppLanguage::En, "Shrubs") => "Shrubs",
        (AppLanguage::Pt, "Shrubs") => "Arbustos",
        (AppLanguage::Es, "Shrubs") => "Arbustos",
        (AppLanguage::En, "Trees") => "Trees",
        (AppLanguage::Pt, "Trees") => "Árvores",
        (AppLanguage::Es, "Trees") => "Árboles",
        (AppLanguage::En, "Rocks") => "Rocks",
        (AppLanguage::Pt, "Rocks") => "Rochas",
        (AppLanguage::Es, "Rocks") => "Rocas",
        (AppLanguage::En, "Shells") => "Shells",
        (AppLanguage::Pt, "Shells") => "Estruturas",
        (AppLanguage::Es, "Shells") => "Estructuras",
        (AppLanguage::En, "Community Objects") => "Community Objects",
        (AppLanguage::Pt, "Community Objects") => "Objetos Comunitários",
        (AppLanguage::Es, "Community Objects") => "Objetos Comunitarios",
        (AppLanguage::En, "Elevators") => "Elevators",
        (AppLanguage::Pt, "Elevators") => "Elevadores",
        (AppLanguage::Es, "Elevators") => "Ascensores",
        (AppLanguage::En, "Spiral Stairs") => "Spiral Stairs",
        (AppLanguage::Pt, "Spiral Stairs") => "Escadas em Espiral",
        (AppLanguage::Es, "Spiral Stairs") => "Escaleras de Caracol",
        (AppLanguage::En, "Blueprints") => "Blueprints",
        (AppLanguage::Pt, "Blueprints") => "Plantas de Construção",
        (AppLanguage::Es, "Blueprints") => "Planos",
        (AppLanguage::En, "Resort Objects") => "Resort Objects",
        (AppLanguage::Pt, "Resort Objects") => "Objetos de Resort",
        (AppLanguage::Es, "Resort Objects") => "Objetos de Resort",
        (AppLanguage::En, "Modular Arches") => "Modular Arches",
        (AppLanguage::Pt, "Modular Arches") => "Arcos Modulares",
        (AppLanguage::Es, "Modular Arches") => "Arcos Modulares",

        _ => "Unknown",
    }
}

fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    let bytes = data.get(offset..offset + 4)?;
    Some(u32::from_le_bytes(bytes.try_into().ok()?))
}

fn read_i32(data: &[u8], offset: usize) -> Option<i32> {
    let bytes = data.get(offset..offset + 4)?;
    Some(i32::from_le_bytes(bytes.try_into().ok()?))
}

fn read_be_7bit_string(data: &[u8], offset: &mut usize) -> Option<String> {
    let mut byte_len = 0usize;
    let mut shift = 0usize;
    loop {
        if shift > 56 {
            return None;
        }
        let byte = *data.get(*offset)?;
        *offset += 1;
        byte_len |= ((byte & 0x7F) as usize) << shift;
        if (byte & 0x80) == 0 {
            break;
        }
        shift += 7;
    }

    if byte_len == 0 {
        return Some(String::new());
    }
    if byte_len % 2 != 0 {
        return None;
    }

    let bytes = data.get(*offset..offset.checked_add(byte_len)?)?;
    *offset += byte_len;
    let words = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    String::from_utf16(&words).ok()
}

fn parse_casp_core(data: &[u8]) -> Option<CaspCore> {
    if data.len() < 12 {
        return None;
    }

    let mut offset = 8usize;
    let preset_count = read_u32(data, offset)? as usize;
    offset += 4;
    if preset_count > 4096 {
        return None;
    }

    for _ in 0..preset_count {
        let xml_len = read_i32(data, offset)?;
        offset += 4;
        if xml_len < 0 || xml_len > 1_000_000 {
            return None;
        }
        let xml_bytes = (xml_len as usize).checked_mul(2)?;
        offset = offset.checked_add(xml_bytes)?.checked_add(4)?;
        if offset > data.len() {
            return None;
        }
    }

    let _name = read_be_7bit_string(data, &mut offset)?;
    offset = offset.checked_add(4)?; // sort priority
    offset = offset.checked_add(1)?; // unknown byte

    let clothing_type = read_u32(data, offset)?;
    offset += 4;
    let type_flags = read_u32(data, offset)?;
    offset += 4;
    let age_species_gender = read_u32(data, offset)?;
    offset += 4;
    let clothing_category = read_u32(data, offset)?;

    Some(CaspCore {
        clothing_type,
        type_flags,
        age_species_gender,
        clothing_category,
    })
}

fn casp_age_label(value: u32, language: AppLanguage) -> String {
    let bits = value & 0x7F;
    let labels = [
        (0x01, language.baby()),
        (0x02, language.toddler()),
        (0x04, language.child()),
        (0x08, language.teen()),
        (0x10, match language {
            AppLanguage::En => "Young Adult",
            AppLanguage::Pt => "Jovem Adulto",
            AppLanguage::Es => "Adulto Joven",
        }),
        (0x20, match language {
            AppLanguage::En => "Adult",
            AppLanguage::Pt => "Adulto",
            AppLanguage::Es => "Adulto",
        }),
        (0x40, language.elder()),
    ];

    let selected = labels
        .into_iter()
        .enumerate()
        .filter_map(|(index, (flag, label))| ((bits & flag) != 0).then_some((index, label)))
        .collect::<Vec<_>>();

    if selected.is_empty() {
        language.unknown().to_string()
    } else if selected.len() == 1 {
        selected[0].1.to_string()
    } else {
        let contiguous = selected
            .windows(2)
            .all(|pair| pair[1].0 == pair[0].0 + 1);
        let conjunction = match language {
            AppLanguage::En => if contiguous { " to " } else { " and " },
            AppLanguage::Pt => if contiguous { " a " } else { " e " },
            AppLanguage::Es => if contiguous { " a " } else { " y " },
        };
        if contiguous {
            format!("{}{}", selected.first().unwrap().1, conjunction) + selected.last().unwrap().1
        } else {
            selected
                .iter()
                .map(|(_, label)| *label)
                .collect::<Vec<_>>()
                .join(conjunction)
        }
    }
}

fn casp_gender_label(value: u32, language: AppLanguage) -> String {
    match (value >> 12) & 0x3 {
        0x1 => language.male().to_string(),
        0x2 => language.female().to_string(),
        0x3 => language.unisex().to_string(),
        _ => language.unknown().to_string(),
    }
}

fn casp_species_label(value: u32, clothing_type: u32, language: AppLanguage) -> Option<String> {
    let species = (value >> 8) & 0xF;
    let key = match species {
        0x1 => Some("Human"),
        0x2 => Some("Horse"),
        0x3 => Some("Cat"),
        0x4 => Some("Dog"),
        0x5 => Some("Little Dog"),
        0x6 => Some("Deer"),
        0x7 => Some("Raccoon"),
        // Some human CASP versions leave the species nibble at zero.  Treat
        // that value as human only when the same field also contains a valid
        // human age/gender combination and the clothing type is not a pet
        // type.  Other zero/unknown combinations remain undisclosed instead
        // of being presented as an invented species.
        0x0 if (value & 0x7F) != 0
            && ((value >> 12) & 0x3) != 0
            && !(0x2F..=0x3B).contains(&clothing_type) => Some("Human"),
        _ => None,
    };
    key.map(|value| tr(language, value).to_string())
}

fn casp_usage_categories(flags: u32, language: AppLanguage) -> Vec<String> {
    let definitions = [
        (0x0000_0001, "Naked"),
        (0x0000_0002, "Everyday"),
        (0x0000_0004, "Formalwear"),
        (0x0000_0008, "Sleepwear"),
        (0x0000_0010, "Swimwear"),
        (0x0000_0020, "Athletic"),
        (0x0000_0040, "Singed"),
        (0x0000_0080, "Martial Arts"),
        (0x0000_0100, "Career"),
        (0x0000_0200, "Firefighting"),
        (0x0000_0400, "Makeover"),
        (0x0001_0000, "Child Imagination"),
        (0x0002_0000, "Special"),
        (0x0004_0000, "Outerwear"),
        (0x0008_0000, "Supernatural"),
    ];

    definitions
        .into_iter()
        .filter(|(flag, _)| (flags & *flag) != 0)
        .map(|(_, key)| localize_usage(language, key))
        .collect()
}

fn localize_usage(language: AppLanguage, key: &str) -> String {
    let value = match (language, key) {
        (AppLanguage::Pt, "Naked") => "Nu",
        (AppLanguage::Es, "Naked") => "Desnudo",
        (AppLanguage::Pt, "Everyday") => "Cotidiano",
        (AppLanguage::Es, "Everyday") => "Diario",
        (AppLanguage::Pt, "Formalwear") => "Formal",
        (AppLanguage::Es, "Formalwear") => "Formal",
        (AppLanguage::Pt, "Sleepwear") => "Roupa de Dormir",
        (AppLanguage::Es, "Sleepwear") => "Ropa de Dormir",
        (AppLanguage::Pt, "Swimwear") => "Roupa de Banho",
        (AppLanguage::Es, "Swimwear") => "Traje de Baño",
        (AppLanguage::Pt, "Athletic") => "Atlético",
        (AppLanguage::Es, "Athletic") => "Deportivo",
        (AppLanguage::Pt, "Singed") => "Chamuscado",
        (AppLanguage::Es, "Singed") => "Chamuscado",
        (AppLanguage::Pt, "Martial Arts") => "Artes Marciais",
        (AppLanguage::Es, "Martial Arts") => "Artes Marciales",
        (AppLanguage::Pt, "Career") => "Carreira",
        (AppLanguage::Es, "Career") => "Profesión",
        (AppLanguage::Pt, "Firefighting") => "Bombeiro",
        (AppLanguage::Es, "Firefighting") => "Bombero",
        (AppLanguage::Pt, "Makeover") => "Transformação",
        (AppLanguage::Es, "Makeover") => "Cambio de Imagen",
        (AppLanguage::Pt, "Child Imagination") => "Imaginação Infantil",
        (AppLanguage::Es, "Child Imagination") => "Imaginación Infantil",
        (AppLanguage::Pt, "Special") => "Especial",
        (AppLanguage::Es, "Special") => "Especial",
        (AppLanguage::Pt, "Outerwear") => "Roupa de Frio",
        (AppLanguage::Es, "Outerwear") => "Ropa de Abrigo",
        (AppLanguage::Pt, "Supernatural") => "Sobrenatural",
        (AppLanguage::Es, "Supernatural") => "Sobrenatural",
        _ => key,
    };
    value.to_string()
}

fn casp_category(core: CaspCore) -> (&'static str, Option<&'static str>) {
    match core.clothing_type {
        0x01 => ("Hair", None),
        0x02 => ("Hair", Some("Scalp")),
        0x03 => ("Genetics", Some("Face")),
        0x04 => ("Clothing", Some("Outfit")),
        0x05 => ("Clothing", Some("Top")),
        0x06 => ("Clothing", Some("Bottom")),
        0x07 => ("Clothing", Some("Shoes")),
        0x08 => ("Accessories", Some("Other")),
        0x09 => ("Accessories", Some("Necklace")),
        0x0A => ("Accessories", Some("Nose Ring")),
        0x0B | 0x1E | 0x1F => ("Accessories", Some("Earrings")),
        0x0C => ("Accessories", Some("Glasses")),
        0x0D => ("Accessories", Some("Bracelets")),
        0x0E | 0x0F => ("Accessories", Some("Rings")),
        0x10 => ("Hair", Some("Facial Hair")),
        0x11 => ("Makeup", Some("Lipstick")),
        0x12 => ("Makeup", Some("Eyeshadow")),
        0x13 => ("Makeup", Some("Eyeliner")),
        0x14 => ("Makeup", Some("Blush")),
        0x15 => ("Makeup", Some("Other")),
        0x16 => ("Genetics", Some("Eyebrows")),
        0x17 => ("Genetics", Some("Eyes")),
        0x18 => ("Accessories", Some("Gloves")),
        0x19 => ("Accessories", Some("Socks")),
        0x1A => ("Makeup", Some("Mascara")),
        0x1B | 0x1C | 0x1D | 0x26 => ("Makeup", Some("Skin Details")),
        0x20 => ("Accessories", Some("Arm Band")),
        0x21 | 0x22 => ("Makeup", Some("Tattoos")),
        0x23 => ("Genetics", Some("Teeth")),
        0x24 | 0x25 => ("Accessories", Some("Garters")),
        // BodyHairChestStomach and the related CASP values are one approved
        // Body Hair family. Keep only the gender level physically; the
        // individual body region and age flags remain metadata.
        0x27..=0x2E => ("Body Hair", None),
        0x2F..=0x3B => ("Pets", Some("Other")),
        _ if (core.type_flags & 0x10) != 0 => ("Accessories", Some("Other")),
        _ if (core.type_flags & 0x01) != 0 => ("Hair", None),
        _ if (core.type_flags & 0x08) != 0 => ("Clothing", Some("Other")),
        _ => ("Unknown", None),
    }
}

fn body_hair_root(language: AppLanguage) -> &'static str {
    match language {
        // The Portuguese manual model has a shared hair/fur container, with
        // Body Hair as the next physical level. English keeps the explicit
        // Body Hair name requested by the localized taxonomy.
        AppLanguage::En => "Body Hair",
        AppLanguage::Pt => "Cabelos e Pêlos",
        AppLanguage::Es => "Cabello y Pelo",
    }
}

pub fn classify_casp(data: &[u8], language: AppLanguage) -> Option<CatalogClassification> {
    let core = parse_casp_core(data)?;
    let (main_key, sub_key) = casp_category(core);

    let main_category = tr(language, main_key).to_string();
    let sub_category = sub_key.map(|key| tr(language, key).to_string());
    let age = casp_age_label(core.age_species_gender, language);
    let gender = casp_gender_label(core.age_species_gender, language);
    let species = casp_species_label(core.age_species_gender, core.clothing_type, language);

    let mut folder_parts = if main_key == "Body Hair" {
        vec![
            body_hair_root(language).to_string(),
            main_category.clone(),
        ]
    } else {
        vec![tr(language, "CAS").to_string(), main_category.clone()]
    };
    if main_key != "Pets" {
        folder_parts.push(gender.clone());
        // Accessories retain gender and optional accessory subtype, but
        // age flags remain available as metadata rather than folder levels.
        // Facial hair does not receive an age folder. Body Hair is a primary
        // category and therefore also has only its gender level physically.
        let age_is_physical = !matches!(sub_key, Some("Facial Hair")) && main_key != "Body Hair";
        if main_key != "Accessories" && age_is_physical {
            folder_parts.push(age.clone());
        }
    }
    if let Some(sub) = &sub_category {
        folder_parts.push(sub.clone());
    }

    let technical_reason = format!(
        "CASP clothingType=0x{:08X}; typeFlags=0x{:08X}; ageSpeciesGender=0x{:08X}; clothingCategory=0x{:08X} => {}",
        core.clothing_type,
        core.type_flags,
        core.age_species_gender,
        core.clothing_category,
        folder_parts.join("\\")
    );

    Some(CatalogClassification {
        source: "CASP".to_string(),
        kind: "cas".to_string(),
        main_category,
        sub_category,
        gender: Some(gender),
        age: Some(age),
        species,
        usage_categories: casp_usage_categories(core.clothing_category, language),
        candidate_folder_parts: vec![folder_parts.clone()],
        folder_parts,
        ambiguous: false,
        technical_reason,
    })
}

fn read_7bit_encoded_usize(cur: &mut Cursor<&[u8]>) -> Option<usize> {
    let mut result = 0usize;
    let mut shift = 0usize;
    loop {
        if shift >= usize::BITS as usize {
            return None;
        }
        let byte = cur.read_u8().ok()?;
        result |= ((byte & 0x7F) as usize) << shift;
        if (byte & 0x80) == 0 {
            return Some(result);
        }
        shift += 7;
    }
}

fn read_be_unicode_string(cur: &mut Cursor<&[u8]>) -> Option<String> {
    let byte_len = read_7bit_encoded_usize(cur)?;
    if byte_len == 0 {
        return Some(String::new());
    }
    if byte_len % 2 != 0 {
        return None;
    }

    let mut bytes = vec![0u8; byte_len];
    cur.read_exact(&mut bytes).ok()?;
    let words = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    String::from_utf16(&words).ok()
}

fn skip_objd_material_list(cur: &mut Cursor<&[u8]>) -> Option<()> {
    let count = cur.read_i32::<LittleEndian>().ok()?;
    if count < 0 || count > 100_000 {
        return None;
    }

    for _ in 0..count {
        let material_type = cur.read_u8().ok()?;
        if material_type != 1 {
            cur.read_u32::<LittleEndian>().ok()?;
        }

        let after_rel = cur.position().checked_add(4)?;
        let rel_end = cur.read_u32::<LittleEndian>().ok()? as u64;
        let material_end = after_rel.checked_add(rel_end)?;

        cur.read_u16::<LittleEndian>().ok()?;
        cur.read_u32::<LittleEndian>().ok()?;
        cur.read_u32::<LittleEndian>().ok()?;

        if material_end < cur.position() || material_end > cur.get_ref().len() as u64 {
            return None;
        }
        cur.set_position(material_end);
        cur.read_u32::<LittleEndian>().ok()?;
    }
    Some(())
}

fn skip_catalog_common(cur: &mut Cursor<&[u8]>) -> Option<()> {
    let common_version = cur.read_u32::<LittleEndian>().ok()?;
    cur.read_u64::<LittleEndian>().ok()?; // name STBL guid
    cur.read_u64::<LittleEndian>().ok()?; // description STBL guid
    let _ = read_be_unicode_string(cur)?; // internal name
    let _ = read_be_unicode_string(cur)?; // internal description
    cur.read_f32::<LittleEndian>().ok()?; // price
    cur.read_f32::<LittleEndian>().ok()?; // niceness
    cur.read_f32::<LittleEndian>().ok()?; // crap score
    cur.read_u8().ok()?; // product status
    cur.read_u64::<LittleEndian>().ok()?; // icon iid
    cur.read_u8().ok()?;
    cur.read_f32::<LittleEndian>().ok()?; // environment
    cur.read_u32::<LittleEndian>().ok()?; // fire type
    cur.read_u8().ok()?; // stealable
    cur.read_u8().ok()?; // repossessable
    cur.read_u32::<LittleEndian>().ok()?; // ui sort

    if common_version >= 0x0D {
        cur.read_u8().ok()?;
    }
    if common_version >= 0x0E {
        cur.read_u8().ok()?;
    }
    if common_version >= 0x0F {
        cur.read_u32::<LittleEndian>().ok()?;
    }
    Some(())
}

fn parse_objd_flags(data: &[u8]) -> Option<ObjdCatalogFlags> {
    let mut cur = Cursor::new(data);
    let version = cur.read_u32::<LittleEndian>().ok()?;
    cur.read_u32::<LittleEndian>().ok()?; // TGI offset
    cur.read_u32::<LittleEndian>().ok()?; // TGI size

    skip_objd_material_list(&mut cur)?;
    if version >= 0x16 {
        let _ = read_be_unicode_string(&mut cur)?;
    }
    skip_catalog_common(&mut cur)?;

    cur.read_u32::<LittleEndian>().ok()?; // OBJK index
    let object_type_flags = cur.read_u32::<LittleEndian>().ok()?;
    if version >= 0x1A {
        cur.read_u32::<LittleEndian>().ok()?;
    }

    cur.read_u32::<LittleEndian>().ok()?; // wall placement
    cur.read_u32::<LittleEndian>().ok()?; // movement
    cur.read_u32::<LittleEndian>().ok()?; // wall cutout tiles
    cur.read_u32::<LittleEndian>().ok()?; // levels

    let wallmask_count = cur.read_u8().ok()? as usize;
    for _ in 0..wallmask_count {
        for _ in 0..4 {
            cur.read_f32::<LittleEndian>().ok()?;
        }
        cur.read_u32::<LittleEndian>().ok()?;
        cur.read_u32::<LittleEndian>().ok()?;
    }

    cur.read_u8().ok()?; // script enabled
    cur.read_u32::<LittleEndian>().ok()?; // diagonal objd index
    cur.read_u32::<LittleEndian>().ok()?; // ambience hash

    let room_flags = cur.read_u32::<LittleEndian>().ok()?;
    let function_category_flags = cur.read_u32::<LittleEndian>().ok()?;
    let sub_category1_flags = cur.read_u64::<LittleEndian>().ok()?;
    let sub_category2_flags = if version >= 0x1C {
        cur.read_u64::<LittleEndian>().ok()?
    } else {
        0
    };
    let room_sub_category_flags = cur.read_u64::<LittleEndian>().ok()?;
    let build_category_flags = cur.read_u32::<LittleEndian>().ok()?;

    Some(ObjdCatalogFlags {
        object_type_flags,
        room_flags,
        room_sub_category_flags,
        function_category_flags,
        sub_category1_flags,
        sub_category2_flags,
        build_category_flags,
    })
}

fn decode_build_all(flags: ObjdCatalogFlags) -> Vec<&'static str> {
    [
        (0x0000_0002, "Doors"),
        (0x0000_0004, "Windows"),
        (0x0000_0008, "Gates"),
        (0x0000_0010, "Columns"),
        (0x0000_0020, "Rabbit Holes"),
        (0x0000_0040, "Fireplaces"),
        (0x0000_0080, "Chimneys"),
        (0x0000_0100, "Arches"),
        (0x0000_0200, "Flowers"),
        (0x0000_0400, "Shrubs"),
        (0x0000_0800, "Trees"),
        (0x0000_1000, "Rugs"),
        (0x0000_2000, "Rocks"),
        (0x0000_4000, "Shells"),
        (0x0000_8000, "Community Objects"),
        (0x0001_0000, "Elevators"),
        (0x0002_0000, "Spiral Stairs"),
        (0x1000_0000, "Blueprints"),
        (0x2000_0000, "Resort Objects"),
        (0x4000_0000, "Modular Arches"),
    ]
    .into_iter()
    .filter_map(|(flag, label)| ((flags.build_category_flags & flag) != 0).then_some(label))
    .collect()
}

fn decode_buy_main_all(flags: u32) -> Vec<&'static str> {
    [
        (0x0000_0002, "Appliances"),
        (0x0000_0004, "Electronics"),
        (0x0000_0008, "Entertainment"),
        (0x0000_0020, "Lighting"),
        (0x0000_0040, "Plumbing"),
        (0x0000_0080, "Decor"),
        (0x0000_0100, "Children"),
        (0x0000_0200, "Storage"),
        (0x0000_0800, "Comfort"),
        (0x0000_1000, "Surfaces"),
        (0x0000_2000, "Vehicles"),
        (0x0000_4000, "Pets"),
        (0x0000_8000, "Show Stage"),
        (0x0001_0000, "Resort"),
        (0x4000_0000, "Debug"),
    ]
    .into_iter()
    .filter_map(|(flag, label)| ((flags & flag) != 0).then_some(label))
    .collect()
}

fn decode_buy_sub_all(main: &str, sub1: u64, sub2: u64) -> Vec<&'static str> {
    let candidates: &[(u64, &str)] = match main {
        "Appliances" => &[
            (0x0000_0000_0000_0002, "Miscellaneous"),
            (0x0000_0000_0000_0004, "Small Appliances"),
            (0x0000_0000_0000_0008, "Large Appliances"),
        ],
        "Electronics" => &[
            (0x0000_0000_0000_0080, "TVs"),
            (0x0000_0000_0000_0100, "Miscellaneous"),
            (0x0000_0000_0000_0400, "Audio"),
            (0x0000_0000_0000_0800, "Computers"),
        ],
        "Entertainment" => &[
            (0x0000_0000_0000_1000, "Hobbies & Skills"),
            (0x0000_0000_0000_2000, "Sports"),
            (0x0000_0000_0002_0000, "Parties"),
            (0x0000_0000_0004_0000, "Miscellaneous"),
        ],
        "Comfort" => &[
            (0x0000_0000_0000_4000, "Living Chairs"),
            (0x0000_0000_0100_0000, "Lounge Chairs"),
            (0x0000_0040_0000_0000, "Dining Chairs"),
            (0x0000_0080_0000_0000, "Sofas & Loveseats"),
            (0x0000_0100_0000_0000, "Miscellaneous"),
            (0x0000_0400_0000_0000, "Beds"),
        ],
        "Lighting" => &[
            (0x0000_0000_0008_0000, "Ceiling Lights"),
            (0x0000_0000_0010_0000, "Floor Lamps"),
            (0x0000_0000_0020_0000, "Table Lamps"),
            (0x0000_0000_0040_0000, "Wall Lamps"),
            (0x0000_0000_0080_0000, "Outdoor Lights"),
            (0x0100_0000_0000_0000, "Miscellaneous"),
        ],
        "Plumbing" => &[
            (0x0000_0000_0200_0000, "Sinks"),
            (0x0000_0000_0400_0000, "Toilets"),
            (0x0000_0000_0800_0000, "Showers & Tubs"),
            (0x0200_0000_0000_0000, "Miscellaneous"),
        ],
        "Decor" => &[
            (0x0000_0000_1000_0000, "Miscellaneous"),
            (0x0000_0000_2000_0000, "Sculptures"),
            (0x0000_0000_4000_0000, "Paintings & Posters"),
            (0x0000_0000_8000_0000, "Plants"),
            (0x0000_0001_0000_0000, "Mirrors"),
            (0x0000_0200_0000_0000, "Roof Decorations"),
            (0x0040_0000_0000_0000, "Curtains & Blinds"),
            (0x2000_0000_0000_0000, "Rugs"),
        ],
        "Storage" => &[
            (0x0000_0008_0000_0000, "Bookshelves"),
            (0x0000_0020_0000_0000, "Dressers"),
            (0x0400_0000_0000_0000, "Miscellaneous"),
        ],
        "Surfaces" => &[
            (0x0000_0010_0000_0000, "Displays"),
            (0x0000_1000_0000_0000, "Coffee Tables"),
            (0x0000_2000_0000_0000, "Counters"),
            (0x0000_4000_0000_0000, "Desks"),
            (0x0000_8000_0000_0000, "End Tables"),
            (0x0001_0000_0000_0000, "Dining Tables"),
            (0x0020_0000_0000_0000, "Cabinets"),
            (0x0800_0000_0000_0000, "Miscellaneous"),
        ],
        "Children" => &[
            (0x0002_0000_0000_0000, "Furniture"),
            (0x0004_0000_0000_0000, "Toys"),
            (0x0080_0000_0000_0000, "Miscellaneous"),
        ],
        "Vehicles" => &[
            (0x0008_0000_0000_0000, "Cars"),
            (0x0010_0000_0000_0000, "Bicycles"),
            (0x1000_0000_0000_0000, "Miscellaneous"),
        ],
        "Pets" => &[
            (0x0000_0000_0000_8000, "Horses"),
            (0x0000_0002_0000_0000, "Dogs"),
            (0x4000_0000_0000_0000, "Cats"),
        ],
        "Debug" => &[
            (0x0000_0000_0000_0010, "Tomb Objects"),
            (0x0000_0000_0000_0020, "Fish Spawners"),
            (0x0000_0000_0000_0040, "Plant & Seed Spawners"),
            (0x0000_0000_0000_0200, "Rock, Gem & Metal Spawners"),
            (0x0000_0000_0001_0000, "Insect Spawners"),
            (0x0000_0004_0000_0000, "Miscellaneous"),
        ],
        _ => &[],
    };

    let mut found = candidates
        .iter()
        .filter_map(|(flag, label)| ((sub1 & *flag) != 0).then_some(*label))
        .collect::<Vec<_>>();

    match main {
        "Show Stage" => {
            if (sub2 & 0x2) != 0 { found.push("Lighting"); }
            if (sub2 & 0x4) != 0 { found.push("Props"); }
            if (sub2 & 0x8) != 0 { found.push("Miscellaneous"); }
        }
        "Resort" => {
            if (sub2 & 0x10) != 0 { found.push("Underwater Objects"); }
            if (sub2 & 0x20) != 0 { found.push("Miscellaneous"); }
            if (sub2 & 0x40) != 0 { found.push("Boats"); }
        }
        _ => {}
    }

    found
}

pub fn classify_objd(data: &[u8], language: AppLanguage) -> Option<CatalogClassification> {
    let flags = parse_objd_flags(data)?;

    let build_options = decode_build_all(flags);
    if !build_options.is_empty() {
        let root = tr(language, "Build").to_string();
        let candidate_folder_parts = build_options
            .iter()
            .map(|sub| vec![root.clone(), tr(language, sub).to_string()])
            .collect::<Vec<_>>();
        let ambiguous = candidate_folder_parts.len() != 1;
        let folder_parts = if ambiguous {
            Vec::new()
        } else {
            candidate_folder_parts[0].clone()
        };
        let sub_category = if ambiguous {
            None
        } else {
            Some(tr(language, build_options[0]).to_string())
        };

        let technical_reason = format!(
            "OBJD buildCategory=0x{:08X}; functionCategory=0x{:08X}; subCategory1=0x{:016X}; subCategory2=0x{:016X} => {}",
            flags.build_category_flags,
            flags.function_category_flags,
            flags.sub_category1_flags,
            flags.sub_category2_flags,
            if folder_parts.is_empty() {
                candidate_folder_parts
                    .iter()
                    .map(|parts| parts.join("\\"))
                    .collect::<Vec<_>>()
                    .join(" | ")
            } else {
                folder_parts.join("\\")
            }
        );

        return Some(CatalogClassification {
            source: "OBJD".to_string(),
            kind: "build".to_string(),
            main_category: root,
            sub_category,
            gender: None,
            age: None,
            species: None,
            usage_categories: Vec::new(),
            folder_parts,
            candidate_folder_parts,
            ambiguous,
            technical_reason,
        });
    }

    let buy_options = decode_buy_main_all(flags.function_category_flags);
    if buy_options.is_empty() {
        return None;
    }

    let root = tr(language, "Objects").to_string();
    let mut candidate_folder_parts = Vec::new();

    for main in &buy_options {
        let main_label = tr(language, main).to_string();
        let sub_options = decode_buy_sub_all(
            main,
            flags.sub_category1_flags,
            flags.sub_category2_flags,
        );

        if sub_options.is_empty() {
            candidate_folder_parts.push(vec![root.clone(), main_label]);
        } else {
            for sub in sub_options {
                candidate_folder_parts.push(vec![
                    root.clone(),
                    main_label.clone(),
                    tr(language, sub).to_string(),
                ]);
            }
        }
    }

    candidate_folder_parts.sort();
    candidate_folder_parts.dedup();

    let ambiguous = candidate_folder_parts.len() != 1;
    let folder_parts = if ambiguous {
        Vec::new()
    } else {
        candidate_folder_parts[0].clone()
    };

    let main_category = if buy_options.len() == 1 {
        tr(language, buy_options[0]).to_string()
    } else {
        buy_options
            .iter()
            .map(|value| tr(language, value))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let sub_category = if !ambiguous && folder_parts.len() >= 3 {
        Some(folder_parts[2].clone())
    } else {
        None
    };

    let technical_reason = format!(
        "OBJD functionCategory=0x{:08X}; subCategory1=0x{:016X}; subCategory2=0x{:016X}; roomSubCategory=0x{:016X}; buildCategory=0x{:08X} => {}",
        flags.function_category_flags,
        flags.sub_category1_flags,
        flags.sub_category2_flags,
        flags.room_sub_category_flags,
        flags.build_category_flags,
        if folder_parts.is_empty() {
            candidate_folder_parts
                .iter()
                .map(|parts| parts.join("\\"))
                .collect::<Vec<_>>()
                .join(" | ")
        } else {
            folder_parts.join("\\")
        }
    );

    Some(CatalogClassification {
        source: "OBJD".to_string(),
        kind: "buy".to_string(),
        main_category,
        sub_category,
        gender: None,
        age: None,
        species: None,
        usage_categories: {
            let mut usage = room_usage(flags.room_flags, language);
            usage.extend(room_subcategory_usage(flags.room_sub_category_flags, language));
            usage
        },
        folder_parts,
        candidate_folder_parts,
        ambiguous,
        technical_reason,
    })
}

fn room_subcategory_usage(flags: u64, language: AppLanguage) -> Vec<String> {
    let definitions: &[(u64, &str, &str, &str)] = &[
        (0x0000_0000_0000_0002, "Dishwashers", "Lava-louças", "Lavavajillas"),
        (0x0000_0000_0000_0004, "Small Appliances", "Pequenos Eletrodomésticos", "Pequeños Electrodomésticos"),
        (0x0000_0000_0000_0008, "Refrigerators", "Geladeiras", "Refrigeradores"),
        (0x0000_0000_0000_0010, "Trash", "Lixo", "Basura"),
        (0x0000_0000_0000_0020, "Alarms", "Alarmes", "Alarmas"),
        (0x0000_0000_0000_0040, "Phones", "Telefones", "Teléfonos"),
        (0x0000_0000_0000_0080, "TVs", "TVs", "TVs"),
        (0x0000_0000_0000_0100, "Smoke Alarms", "Detectores de Fumaça", "Detectores de Humo"),
        (0x0000_0000_0000_0400, "Audio", "Áudio", "Audio"),
        (0x0000_0000_0000_0800, "Computers", "Computadores", "Ordenadores"),
        (0x0000_0000_0000_1000, "Hobbies & Skills", "Hobbies e Habilidades", "Aficiones y Habilidades"),
        (0x0000_0000_0000_2000, "Indoor Activities", "Atividades Internas", "Actividades de Interior"),
        (0x0000_0000_0000_4000, "Living Chairs", "Poltronas", "Sillones"),
        (0x0000_0000_0000_8000, "Office Chairs", "Cadeiras de Escritório", "Sillas de Oficina"),
        (0x0000_0000_0001_0000, "Stoves", "Fogões", "Cocinas"),
        (0x0000_0000_0002_0000, "Eating Out", "Refeições Fora", "Comer Fuera"),
        (0x0000_0000_0004_0000, "Outdoor Activities", "Atividades Externas", "Actividades al Aire Libre"),
        (0x0000_0000_0008_0000, "Ceiling Lights", "Luzes de Teto", "Luces de Techo"),
        (0x0000_0000_0010_0000, "Floor Lamps", "Luminárias de Piso", "Lámparas de Pie"),
        (0x0000_0000_0020_0000, "Table Lamps", "Luminárias de Mesa", "Lámparas de Mesa"),
        (0x0000_0000_0040_0000, "Wall Lamps", "Luzes de Parede", "Luces de Pared"),
        (0x0000_0000_0080_0000, "Outdoor Lights", "Iluminação Externa", "Iluminación Exterior"),
        (0x0000_0000_0100_0000, "Showers", "Chuveiros", "Duchas"),
        (0x0000_0000_0200_0000, "Sinks", "Pias", "Lavabos"),
        (0x0000_0000_0400_0000, "Toilets", "Vasos Sanitários", "Inodoros"),
        (0x0000_0000_0800_0000, "Tubs", "Banheiras", "Bañeras"),
        (0x0000_0000_1000_0000, "Accents", "Acessórios Decorativos", "Detalles Decorativos"),
        (0x0000_0000_2000_0000, "Lawn Decor", "Decoração de Jardim", "Decoración de Jardín"),
        (0x0000_0000_4000_0000, "Wall Art - Adult", "Arte de Parede - Adulto", "Arte de Pared - Adulto"),
        (0x0000_0000_8000_0000, "Plants", "Plantas", "Plantas"),
        (0x0000_0001_0000_0000, "Mirrors", "Espelhos", "Espejos"),
        (0x0000_0002_0000_0000, "Video Games", "Videogames", "Videojuegos"),
        (0x0000_0004_0000_0000, "Wall Art - Kids", "Arte de Parede - Infantil", "Arte de Pared - Infantil"),
        (0x0000_0008_0000_0000, "Bookshelves", "Estantes de Livros", "Librerías"),
        (0x0000_0010_0000_0000, "Cabinets", "Armários", "Armarios"),
        (0x0000_0020_0000_0000, "Dressers", "Cômodas", "Cómodas"),
        (0x0000_0040_0000_0000, "Dining Chairs", "Cadeiras de Jantar", "Sillas de Comedor"),
        (0x0000_0080_0000_0000, "Sofas", "Sofás", "Sofás"),
        (0x0000_0100_0000_0000, "Outdoor Seating", "Assentos Externos", "Asientos de Exterior"),
        (0x0000_0200_0000_0000, "Roof Decorations", "Decorações de Telhado", "Decoraciones de Tejado"),
        (0x0000_0400_0000_0000, "Beds", "Camas", "Camas"),
        (0x0000_0800_0000_0000, "Bar Stools", "Banquetas", "Taburetes de Bar"),
        (0x0000_1000_0000_0000, "Coffee Tables", "Mesas de Centro", "Mesas de Centro"),
        (0x0000_2000_0000_0000, "Counters", "Balcões", "Encimeras"),
        (0x0000_4000_0000_0000, "Desks", "Escrivaninhas", "Escritorios"),
        (0x0000_8000_0000_0000, "End Tables", "Mesas Laterais", "Mesas Auxiliares"),
        (0x0001_0000_0000_0000, "Dining Tables", "Mesas de Jantar", "Mesas de Comedor"),
        (0x0002_0000_0000_0000, "Furniture", "Móveis", "Muebles"),
        (0x0004_0000_0000_0000, "Toys", "Brinquedos", "Juguetes"),
        (0x0008_0000_0000_0000, "Transport", "Transporte", "Transporte"),
        (0x0010_0000_0000_0000, "Bars", "Bares", "Bares"),
        (0x0020_0000_0000_0000, "Clocks", "Relógios", "Relojes"),
        (0x0040_0000_0000_0000, "Window Decor", "Decoração de Janelas", "Decoración de Ventanas"),
        (0x0080_0000_0000_0000, "Kids Decor", "Decoração Infantil", "Decoración Infantil"),
        (0x0100_0000_0000_0000, "Misc Decor", "Decoração Diversa", "Decoración Variada"),
        (0x0200_0000_0000_0000, "Rugs", "Tapetes", "Alfombras"),
        (0x0400_0000_0000_0000, "Laundry", "Lavanderia", "Lavandería"),
        (0x0800_0000_0000_0000, "Pet Essentials", "Itens Essenciais para Animais", "Artículos Esenciales para Mascotas"),
    ];

    definitions
        .iter()
        .filter(|(flag, _, _, _)| (flags & *flag) != 0)
        .map(|(_, en, pt, es)| match language {
            AppLanguage::En => (*en).to_string(),
            AppLanguage::Pt => (*pt).to_string(),
            AppLanguage::Es => (*es).to_string(),
        })
        .collect()
}

fn room_usage(flags: u32, language: AppLanguage) -> Vec<String> {
    let definitions = [
        (0x0000_0002, "Living Room"),
        (0x0000_0004, "Dining Room"),
        (0x0000_0008, "Kitchen"),
        (0x0000_0010, "Kids Bedroom"),
        (0x0000_0020, "Bathroom"),
        (0x0000_0040, "Bedroom"),
        (0x0000_0080, "Study"),
        (0x0000_0100, "Outdoors"),
        (0x0000_0200, "Community"),
        (0x0000_0400, "Residential"),
        (0x0000_0800, "Pool"),
        (0x0000_1000, "Fountain"),
        (0x0000_2000, "Resort Lobby"),
        (0x0000_4000, "Resort Spa"),
        (0x0000_8000, "Resort Gym"),
        (0x0001_0000, "Resort Restaurant"),
        (0x0002_0000, "Resort Tiki Lounge"),
        (0x0004_0000, "Resort Arcade"),
        (0x0008_0000, "Resort Art Gallery"),
        (0x0010_0000, "Resort Dance Hall"),
        (0x0020_0000, "Resort Outdoor Party Area"),
        (0x0040_0000, "Resort Pool Area"),
    ];

    definitions
        .into_iter()
        .filter(|(flag, _)| (flags & *flag) != 0)
        .map(|(_, key)| match (language, key) {
            (AppLanguage::Pt, "Living Room") => "Sala de Estar".to_string(),
            (AppLanguage::Es, "Living Room") => "Sala de Estar".to_string(),
            (AppLanguage::Pt, "Dining Room") => "Sala de Jantar".to_string(),
            (AppLanguage::Es, "Dining Room") => "Comedor".to_string(),
            (AppLanguage::Pt, "Kitchen") => "Cozinha".to_string(),
            (AppLanguage::Es, "Kitchen") => "Cocina".to_string(),
            (AppLanguage::Pt, "Kids Bedroom") => "Quarto Infantil".to_string(),
            (AppLanguage::Es, "Kids Bedroom") => "Dormitorio Infantil".to_string(),
            (AppLanguage::Pt, "Bathroom") => "Banheiro".to_string(),
            (AppLanguage::Es, "Bathroom") => "Baño".to_string(),
            (AppLanguage::Pt, "Bedroom") => "Quarto".to_string(),
            (AppLanguage::Es, "Bedroom") => "Dormitorio".to_string(),
            (AppLanguage::Pt, "Study") => "Escritório".to_string(),
            (AppLanguage::Es, "Study") => "Estudio".to_string(),
            (AppLanguage::Pt, "Outdoors") => "Área Externa".to_string(),
            (AppLanguage::Es, "Outdoors") => "Exterior".to_string(),
            (AppLanguage::Pt, "Community") => "Comunitário".to_string(),
            (AppLanguage::Es, "Community") => "Comunitario".to_string(),
            (AppLanguage::Pt, "Residential") => "Residencial".to_string(),
            (AppLanguage::Es, "Residential") => "Residencial".to_string(),
            (AppLanguage::Pt, "Pool") => "Piscina".to_string(),
            (AppLanguage::Es, "Pool") => "Piscina".to_string(),
            (AppLanguage::Pt, "Fountain") => "Fonte".to_string(),
            (AppLanguage::Es, "Fountain") => "Fuente".to_string(),
            (AppLanguage::Pt, "Resort Lobby") => "Lobby do Resort".to_string(),
            (AppLanguage::Es, "Resort Lobby") => "Vestíbulo del Resort".to_string(),
            (AppLanguage::Pt, "Resort Spa") => "Spa do Resort".to_string(),
            (AppLanguage::Es, "Resort Spa") => "Spa del Resort".to_string(),
            (AppLanguage::Pt, "Resort Gym") => "Academia do Resort".to_string(),
            (AppLanguage::Es, "Resort Gym") => "Gimnasio del Resort".to_string(),
            (AppLanguage::Pt, "Resort Restaurant") => "Restaurante do Resort".to_string(),
            (AppLanguage::Es, "Resort Restaurant") => "Restaurante del Resort".to_string(),
            (AppLanguage::Pt, "Resort Tiki Lounge") => "Lounge Tiki do Resort".to_string(),
            (AppLanguage::Es, "Resort Tiki Lounge") => "Salón Tiki del Resort".to_string(),
            (AppLanguage::Pt, "Resort Arcade") => "Fliperama do Resort".to_string(),
            (AppLanguage::Es, "Resort Arcade") => "Sala de Juegos del Resort".to_string(),
            (AppLanguage::Pt, "Resort Art Gallery") => "Galeria de Arte do Resort".to_string(),
            (AppLanguage::Es, "Resort Art Gallery") => "Galería de Arte del Resort".to_string(),
            (AppLanguage::Pt, "Resort Dance Hall") => "Salão de Dança do Resort".to_string(),
            (AppLanguage::Es, "Resort Dance Hall") => "Salón de Baile del Resort".to_string(),
            (AppLanguage::Pt, "Resort Outdoor Party Area") => "Área Externa de Festas do Resort".to_string(),
            (AppLanguage::Es, "Resort Outdoor Party Area") => "Área Exterior de Fiestas del Resort".to_string(),
            (AppLanguage::Pt, "Resort Pool Area") => "Área da Piscina do Resort".to_string(),
            (AppLanguage::Es, "Resort Pool Area") => "Área de Piscina del Resort".to_string(),
            _ => key.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buy_main_uses_function_category_flags() {
        assert_eq!(decode_buy_main_all(0x0000_0800), vec!["Comfort"]);
        assert_eq!(decode_buy_main_all(0x0000_1000), vec!["Surfaces"]);
    }

    #[test]
    fn buy_subcategory_respects_parent_category() {
        assert_eq!(
            decode_buy_sub_all("Comfort", 0x0000_0080_0000_0000, 0),
            vec!["Sofas & Loveseats"]
        );
        assert_eq!(
            decode_buy_sub_all("Surfaces", 0x0001_0000_0000_0000, 0),
            vec!["Dining Tables"]
        );
    }

    #[test]
    fn build_flags_are_preferred_over_buy_flags() {
        let flags = ObjdCatalogFlags {
            object_type_flags: 0,
            room_flags: 0,
            room_sub_category_flags: 0,
            function_category_flags: 0x80,
            sub_category1_flags: 0,
            sub_category2_flags: 0,
            build_category_flags: 0x4,
        };
        assert_eq!(decode_build_all(flags), vec!["Windows"]);
    }

    #[test]
    fn object_type_flags_alone_do_not_invent_a_build_category() {
        let flags = ObjdCatalogFlags {
            object_type_flags: 0x10,
            room_flags: 0,
            room_sub_category_flags: 0,
            function_category_flags: 0,
            sub_category1_flags: 0,
            sub_category2_flags: 0,
            build_category_flags: 0,
        };
        assert_eq!(decode_build_all(flags), Vec::<&'static str>::new());
    }

    #[test]
    fn documented_pet_subcategories_are_distinct() {
        assert_eq!(
            decode_buy_sub_all("Pets", 0x0000_0002_0000_0000, 0),
            vec!["Dogs"]
        );
        assert_eq!(
            decode_buy_sub_all("Pets", 0x4000_0000_0000_0000, 0),
            vec!["Cats"]
        );
    }


    fn push_u32(data: &mut Vec<u8>, value: u32) {
        data.extend_from_slice(&value.to_le_bytes());
    }

    fn push_u64(data: &mut Vec<u8>, value: u64) {
        data.extend_from_slice(&value.to_le_bytes());
    }

    fn push_f32(data: &mut Vec<u8>, value: f32) {
        data.extend_from_slice(&value.to_le_bytes());
    }

    fn minimal_casp(
        clothing_type: u32,
        type_flags: u32,
        age_species_gender: u32,
        clothing_category: u32,
    ) -> Vec<u8> {
        let mut data = Vec::new();
        push_u32(&mut data, 0x12); // version
        push_u32(&mut data, 0); // TGI offset
        push_u32(&mut data, 0); // preset count
        data.push(0); // empty UnicodeBE 7BITSTR
        push_f32(&mut data, 0.0); // sort priority
        data.push(0); // unknown byte
        push_u32(&mut data, clothing_type);
        push_u32(&mut data, type_flags);
        push_u32(&mut data, age_species_gender);
        push_u32(&mut data, clothing_category);
        data
    }

    fn minimal_objd(
        function_category: u32,
        sub_category1: u64,
        sub_category2: u64,
        build_category: u32,
    ) -> Vec<u8> {
        let mut data = Vec::new();
        push_u32(&mut data, 0x1C); // OBJD version
        push_u32(&mut data, 0); // TGI offset
        push_u32(&mut data, 0); // TGI size
        push_u32(&mut data, 0); // material count
        data.push(0); // instance name (version >= 0x16)

        // Catalog Common, version 0x0F.
        push_u32(&mut data, 0x0F);
        push_u64(&mut data, 0); // name guid
        push_u64(&mut data, 0); // desc guid
        data.push(0); // internal name
        data.push(0); // internal description
        push_f32(&mut data, 0.0); // price
        push_f32(&mut data, 1.0); // niceness
        push_f32(&mut data, 0.0); // crap score
        data.push(0); // product status
        push_u64(&mut data, 0); // icon iid
        data.push(0);
        push_f32(&mut data, 0.0); // environment
        push_u32(&mut data, 0); // fire type
        data.push(0); // stealable
        data.push(0); // repossessable
        push_u32(&mut data, 0); // UI sort
        data.push(0); // placeable on roof
        data.push(0); // visible in worldbuilder
        push_u32(&mut data, 0); // product name hash

        push_u32(&mut data, 0); // OBJK index
        push_u32(&mut data, 0); // ObjectTypeFlags
        push_u32(&mut data, 0); // ObjectTypeFlags2 (version >= 0x1A)
        push_u32(&mut data, 0); // wall placement
        push_u32(&mut data, 0); // movement
        push_u32(&mut data, 0); // wall cutout tiles
        push_u32(&mut data, 0); // levels
        data.push(0); // wallmask count
        data.push(0); // script enabled
        push_u32(&mut data, 0); // diagonal index
        push_u32(&mut data, 0); // ambience hash
        push_u32(&mut data, 0x2); // Living Room
        push_u32(&mut data, function_category);
        push_u64(&mut data, sub_category1);
        push_u64(&mut data, sub_category2);
        push_u64(&mut data, 0); // sub-room flags
        push_u32(&mut data, build_category);
        data
    }

    #[test]
    fn casp_parser_reads_real_catalog_fields() {
        // Female Human YA+Adult Top; Everyday + Formalwear.
        let data = minimal_casp(
            0x05,
            0x08,
            0x0000_2130,
            0x0000_0002 | 0x0000_0004,
        );
        let classification = classify_casp(&data, AppLanguage::En).unwrap();

        assert_eq!(classification.source, "CASP");
        assert_eq!(classification.main_category, "Clothing");
        assert_eq!(classification.sub_category.as_deref(), Some("Top"));
        assert_eq!(classification.gender.as_deref(), Some("Female"));
        assert_eq!(classification.age.as_deref(), Some("Young Adult to Adult"));
        assert_eq!(classification.species.as_deref(), Some("Human"));
        assert_eq!(
            classification.folder_parts,
            vec!["CAS", "Clothing", "Female", "Young Adult to Adult", "Top"]
        );
        assert_eq!(
            classification.usage_categories,
            vec!["Everyday", "Formalwear"]
        );
    }

    #[test]
    fn casp_zero_species_is_human_only_with_valid_human_flags() {
        let human = classify_casp(&minimal_casp(0x05, 0, 0x0000_2030, 0), AppLanguage::En)
            .unwrap();
        assert_eq!(human.species.as_deref(), Some("Human"));

        let undisclosed = classify_casp(&minimal_casp(0x05, 0, 0, 0), AppLanguage::Pt).unwrap();
        assert_eq!(undisclosed.species, None);

        let pet = classify_casp(&minimal_casp(0x2F, 0, 0x0000_2000, 0), AppLanguage::En).unwrap();
        assert_eq!(pet.species, None);
    }

    #[test]
    fn accessories_keep_gender_and_subtype_but_no_age_folder() {
        // 0x18 is gloves; one CASP advertises teen through elder eligibility.
        for language in [AppLanguage::En, AppLanguage::Pt, AppLanguage::Es] {
            let data = minimal_casp(0x18, 0, 0x0000_2178, 0);
            let item = classify_casp(&data, language).unwrap();
            assert_eq!(item.folder_parts.len(), 4);
            assert!(item.folder_parts[1].eq_ignore_ascii_case(tr(language, "Accessories")));
            assert_eq!(item.folder_parts[2], tr(language, "Female"));
            assert_eq!(item.folder_parts[3], tr(language, "Gloves"));
            assert!(item.age.unwrap().contains(tr(language, "Unknown")) == false);
        }
    }

    #[test]
    fn brazilian_toddler_and_newborn_remain_distinct() {
        assert_eq!(casp_age_label(0x02, AppLanguage::Pt), "Bebê");
        assert_eq!(casp_age_label(0x01, AppLanguage::Pt), "Recém-Nascido");
        assert_eq!(casp_age_label(0x04, AppLanguage::Pt), "Criança");
        assert_eq!(casp_age_label(0x04 | 0x20, AppLanguage::Pt), "Criança e Adulto");
        assert_eq!(casp_age_label(0x04 | 0x08 | 0x10 | 0x20 | 0x40, AppLanguage::Pt), "Criança a Idoso");
    }

    #[test]
    fn facial_and_body_hair_have_no_physical_age_level() {
        for clothing_type in [0x10, 0x27] {
            let item = classify_casp(
                &minimal_casp(clothing_type, 0, 0x0000_2178, 0),
                AppLanguage::Pt,
            )
            .unwrap();
            if clothing_type == 0x10 {
                assert_eq!(item.main_category, "Cabelos");
                assert_eq!(item.sub_category.as_deref(), Some("Pelos Faciais"));
            } else {
                assert_eq!(item.main_category, "Pelos Corporais");
                assert_eq!(item.sub_category, None);
                assert_eq!(
                    item.folder_parts,
                    vec!["Cabelos e Pêlos", "Pelos Corporais", "Feminino"]
                );
            }
            assert!(item.folder_parts.iter().all(|part| !part.contains("Idade")));
            assert!(item.folder_parts.iter().all(|part| part != "Múltiplas Idades"));
        }
    }

    #[test]
    fn body_hair_synthetic_male_female_and_unisex_use_only_gender() {
        for (gender_bits, expected_gender) in [
            (0x1u32 << 12, "Masculino"),
            (0x2u32 << 12, "Feminino"),
            (0x3u32 << 12, "Unissex"),
        ] {
            let item = classify_casp(
                &minimal_casp(0x27, 0, gender_bits | 0x10 | 0x20 | 0x40, 0),
                AppLanguage::Pt,
            )
            .unwrap();
            assert_eq!(item.gender.as_deref(), Some(expected_gender));
            assert_eq!(
                item.folder_parts,
                vec!["Cabelos e Pêlos", "Pelos Corporais", expected_gender]
            );
            assert!(!item
                .folder_parts
                .iter()
                .any(|part| part.contains("Idade") || part.contains("Peito") || part.contains("Braço")));
        }
    }

    #[test]
    fn objd_parser_reads_buy_category_and_subcategory() {
        let data = minimal_objd(
            0x0000_0800, // Comfort
            0x0000_0080_0000_0000, // Sofas & Loveseats
            0,
            0,
        );
        let classification = classify_objd(&data, AppLanguage::En).unwrap();

        assert_eq!(classification.source, "OBJD");
        assert_eq!(classification.kind, "buy");
        assert_eq!(classification.main_category, "Comfort");
        assert_eq!(
            classification.sub_category.as_deref(),
            Some("Sofas & Loveseats")
        );
        assert_eq!(
            classification.folder_parts,
            vec!["Objects", "Comfort", "Sofas & Loveseats"]
        );
        assert_eq!(classification.usage_categories, vec!["Living Room"]);
        assert!(!classification.ambiguous);
        assert_eq!(
            classification.candidate_folder_parts,
            vec![vec!["Objects", "Comfort", "Sofas & Loveseats"]]
        );
    }

    #[test]
    fn objd_with_multiple_buy_categories_requires_review() {
        let data = minimal_objd(
            0x0000_0800 | 0x0000_1000, // Comfort + Surfaces
            0x0000_0080_0000_0000 | 0x0001_0000_0000_0000,
            0,
            0,
        );
        let classification = classify_objd(&data, AppLanguage::En).unwrap();
        assert!(classification.ambiguous);
        assert!(classification.folder_parts.is_empty());
        assert_eq!(classification.candidate_folder_parts.len(), 2);
    }

    #[test]
    fn objd_parser_uses_build_category_when_present() {
        let data = minimal_objd(
            0x0000_0080, // Decor is present but build placement is authoritative here.
            0,
            0,
            0x0000_0004, // Window
        );
        let classification = classify_objd(&data, AppLanguage::Pt).unwrap();

        assert_eq!(classification.kind, "build");
        assert_eq!(classification.main_category, "Construção");
        assert_eq!(classification.sub_category.as_deref(), Some("Janelas"));
        assert_eq!(
            classification.folder_parts,
            vec!["Construção", "Janelas"]
        );
    }
}
