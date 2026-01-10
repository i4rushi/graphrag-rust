pub mod llm;
pub mod local_search;
pub mod global_search;

pub use llm::QueryLLM;
pub use local_search::{LocalSearchEngine, LocalSearchResult};
pub use global_search::{GlobalSearchEngine, GlobalSearchResult};

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
pub fn placeholder() {
    println!("query module - to be implemented");
}
