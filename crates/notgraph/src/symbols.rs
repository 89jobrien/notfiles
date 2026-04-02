use crate::types::{CrateName, ModPath, Symbol, SymbolKind, SymbolTable};
use anyhow::Result;
use std::path::Path;
use syn::{Visibility, visit::Visit};
use walkdir::WalkDir;

struct SymbolCollector {
    mod_path: ModPath,
    symbols: Vec<Symbol>,
}

impl SymbolCollector {
    fn new(mod_path: ModPath) -> Self {
        Self { mod_path, symbols: Vec::new() }
    }

    fn is_pub(vis: &Visibility) -> bool {
        matches!(vis, Visibility::Public(_))
    }

    fn push(&mut self, kind: SymbolKind, name: String, is_pub: bool) {
        self.symbols.push(Symbol { mod_path: self.mod_path.clone(), kind, name, is_pub });
    }
}

impl<'ast> Visit<'ast> for SymbolCollector {
    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        self.push(SymbolKind::Struct, node.ident.to_string(), Self::is_pub(&node.vis));
    }
    fn visit_item_enum(&mut self, node: &'ast syn::ItemEnum) {
        self.push(SymbolKind::Enum, node.ident.to_string(), Self::is_pub(&node.vis));
    }
    fn visit_item_trait(&mut self, node: &'ast syn::ItemTrait) {
        self.push(SymbolKind::Trait, node.ident.to_string(), Self::is_pub(&node.vis));
    }
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.push(SymbolKind::Fn, node.sig.ident.to_string(), Self::is_pub(&node.vis));
    }
    fn visit_item_type(&mut self, node: &'ast syn::ItemType) {
        self.push(SymbolKind::Type, node.ident.to_string(), Self::is_pub(&node.vis));
    }
    fn visit_item_const(&mut self, node: &'ast syn::ItemConst) {
        self.push(SymbolKind::Const, node.ident.to_string(), Self::is_pub(&node.vis));
    }
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        let child_path = format!("{}::{}", self.mod_path, node.ident);
        let parent = std::mem::replace(&mut self.mod_path, child_path);
        syn::visit::visit_item_mod(self, node);
        self.mod_path = parent;
    }
}


pub fn build(krate: CrateName, src_dir: &Path) -> Result<SymbolTable> {
    let mut symbols: Vec<Symbol> = Vec::new();
    for entry in WalkDir::new(src_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "rs"))
    {
        let path = entry.path();
        let content = std::fs::read_to_string(path)?;
        let file = syn::parse_file(&content)?;
        let rel = path.strip_prefix(src_dir)?;
        let mod_path = crate::types::path_to_mod(&krate, rel);
        let mut collector = SymbolCollector::new(mod_path);
        collector.visit_file(&file);
        symbols.extend(collector.symbols);
    }
    Ok(SymbolTable { krate, symbols })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_table() -> SymbolTable {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/symbols/src");
        build("symbols".to_string(), &fixture).unwrap()
    }

    #[test]
    fn detects_pub_struct() {
        let t = fixture_table();
        assert!(t.symbols.iter().any(|s| s.name == "Foo" && s.kind == SymbolKind::Struct && s.is_pub));
    }

    #[test]
    fn detects_pub_enum() {
        let t = fixture_table();
        assert!(t.symbols.iter().any(|s| s.name == "Bar" && s.kind == SymbolKind::Enum && s.is_pub));
    }

    #[test]
    fn detects_pub_trait() {
        let t = fixture_table();
        assert!(t.symbols.iter().any(|s| s.name == "Baz" && s.kind == SymbolKind::Trait && s.is_pub));
    }

    #[test]
    fn detects_pub_fn() {
        let t = fixture_table();
        assert!(t.symbols.iter().any(|s| s.name == "qux" && s.kind == SymbolKind::Fn && s.is_pub));
    }

    #[test]
    fn detects_pub_type_alias() {
        let t = fixture_table();
        assert!(t.symbols.iter().any(|s| s.name == "Alias" && s.kind == SymbolKind::Type && s.is_pub));
    }

    #[test]
    fn detects_pub_const() {
        let t = fixture_table();
        assert!(t.symbols.iter().any(|s| s.name == "VALUE" && s.kind == SymbolKind::Const && s.is_pub));
    }

    #[test]
    fn private_struct_is_not_pub() {
        let t = fixture_table();
        let p = t.symbols.iter().find(|s| s.name == "Private").unwrap();
        assert!(!p.is_pub);
    }
}
