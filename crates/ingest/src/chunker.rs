//use unicode_segmentation::UnicodeSegmentation;
use crate::chunk::Chunk;

pub struct ChunkerConfig {
    pub target_tokens_min: usize,
    pub target_tokens_max: usize,
    pub overlap_tokens: usize,
}

impl Default for ChunkerConfig {
    fn default() -> Self {
        Self {
            target_tokens_min: 700,
            target_tokens_max: 900,
            overlap_tokens: 100,
        }
    }
}

pub struct Chunker {
    config: ChunkerConfig,
}

impl Chunker {
    pub fn new(config: ChunkerConfig) -> Self {
        Self { config }
    }

    pub fn chunk_text(
        &self,
        doc_id: &str,
        text: &str,
        source: &str,
    ) -> Vec<Chunk> {
        let mut chunks = Vec::new();
        
        // Split by headings first (markdown and plain text)
        let sections = self.split_by_headings(text);
        
        let mut current_offset = 0;
        
        for section in sections {
            let section_start = current_offset;
            
            // If section is small enough, make it one chunk
            if self.estimate_tokens(&section) <= self.config.target_tokens_max {
                if !section.trim().is_empty() {
                    chunks.push(Chunk::new(
                        doc_id.to_string(),
                        section.to_string(),
                        source.to_string(),
                        (section_start, section_start + section.len()),
                    ));
                }
                current_offset += section.len();
                continue;
            }
            
            // Otherwise, split by paragraphs
            let paragraphs = self.split_by_paragraphs(&section);
            let mut buffer = String::new();
            let mut buffer_start = section_start;
            
            for para in paragraphs {
                let para_tokens = self.estimate_tokens(&para);
                let buffer_tokens = self.estimate_tokens(&buffer);
                
                // If adding this paragraph exceeds max, flush buffer
                if buffer_tokens + para_tokens > self.config.target_tokens_max && !buffer.is_empty() {
                    chunks.push(Chunk::new(
                        doc_id.to_string(),
                        buffer.clone(),
                        source.to_string(),
                        (buffer_start, buffer_start + buffer.len()),
                    ));
                    
                    // Start new buffer with overlap
                    buffer = self.get_overlap(&buffer, self.config.overlap_tokens);
                    buffer_start = current_offset - buffer.len();
                }
                
                buffer.push_str(&para);
                buffer.push_str("\n\n");
                current_offset += para.len() + 2;
            }
            
            // Flush remaining buffer
            if !buffer.trim().is_empty() {
                chunks.push(Chunk::new(
                    doc_id.to_string(),
                    buffer,
                    source.to_string(),
                    (buffer_start, current_offset),
                ));
            }
        }
        
        chunks
    }

    fn split_by_headings(&self, text: &str) -> Vec<String> {
        let mut sections = Vec::new();
        let mut current_section = String::new();
        
        for line in text.lines() {
            // Check if line is a markdown heading
            if line.trim_start().starts_with('#') {
                if !current_section.is_empty() {
                    sections.push(current_section);
                    current_section = String::new();
                }
            }
            current_section.push_str(line);
            current_section.push('\n');
        }
        
        if !current_section.is_empty() {
            sections.push(current_section);
        }
        
        if sections.is_empty() {
            sections.push(text.to_string());
        }
        
        sections
    }

    fn split_by_paragraphs(&self, text: &str) -> Vec<String> {
        text.split("\n\n")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    fn estimate_tokens(&self, text: &str) -> usize {
        let word_count = text.split_whitespace().count();
        (word_count as f64 * 1.3) as usize
    }

    fn get_overlap(&self, text: &str, target_tokens: usize) -> String {
        let words: Vec<&str> = text.split_whitespace().collect();
        let target_words = (target_tokens as f64 / 1.3) as usize;
        
        if words.len() <= target_words {
            return text.to_string();
        }
        
        words[words.len().saturating_sub(target_words)..]
            .join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_chunking() {
        let chunker = Chunker::new(ChunkerConfig::default());
        let text = "This is a test paragraph.\n\nThis is another paragraph.";
        let chunks = chunker.chunk_text("test-doc", text, "test.txt");
        
        assert!(!chunks.is_empty());
        assert_eq!(chunks[0].doc_id, "test-doc");
    }
}