fn main() -> Result<(), Box<dyn std::error::Error>> {
    sevi_test_tools::touch_eval::normeval(&std::env::args().skip(1).collect::<Vec<_>>())
}
