// src/bootstrap/resolver.rs
use crate::meta_ast::{GrammarDefinition, Rule};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;

pub struct GrammarResolver {
    base_path: PathBuf,
}

impl GrammarResolver {
    pub fn new<P: AsRef<Path>>(base_path: P) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
        }
    }

    /// Lädt eine Grammatik und löst alle Vererbungen auf.
    /// Gibt eine "flache" Grammatik zurück, die alles enthält.
    pub fn resolve(&self, file_name: &str) -> GrammarDefinition {
        // 1. Parse die angeforderte Datei (Leaf)
        let leaf_grammar = self.parse_file(file_name);

        // 2. Prüfe auf Vererbung (Rekursionsanker)
        let parent_name = match &leaf_grammar.inherits {
            None => return leaf_grammar,
            Some(ident) => ident.to_string(),
        };

        // 3. Rekursiver Aufruf: Lade den Parent (vollständig aufgelöst)
        // Konvention: Parent "Core" liegt in "Core.grammar"
        let parent_file = format!("{}.grammar", parent_name);
        let parent_grammar = self.resolve(&parent_file);

        // 4. Merge: Parent + Leaf = Result
        self.merge(parent_grammar, leaf_grammar)
    }

    fn merge(&self, mut base: GrammarDefinition, child: GrammarDefinition) -> GrammarDefinition {
        // Wir bauen eine Map für schnellen Lookup der Base-Regeln
        let mut rule_map: HashMap<String, usize> = base.rules.iter()
            .enumerate()
            .map(|(i, r)| (r.name.to_string(), i))
            .collect();

        // Wir iterieren über die Regeln des Kindes
        for child_rule in child.rules {
            let name = child_rule.name.to_string();
            
            if let Some(&index) = rule_map.get(&name) {
                // OVERRIDE: Die Regel existiert schon -> Ersetzen
                println!("::note Overriding rule '{}' from base grammar", name);
                base.rules[index] = child_rule;
            } else {
                // EXTEND: Neue Regel -> Anhängen
                base.rules.push(child_rule);
            }
        }

        // Der Name der resultierenden Grammatik ist der des Kindes
        base.name = child.name;
        base.inherits = None; // Vererbung ist jetzt "eingebacken"
        
        base
    }

    fn parse_file(&self, file_name: &str) -> GrammarDefinition {
        let path = self.base_path.join(file_name);
        let content = fs::read_to_string(&path)
            .expect(&format!("Grammar file not found: {:?}", path));

        // Hier rufen wir den 'syn' Parser auf, den wir in bootstrap/grammar.rs haben
        // (Dies ist der Platzhalter-Aufruf)
        syn::parse_str::<GrammarDefinition>(&content)
            .expect(&format!("Failed to parse grammar: {}", file_name))
    }
}

