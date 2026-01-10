pub mod vanilla_rag;
pub mod test_set;
pub mod benchmark;
pub mod plots;

pub use vanilla_rag::VanillaRAG;
pub use test_set::get_test_set;
pub use benchmark::{Benchmarker, BenchmarkResults};
pub use plots::generate_plots;

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
