use regex::Regex;
use std::collections::HashMap;

pub struct EntityNormalizer {
    /// Maps normalized name -> canonical name
    aliases: HashMap<String, String>,
}

impl EntityNormalizer {
    pub fn new() -> Self {
        Self {
            aliases: HashMap::new(),
        }
    }

    /// Normalize entity name: lowercase, trim punctuation, handle common variations
    pub fn normalize(&mut self, name: &str) -> String {
        // Convert to lowercase
        let mut normalized = name.to_lowercase();
        
        // Trim leading/trailing punctuation and whitespace
        normalized = normalized.trim().to_string();
        
        // Remove common punctuation
        let re = Regex::new(r"[.,!?;:']").unwrap();
        normalized = re.replace_all(&normalized, "").to_string();
        
        // Collapse multiple spaces
        let re = Regex::new(r"\s+").unwrap();
        normalized = re.replace_all(&normalized, " ").to_string();
        
        // Check if we've seen a similar entity
        if let Some(canonical) = self.aliases.get(&normalized) {
            return canonical.clone();
        }
        
        // Check for near-duplicates (simple fuzzy matching)
        let mut found_canonical = None;

        for (existing_norm, canonical) in &self.aliases {
            if self.are_similar(&normalized, existing_norm) {
                // Map this new variant to the existing canonical form
                found_canonical = Some(canonical.clone());
                break;
            }
        }
        
        if let Some(canonical) = found_canonical {
            self.aliases.insert(normalized.clone(), canonical.clone());
            return canonical.clone();
        }
        
        // This is a new entity - use the normalized form as canonical
        self.aliases.insert(normalized.clone(), normalized.clone());
        normalized
    }

    /// Simple similarity check - can be improved with edit distance
    fn are_similar(&self, a: &str, b: &str) -> bool {
        // Same after normalization
        if a == b {
            return true;
        }
        
        // One is contained in the other (handles AI vs artificial intelligence)
        if a.contains(b) || b.contains(a) {
            return true;
        }
        
        // Check if they share most words (for multi-word entities)
        let words_a: Vec<&str> = a.split_whitespace().collect();
        let words_b: Vec<&str> = b.split_whitespace().collect();
        
        if words_a.len() > 1 && words_b.len() > 1 {
            let common: usize = words_a.iter()
                .filter(|w| words_b.contains(w))
                .count();
            
            let total = words_a.len().max(words_b.len());
            return common as f64 / total as f64 > 0.7; // 70% overlap
        }
        
        false
    }

    /// Get the mapping of all aliases
    pub fn get_aliases(&self) -> &HashMap<String, String> {
        &self.aliases
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalization() {
        let mut normalizer = EntityNormalizer::new();
        
        assert_eq!(normalizer.normalize("GraphRAG"), "graphrag");
        assert_eq!(normalizer.normalize("GraphRAG!"), "graphrag");
        assert_eq!(normalizer.normalize("  GraphRAG  "), "graphrag");
    }

    #[test]
    fn test_alias_resolution() {
        let mut normalizer = EntityNormalizer::new();
        
        let n1 = normalizer.normalize("OpenAI");
        let n2 = normalizer.normalize("OpenAI Inc");
        
        // Should resolve to the same canonical form
        assert_eq!(n1, n2);
    }
}