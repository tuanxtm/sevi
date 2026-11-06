fn main() -> Result<(), Box<dyn std::error::Error>> {
    sevi_test_tools::sim::run(&std::env::args().skip(1).collect::<Vec<_>>())
}
