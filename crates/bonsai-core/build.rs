use serde::Deserialize;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct GrammarConfig {
    language: Vec<LanguageEntry>,
}

#[derive(Deserialize)]
struct LanguageEntry {
    name: String,
    grammar: String,
    extensions: Vec<String>,
    src: String,
    queries: Option<String>,
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir.parent().unwrap().parent().unwrap();
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let config_path = workspace_root.join("grammars.toml");
    println!("cargo:rerun-if-changed={}", config_path.display());

    let config_str = fs::read_to_string(&config_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", config_path.display(), e));
    let config: GrammarConfig = toml::from_str(&config_str)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {}", config_path.display(), e));

    for lang in &config.language {
        compile_grammar(workspace_root, lang);
    }

    generate_languages_rs(&out_dir, &config.language, workspace_root);
}

fn compile_grammar(workspace_root: &Path, lang: &LanguageEntry) {
    let grammar_dir = workspace_root.join(&lang.grammar);
    let src_dir = grammar_dir.join(&lang.src);

    let parser_c = src_dir.join("parser.c");
    assert!(
        parser_c.exists(),
        "parser.c not found at {}",
        parser_c.display()
    );
    println!("cargo:rerun-if-changed={}", parser_c.display());

    let mut build = cc::Build::new();
    build
        .include(&src_dir)
        .flag_if_supported("-std=c11")
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-trigraphs")
        .file(&parser_c);

    let scanner_c = src_dir.join("scanner.c");
    if scanner_c.exists() {
        println!("cargo:rerun-if-changed={}", scanner_c.display());
        build.file(&scanner_c);
    }

    build.compile(&format!("tree_sitter_{}", lang.name));

    // Handle C++ scanner separately if present
    let scanner_cc = src_dir.join("scanner.cc");
    if scanner_cc.exists() {
        println!("cargo:rerun-if-changed={}", scanner_cc.display());
        cc::Build::new()
            .cpp(true)
            .include(&src_dir)
            .flag_if_supported("-std=c++14")
            .flag_if_supported("-Wno-unused-parameter")
            .file(&scanner_cc)
            .compile(&format!("tree_sitter_{}_scanner_cpp", lang.name));
    }
}

/// A type entry from node-types.json.
#[derive(Deserialize)]
struct NodeTypeEntry {
    #[serde(rename = "type")]
    type_name: String,
    subtypes: Option<Vec<NodeTypeRef>>,
}

#[derive(Deserialize)]
struct NodeTypeRef {
    #[serde(rename = "type")]
    type_name: String,
    named: bool,
}

/// Parse node-types.json and return supertype→[subtype] mappings.
/// Supertypes are entries with a `subtypes` array (convention: names start with `_`).
fn parse_node_types(grammar_dir: &Path, src_dir_name: &str) -> Vec<(String, Vec<String>)> {
    let node_types_path = grammar_dir.join(src_dir_name).join("node-types.json");
    if !node_types_path.exists() {
        return Vec::new();
    }
    println!("cargo:rerun-if-changed={}", node_types_path.display());

    let json_str = fs::read_to_string(&node_types_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", node_types_path.display(), e));
    let entries: Vec<NodeTypeEntry> = serde_json::from_str(&json_str)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {}", node_types_path.display(), e));

    entries
        .into_iter()
        .filter_map(|entry| {
            entry.subtypes.map(|subtypes| {
                let subtype_names: Vec<String> = subtypes
                    .into_iter()
                    .filter(|s| s.named)
                    .map(|s| s.type_name)
                    .collect();
                (entry.type_name, subtype_names)
            })
        })
        .filter(|(_, subtypes)| !subtypes.is_empty())
        .collect()
}

/// Validate that a language name is a valid Rust identifier (alphanumeric + underscore).
fn validate_identifier(name: &str) -> &str {
    assert!(
        !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
        "invalid language name (must be [a-zA-Z0-9_]+): {name:?}"
    );
    name
}

/// Escape a string for embedding in a Rust string literal.
fn escape_rust_string(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\\' => "\\\\".to_string(),
            '"' => "\\\"".to_string(),
            '\n' => "\\n".to_string(),
            '\r' => "\\r".to_string(),
            '\t' => "\\t".to_string(),
            '\0' => "\\0".to_string(),
            c => c.to_string(),
        })
        .collect()
}

/// Read a .scm file from a directory and return its Rust literal representation.
/// Returns "None" if the file doesn't exist.
fn embed_scm_file(dir: &Path, filename: &str) -> String {
    let path = dir.join(filename);
    if path.exists() {
        println!("cargo:rerun-if-changed={}", path.display());
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));
        format!("Some(\"{}\")", escape_rust_string(&content))
    } else {
        "None".to_string()
    }
}

fn generate_languages_rs(out_dir: &Path, languages: &[LanguageEntry], workspace_root: &Path) {
    let mut code = String::new();

    writeln!(code, "// Auto-generated by build.rs — do not edit").unwrap();
    writeln!(code).unwrap();
    writeln!(code, "use tree_sitter::Language;").unwrap();
    writeln!(code, "use tree_sitter_language::LanguageFn;").unwrap();
    writeln!(code).unwrap();

    // extern "C" declarations
    writeln!(code, "extern \"C\" {{").unwrap();
    for lang in languages {
        let name = validate_identifier(&lang.name);
        writeln!(code, "    fn tree_sitter_{}() -> *const ();", name).unwrap();
    }
    writeln!(code, "}}").unwrap();
    writeln!(code).unwrap();

    // LanguageInfo struct
    writeln!(code, "pub struct LanguageInfo {{").unwrap();
    writeln!(code, "    pub name: &'static str,").unwrap();
    writeln!(code, "    pub extensions: &'static [&'static str],").unwrap();
    writeln!(code, "    /// Embedded contents of locals.scm, if the file exists in the queries directory.").unwrap();
    writeln!(code, "    pub locals_scm: Option<&'static str>,").unwrap();
    writeln!(code, "    /// Embedded contents of tags.scm, if the file exists in the queries directory.").unwrap();
    writeln!(code, "    pub tags_scm: Option<&'static str>,").unwrap();
    writeln!(code, "}}").unwrap();
    writeln!(code).unwrap();

    // get_language function
    writeln!(
        code,
        "pub fn get_language(name: &str) -> Option<Language> {{"
    )
    .unwrap();
    writeln!(code, "    match name {{").unwrap();
    for lang in languages {
        writeln!(
            code,
            "        \"{}\" => Some(Language::new(unsafe {{ LanguageFn::from_raw(tree_sitter_{}) }})),",
            lang.name, lang.name
        )
        .unwrap();
    }
    writeln!(code, "        _ => None,").unwrap();
    writeln!(code, "    }}").unwrap();
    writeln!(code, "}}").unwrap();
    writeln!(code).unwrap();

    // get_language_by_extension function
    writeln!(
        code,
        "pub fn get_language_by_extension(ext: &str) -> Option<(&'static str, Language)> {{"
    )
    .unwrap();
    writeln!(code, "    match ext {{").unwrap();
    for lang in languages {
        let ext_pattern = lang
            .extensions
            .iter()
            .map(|e| format!("\"{}\"", e))
            .collect::<Vec<_>>()
            .join(" | ");
        writeln!(
            code,
            "        {} => get_language(\"{}\").map(|l| (\"{}\", l)),",
            ext_pattern, lang.name, lang.name
        )
        .unwrap();
    }
    writeln!(code, "        _ => None,").unwrap();
    writeln!(code, "    }}").unwrap();
    writeln!(code, "}}").unwrap();
    writeln!(code).unwrap();

    // list_languages function
    writeln!(
        code,
        "pub fn list_languages() -> &'static [LanguageInfo] {{"
    )
    .unwrap();
    writeln!(code, "    &[").unwrap();
    for lang in languages {
        let exts = lang
            .extensions
            .iter()
            .map(|e| format!("\"{}\"", e))
            .collect::<Vec<_>>()
            .join(", ");

        let (locals_content, tags_content) = match &lang.queries {
            Some(queries_dir) => {
                let queries_path = workspace_root.join(queries_dir);
                let locals = embed_scm_file(&queries_path, "locals.scm");
                let tags = embed_scm_file(&queries_path, "tags.scm");
                (locals, tags)
            }
            None => ("None".to_string(), "None".to_string()),
        };

        writeln!(
            code,
            "        LanguageInfo {{ name: \"{}\", extensions: &[{}], locals_scm: {}, tags_scm: {} }},",
            lang.name, exts, locals_content, tags_content
        )
        .unwrap();
    }
    writeln!(code, "    ]").unwrap();
    writeln!(code, "}}").unwrap();

    // Generate node-types supertype mappings per language
    writeln!(code).unwrap();
    writeln!(
        code,
        "/// Get supertype mappings from node-types.json for a language."
    )
    .unwrap();
    writeln!(
        code,
        "/// Returns (supertype_name, [subtype_names]) pairs."
    )
    .unwrap();
    writeln!(
        code,
        "pub fn get_node_types_supertypes(name: &str) -> &'static [(&'static str, &'static [&'static str])] {{"
    )
    .unwrap();
    writeln!(code, "    match name {{").unwrap();

    for lang in languages {
        let grammar_dir = workspace_root.join(&lang.grammar);
        let mappings = parse_node_types(&grammar_dir, &lang.src);

        if mappings.is_empty() {
            writeln!(code, "        \"{}\" => &[],", escape_rust_string(&lang.name)).unwrap();
        } else {
            writeln!(code, "        \"{}\" => &[", escape_rust_string(&lang.name)).unwrap();
            for (supertype, subtypes) in &mappings {
                let subtypes_str = subtypes
                    .iter()
                    .map(|s| format!("\"{}\"", escape_rust_string(s)))
                    .collect::<Vec<_>>()
                    .join(", ");
                writeln!(
                    code,
                    "            (\"{}\", &[{}]),",
                    escape_rust_string(supertype),
                    subtypes_str
                )
                .unwrap();
            }
            writeln!(code, "        ],").unwrap();
        }
    }

    writeln!(code, "        _ => &[],").unwrap();
    writeln!(code, "    }}").unwrap();
    writeln!(code, "}}").unwrap();

    let output_path = out_dir.join("languages.rs");
    fs::write(&output_path, code)
        .unwrap_or_else(|e| panic!("Failed to write {}: {}", output_path.display(), e));
}
