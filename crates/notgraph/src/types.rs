use serde::{Deserialize, Serialize};

pub type CrateName = String;
pub type ModPath = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateGraph {
    pub nodes: Vec<CrateName>,
    pub edges: Vec<(CrateName, CrateName)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleGraph {
    pub krate: CrateName,
    pub nodes: Vec<ModPath>,
    pub edges: Vec<(ModPath, ModPath)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub mod_path: ModPath,
    pub kind: SymbolKind,
    pub name: String,
    pub is_pub: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SymbolKind {
    Struct,
    Enum,
    Trait,
    Fn,
    Type,
    Const,
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SymbolKind::Struct => write!(f, "struct"),
            SymbolKind::Enum => write!(f, "enum"),
            SymbolKind::Trait => write!(f, "trait"),
            SymbolKind::Fn => write!(f, "fn"),
            SymbolKind::Type => write!(f, "type"),
            SymbolKind::Const => write!(f, "const"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolTable {
    pub krate: CrateName,
    pub symbols: Vec<Symbol>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FanStats {
    pub name: String,
    pub fan_in: usize,
    pub fan_out: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModStats {
    pub krate: CrateName,
    pub nodes: Vec<FanStats>,
    pub cycles: Vec<Vec<ModPath>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HotspotKind {
    FanIn,
    FanOut,
}

impl std::fmt::Display for HotspotKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HotspotKind::FanIn => write!(f, "fan-in"),
            HotspotKind::FanOut => write!(f, "fan-out"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hotspot {
    pub name: String,
    pub kind: HotspotKind,
    pub score: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    pub crate_graph: Vec<FanStats>,
    pub module_graphs: Vec<ModStats>,
    pub hotspots: Vec<Hotspot>,
    pub cycles: Vec<Vec<ModPath>>,
}

/// Convert a source file path (relative to src/) to a Rust module path like `crate::foo::bar`.
pub fn path_to_mod(krate: &str, rel: &std::path::Path) -> ModPath {
    let mut parts: Vec<String> = vec![krate.to_string()];
    for component in rel.components() {
        let s = component.as_os_str().to_string_lossy();
        if s == "lib.rs" || s == "main.rs" {
            break;
        }
        let s = s.trim_end_matches(".rs").to_string();
        if s != "mod" {
            parts.push(s);
        }
    }
    parts.join("::")
}
