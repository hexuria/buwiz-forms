use bir_core::validation::official_tin_check_code;

#[test]
fn official_tin_check_code_matches_recorded_chkt_results() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/tin-check-digit-chkt.tsv"
    );
    let text = std::fs::read_to_string(path).expect("recorded chkt table");
    let mut rows = 0usize;
    for line in text
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let (tin, expected) = line.split_once('\t').expect("tin and chkt columns");
        let expected: u8 = expected.parse().expect("numeric chkt exit code");
        assert_eq!(official_tin_check_code(tin), expected, "TIN {tin}");
        rows += 1;
    }
    assert!(rows > 5_000, "recorded table should cover the fixed sample");
}
