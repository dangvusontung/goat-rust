//! Offline-authored static word banks for club naming (Design round 2, Doc A
//! §A1.2/A2.2): nations are real countries; club names are fictional, picked
//! deterministically from these banks by PRNG at genesis (never any runtime
//! LLM call — bible §9 "LLMs at authoring time only").

/// One real country's club-naming recipe: fictional stems (place/club-name
/// fragments in that country's register) combined as `prefix + stem + suffix`.
/// Empty-string affixes represent "no affix" (e.g. a single-word Latin club).
pub struct NationSpec {
    /// Real country name.
    pub name: &'static str,
    /// Fictional name stems, country-flavored, unique within this country.
    pub stems: &'static [&'static str],
    /// Prefix options INCLUDING the empty string "" (no prefix).
    pub prefixes: &'static [&'static str],
    /// Suffix options INCLUDING the empty string "" (no suffix).
    pub suffixes: &'static [&'static str],
}

/// The 20 nations of the generated world, in fixed order (index = NationId).
pub const NATIONS: [NationSpec; 20] = [
    // 1. England — English compound: "X United / City / Town / ...".
    NationSpec {
        name: "England",
        stems: &[
            "Ashford",
            "Brackwell",
            "Solmoor",
            "Kingsmere",
            "Redcliffe",
            "Hallowgate",
            "Wynstead",
            "Marlow",
            "Thornbury",
            "Oakhaven",
            "Cresthill",
            "Fenwick",
            "Draymoor",
            "Hartfield",
            "Ellsworth",
            "Stonebridge",
            "Greymarsh",
            "Ravensdale",
        ],
        prefixes: &[""],
        suffixes: &[
            " United",
            " City",
            " Town",
            " Rovers",
            " Athletic",
            " Wanderers",
            " Albion",
        ],
    },
    // 2. Spain — Iberian prefix: "Real X / Deportivo X / CF X / UD X / X".
    NationSpec {
        name: "Spain",
        stems: &[
            "Valdoria",
            "Montebravo",
            "Riomar",
            "Castelvar",
            "Solterra",
            "Brisalda",
            "Torrecilla",
            "Almagran",
            "Pueblar",
            "Vestigar",
            "Coralvo",
            "Zalmera",
            "Navero",
            "Quintela",
            "Olivara",
            "Merindal",
            "Salobra",
            "Lucenar",
        ],
        prefixes: &["Real ", "Deportivo ", "CF ", "UD ", ""],
        suffixes: &[""],
    },
    // 3. Germany — Germanic: "SV X / FC X / TSV X / X (+ 04)".
    NationSpec {
        name: "Germany",
        stems: &[
            "Eisenwald",
            "Falkenruh",
            "Grunfeld",
            "Hochstein",
            "Nordbruck",
            "Westerhain",
            "Blauheim",
            "Rotwald",
            "Silberfels",
            "Dornheim",
            "Adlerfeld",
            "Lichtenbrunn",
            "Waldtorf",
            "Greifenau",
            "Sturmwald",
            "Kaltbach",
            "Ehrenfels",
            "Morgenroth",
            "Vierlingen",
            "Aschenberg",
        ],
        prefixes: &["", "SV ", "FC ", "TSV "],
        suffixes: &["", " 04"],
    },
    // 4. Italy — Italian: "AC X / SS X / AS X / X".
    NationSpec {
        name: "Italy",
        stems: &[
            "Velloria",
            "Castelmaro",
            "Solavena",
            "Brumante",
            "Torralba",
            "Vignarola",
            "Cortalto",
            "Bellariva",
            "Altoserra",
            "Marevigo",
            "Serralba",
            "Vinaccia",
            "Ombretta",
            "Collerina",
            "Pratoria",
            "Lucamonte",
            "Fioralba",
            "Gravella",
            "Neravola",
            "Trescanto",
        ],
        prefixes: &["AC ", "SS ", "AS ", ""],
        suffixes: &[""],
    },
    // 5. France — French: "AS X / FC X / Olympique X / X".
    NationSpec {
        name: "France",
        stems: &[
            "Valcroix",
            "Belleval",
            "Ormontagne",
            "Solrienne",
            "Vertalan",
            "Chambrelle",
            "Montverre",
            "Lusigne",
            "Ravencourt",
            "Tesselles",
            "Brumagne",
            "Auvielle",
            "Sorvannes",
            "Clairmont",
            "Valbonnet",
            "Quersanne",
            "Miravelle",
            "Tournecy",
        ],
        prefixes: &["AS ", "FC ", "Olympique ", ""],
        suffixes: &[""],
    },
    // 6. Brazil — Latin single-word with optional suffix: "X / X FC / X EC / X SC".
    NationSpec {
        name: "Brazil",
        stems: &[
            "Volcanza",
            "Marejada",
            "Estrelar",
            "Cruzeta",
            "Fluvente",
            "Andaria",
            "Pampero",
            "Litorena",
            "Tropicó",
            "Cordillar",
            "Solaço",
            "Barranca",
            "Vermelhas",
            "Selvana",
            "Riacho",
            "Tucanaço",
            "Cerraço",
            "Guaranti",
            "Aurinegro",
            "Costeira",
            "Manguera",
            "Pantana",
            "Sertana",
            "Bravante",
        ],
        prefixes: &[""],
        suffixes: &["", " FC", " EC", " SC"],
    },
    // 7. Argentina — Latin: "X / X FC / X CA".
    NationSpec {
        name: "Argentina",
        stems: &[
            "Riobravo",
            "Pampaluna",
            "Estelmar",
            "Portavoz",
            "Ventania",
            "Ceibales",
            "Quebracho",
            "Alambrado",
            "Solguazu",
            "Barriales",
            "Mirasur",
            "Costablanca",
            "Atlalaya",
            "Vigilmar",
            "Pueblorojo",
            "Marinete",
            "Lafragua",
            "Bombalera",
            "Criollos",
            "Ventisur",
            "Auriazul",
            "Porteno",
            "Yerbaluz",
            "Salitrero",
        ],
        prefixes: &[""],
        suffixes: &["", " FC", " CA"],
    },
    // 8. Portugal — Iberian prefix: "Sporting X / SL X / FC X / X".
    NationSpec {
        name: "Portugal",
        stems: &[
            "Marverde",
            "Ribagua",
            "Solpenha",
            "Costanegra",
            "Bravomar",
            "Tejonal",
            "Floravila",
            "Penedal",
            "Olivalmar",
            "Fontenova",
            "Carmiel",
            "Serradoiro",
            "Estrelmar",
            "Adiamar",
            "Cascalheira",
            "Brumalva",
            "Nazario",
            "Ventosal",
        ],
        prefixes: &["Sporting ", "SL ", "FC ", ""],
        suffixes: &[""],
    },
    // 9. Netherlands — Dutch: "X / FC X / SC X (+ '05)".
    NationSpec {
        name: "Netherlands",
        stems: &[
            "Voorhaven",
            "Zuidmeer",
            "Noordveld",
            "Waterrijk",
            "Polderstad",
            "Veenhof",
            "Dijkwacht",
            "Merenborg",
            "Sluizerdam",
            "Graanstad",
            "Kanaaldam",
            "Zeebrug",
            "Westhove",
            "Oostzande",
            "Vrijdam",
            "Landermeer",
            "Stormvaart",
            "Hoogvlied",
            "Heideveen",
            "Duinendaal",
        ],
        prefixes: &["", "FC ", "SC "],
        suffixes: &["", " '05"],
    },
    // 10. Belgium — Benelux: "R X / FC X / K X / X".
    NationSpec {
        name: "Belgium",
        stems: &[
            "Vaalbeek",
            "Grimberg",
            "Zandhove",
            "Boskant",
            "Heuvelrode",
            "Koekenberg",
            "Vlasmarkt",
            "Bruinvijver",
            "Steenokker",
            "Waalrode",
            "Montreval",
            "Sartmagne",
            "Vallombre",
            "Chasteler",
            "Brumard",
            "Tournavelle",
            "Melderveld",
            "Ardenneval",
        ],
        prefixes: &["R ", "FC ", "K ", ""],
        suffixes: &[""],
    },
    // 11. Uruguay — Latin: "X / X FC / X CA".
    NationSpec {
        name: "Uruguay",
        stems: &[
            "Rioplata",
            "Cimarrona",
            "Yaguares",
            "Tambero",
            "Cerroluz",
            "Guasquero",
            "Brumario",
            "Talanquera",
            "Pajonal",
            "Estibano",
            "Farolito",
            "Meridiano",
            "Casavela",
            "Horqueta",
            "Molinar",
            "Bajada",
            "Canuelero",
            "Ombues",
            "Tarariras",
            "Vindar",
            "Cerralbo",
            "Puntalito",
            "Ribenorte",
            "Luciernaga",
        ],
        prefixes: &[""],
        suffixes: &["", " FC", " CA"],
    },
    // 12. Colombia — Latin: "X / X FC / X CD".
    NationSpec {
        name: "Colombia",
        stems: &[
            "Nevadiza",
            "Esmeraldo",
            "Quindival",
            "Tropicanal",
            "Cordillano",
            "Selvatia",
            "Brisamar",
            "Yumbala",
            "Guadual",
            "Palmeral",
            "Azufral",
            "Tunalito",
            "Cajibro",
            "Llanoral",
            "Vallenal",
            "Arriero",
            "Colibri",
            "Sabanal",
            "Merengual",
            "Cafetoro",
            "Riofrio",
            "Tejadita",
            "Mompinar",
            "Guaneval",
        ],
        prefixes: &[""],
        suffixes: &["", " FC", " CD"],
    },
    // 13. Croatia — Balkan: "NK X / HNK X / X".
    NationSpec {
        name: "Croatia",
        stems: &[
            "Kresovar",
            "Plavomar",
            "Goranica",
            "Litorin",
            "Dubravac",
            "Kraljmar",
            "Bistrona",
            "Zlatomir",
            "Svetinov",
            "Gornjale",
            "Veligrad",
            "Criklava",
            "Marinjevo",
            "Ostrograd",
            "Primorina",
            "Kastelmar",
            "Neretina",
            "Bracanin",
            "Vranograd",
            "Topolnica",
            "Jabukar",
            "Kotovar",
            "Srebromar",
            "Dubovica",
        ],
        prefixes: &["NK ", "HNK ", ""],
        suffixes: &[""],
    },
    // 14. Mexico — Latin: "CD X / X (+ FC)".
    NationSpec {
        name: "Mexico",
        stems: &[
            "Tecolote",
            "Axolmar",
            "Guajalote",
            "Soltepec",
            "Nayarindo",
            "Quetzalmar",
            "Jaguarete",
            "Mezcalina",
            "Cempasuchil",
            "Tlacorito",
            "Volcanario",
            "Verderio",
            "Cacturno",
            "Nopalito",
            "Serranito",
            "Playamar",
            "Desiertina",
            "Magueyal",
            "Torremaya",
            "Aztecal",
        ],
        prefixes: &["CD ", ""],
        suffixes: &["", " FC"],
    },
    // 15. USA — American compound: "X United / X FC / X City / X SC".
    NationSpec {
        name: "USA",
        stems: &[
            "Ironridge",
            "Bluewater",
            "Redmesa",
            "Copperglade",
            "Lakemere",
            "Summitrun",
            "Pinefall",
            "Cedarfrost",
            "Rivertread",
            "Goldspar",
            "Prairievale",
            "Granitepeak",
            "Harborcrest",
            "Sunspire",
            "Dunefield",
            "Birchrun",
            "Meadowlark",
            "Quartzridge",
            "Flintvale",
            "Stormhaven",
        ],
        prefixes: &[""],
        suffixes: &[" United", " FC", " City", " SC"],
    },
    // 16. Japan — Japanese compound: "X FC / X United / X SC".
    NationSpec {
        name: "Japan",
        stems: &[
            "Sakurano",
            "Hikarizawa",
            "Tsukimori",
            "Kazeoka",
            "Fujisora",
            "Umibara",
            "Yorutani",
            "Asahiga",
            "Kirihara",
            "Momijino",
            "Sorahama",
            "Midorino",
            "Takanebashi",
            "Shirayuki",
            "Hoshimori",
            "Kamizaki",
            "Nishinohara",
            "Oginome",
            "Yanagihara",
            "Akarimura",
            "Furuhama",
            "Tsurubashi",
            "Koganemizu",
            "Hachibara",
        ],
        prefixes: &[""],
        suffixes: &[" FC", " United", " SC"],
    },
    // 17. South Korea — Korean compound: "X FC / X United / X SC".
    NationSpec {
        name: "South Korea",
        stems: &[
            "Haemaru",
            "Byeolnuri",
            "Dalmuri",
            "Solbaek",
            "Cheonghwa",
            "Haneuldan",
            "Baramsan",
            "Saemaru",
            "Gureumchi",
            "Dolgorae",
            "Moranbat",
            "Bitnari",
            "Haemong",
            "Seonuri",
            "Taeyangdo",
            "Pureunsan",
            "Danbirang",
            "Hwanghomaru",
            "Neulpureun",
            "Saebom",
            "Darakbit",
            "Gidungol",
            "Sanchaekro",
            "Areumdam",
        ],
        prefixes: &[""],
        suffixes: &[" FC", " United", " SC"],
    },
    // 18. Nigeria — West African compound: "X FC / X United / X Stars".
    NationSpec {
        name: "Nigeria",
        stems: &[
            "Zabira", "Onigba", "Talokpa", "Okposa", "Egbama", "Dunduma", "Kafanga", "Biruma",
            "Ogadina", "Nsabira", "Lafanga", "Zumbari", "Uroba", "Gwanduma", "Asabira", "Ilefun",
            "Tunbara", "Sagama", "Owebiri", "Gombira", "Yolasa", "Danbuzu", "Rigama", "Kubanni",
        ],
        prefixes: &[""],
        suffixes: &[" FC", " United", " Stars"],
    },
    // 19. Morocco — North African: "X / FC X (+ FC)".
    NationSpec {
        name: "Morocco",
        stems: &[
            "Zahramar", "Benidar", "Wadnour", "Darjemal", "Qasbana", "Zafraan", "Ourzana",
            "Safranor", "Doukkara", "Soukaina", "Jbalmar", "Ouedna", "Tarfala", "Nourlaya",
            "Bouskara", "Souirana", "Draamar", "Tiznala", "Guelmara", "Ifrimar",
        ],
        prefixes: &["", "FC "],
        suffixes: &["", " FC"],
    },
    // 20. Norway — Nordic: "X / FK X / IF X (+ BK)".
    NationSpec {
        name: "Norway",
        stems: &[
            "Fjordvik",
            "Isbreholm",
            "Nordlund",
            "Snohaug",
            "Brimfjor",
            "Granheim",
            "Ulvnest",
            "Bjornfell",
            "Havstrand",
            "Viddeheim",
            "Skogbryn",
            "Elvheim",
            "Fossekall",
            "Steinnes",
            "Ravnfjell",
            "Solbukt",
            "Valhall",
            "Ormevik",
            "Krakeskar",
            "Vinterli",
        ],
        prefixes: &["", "FK ", "IF "],
        suffixes: &["", " BK"],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Needed: 60 unique clubs per country + dedupe margin.
    const MIN_COMBOS: usize = 72;

    #[test]
    fn every_country_has_enough_combos() {
        for spec in &NATIONS {
            let combos = spec.stems.len() * spec.prefixes.len() * spec.suffixes.len();
            assert!(
                combos >= MIN_COMBOS,
                "{}: {} combos < {}",
                spec.name,
                combos,
                MIN_COMBOS
            );
        }
    }

    #[test]
    fn stems_unique_within_country() {
        for spec in &NATIONS {
            let mut seen = HashSet::new();
            for stem in spec.stems {
                assert!(
                    seen.insert(stem),
                    "{}: duplicate stem {:?}",
                    spec.name,
                    stem
                );
            }
        }
    }

    #[test]
    fn no_empty_stems_and_no_stem_whitespace() {
        for spec in &NATIONS {
            for stem in spec.stems {
                assert!(!stem.is_empty(), "{}: empty stem", spec.name);
                assert_eq!(
                    *stem,
                    stem.trim(),
                    "{}: stem {:?} has edge whitespace",
                    spec.name,
                    stem
                );
            }
        }
    }

    #[test]
    fn country_names_non_empty_and_unique() {
        let mut seen = HashSet::new();
        for spec in &NATIONS {
            assert!(!spec.name.is_empty(), "empty country name");
            assert!(seen.insert(spec.name), "duplicate country {:?}", spec.name);
        }
        assert_eq!(NATIONS.len(), 20);
    }

    #[test]
    fn affix_arrays_non_empty_and_contain_empty_option() {
        // Per the spec, compound-register countries (England, USA, Japan,
        // South Korea, Nigeria) have NO empty suffix option: every club there
        // carries an explicit compound suffix ("United", "City", ...).
        const NO_EMPTY_SUFFIX: [&str; 5] = ["England", "USA", "Japan", "South Korea", "Nigeria"];
        for spec in &NATIONS {
            assert!(!spec.prefixes.is_empty(), "{}: no prefixes", spec.name);
            assert!(!spec.suffixes.is_empty(), "{}: no suffixes", spec.name);
            assert!(
                spec.prefixes.contains(&""),
                "{}: prefixes missing empty-string option",
                spec.name
            );
            if NO_EMPTY_SUFFIX.contains(&spec.name) {
                assert!(
                    !spec.suffixes.contains(&""),
                    "{}: suffixes must NOT have an empty-string option",
                    spec.name
                );
            } else {
                assert!(
                    spec.suffixes.contains(&""),
                    "{}: suffixes missing empty-string option",
                    spec.name
                );
            }
        }
    }

    #[test]
    fn affixes_carry_only_single_spaces() {
        for spec in &NATIONS {
            for affix in spec.prefixes.iter().chain(spec.suffixes.iter()) {
                assert!(
                    !affix.contains("  "),
                    "{}: affix {:?} has a double space",
                    spec.name,
                    affix
                );
            }
        }
    }
}
