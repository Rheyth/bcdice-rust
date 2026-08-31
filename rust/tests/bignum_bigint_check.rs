use bcdice::randomizer::SeededRandomizer;

/// fuzz_rust.jsonl の bignum ケースの BigInt化後の出力検証（P4-B18検証用）
///
/// 差分ファズの不一致11件は、いずれも i64飽和時代のバイナリ（2026-08-30 20:08生成、
/// B18-1より前）が出力した stale な fuzz_rust.jsonl によるもの。
/// 本テストは同一入力・同一注入乱数で BigInt化後の実装を直接評価し、
/// Ruby期待値との一致を確認する（= 不一致11件の解消を直接実証する）。
fn run(input: &str, rands: &[(i64, i64)]) -> String {
    let system = bcdice::game_system::game_system_class("DiceBot").expect("DiceBot registered");
    let mut src = SeededRandomizer::new(rands.iter().copied());
    let mut rng = bcdice::randomizer::Randomizer::new(&mut src);
    match bcdice::eval::eval_raw(system, input, &mut rng) {
        Ok(Some(r)) => r.text,
        Ok(None) => "<nil>".to_string(),
        Err(e) => format!("<err:{e}>"),
    }
}

type BignumCase<'a> = (&'a str, Vec<(i64, i64)>, &'a str);

#[test]
fn bignum_cases_match_ruby() {
    // (入力, 注入乱数=reports/fuzz_ruby.jsonl の rands, Ruby期待値出力)
    let cases: Vec<BignumCase> = vec![
        (
            "C10000000000*10000000000",
            vec![],
            "c(10000000000*10000000000) ＞ 100000000000000000000",
        ),
        (
            "C99999999999*99999999999",
            vec![],
            "c(99999999999*99999999999) ＞ 9999999999800000000001",
        ),
        (
            "C9223372036854775808",
            vec![],
            "c(9223372036854775808) ＞ 9223372036854775808",
        ),
        (
            "C9223372036854775807+1",
            vec![],
            "c(9223372036854775807+1) ＞ 9223372036854775808",
        ),
        (
            "C9223372036854775807*2",
            vec![],
            "c(9223372036854775807*2) ＞ 18446744073709551614",
        ),
        (
            "C4611686018427387904*2",
            vec![],
            "c(4611686018427387904*2) ＞ 9223372036854775808",
        ),
        (
            "C-9223372036854775807-2",
            vec![],
            "c(-9223372036854775807-2) ＞ -9223372036854775809",
        ),
        (
            "C-9223372036854775808",
            vec![],
            "c(-9223372036854775808) ＞ -9223372036854775808",
        ),
        (
            "1D6+9223372036854775807",
            vec![(2, 6)],
            "(1D6+9223372036854775807) ＞ 2[2]+9223372036854775807 ＞ 9223372036854775809",
        ),
        (
            "2D6*9223372036854775807",
            vec![(2, 6), (1, 6)],
            "(2D6*9223372036854775807) ＞ 3[2,1]*9223372036854775807 ＞ 27670116110564327421",
        ),
        (
            "2D6>=99999999999999999999",
            vec![(2, 6), (1, 6)],
            "(2D6>=99999999999999999999) ＞ 3[2,1] ＞ 3 ＞ 失敗",
        ),
    ];

    let mut failures = Vec::new();
    for (input, rands, expected) in &cases {
        let actual = run(input, rands);
        if &actual != expected {
            failures.push(format!(
                "input: {input}\n  expected: {expected}\n  actual:   {actual}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "bignum cases mismatching Ruby:\n{}",
        failures.join("\n---\n")
    );
}
