use trycmd::cargo;

#[test]
fn test_examples() {
    trycmd::TestCases::new()
        .default_bin_name("quake")
        .register_bins(cargo::compile_examples([]).unwrap())
        .case("tests/examples/*.toml");
}
