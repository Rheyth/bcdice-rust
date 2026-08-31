//! B15検証: 対象8システムのTOMLケースを全件検証する独立ランナー。
use std::path::{Path, PathBuf};

use bcdice::eval::{eval_command, EvalError};
use bcdice::game_system::GameSystemId;
use bcdice::randomizer::SeededRandomizer;
use bcdice::toml_test::TestDataFile;

/// (rustシステムID, tomlファイル, toml側game_system名, 期待ケース数)
const SYSTEMS: &[(&str, &str, &str, usize)] = &[
    ("MagicaLogia", "MagicaLogia.toml", "MagicaLogia", 155),
    (
        "MagicaLogia_Korean",
        "MagicaLogia_Korean.toml",
        "MagicaLogia:Korean",
        155,
    ),
    (
        "MagicaLogia_SimplifiedChinese",
        "MagicaLogia_SimplifiedChinese.toml",
        "MagicaLogia:SimplifiedChinese",
        155,
    ),
    ("FutariSousa", "FutariSousa.toml", "FutariSousa", 172),
    (
        "FutariSousa_Korean",
        "FutariSousa_Korean.toml",
        "FutariSousa:Korean",
        144,
    ),
    ("TokyoNova", "TokyoNova.toml", "TokyoNova", 8),
    ("WARPS", "WARPS.toml", "WARPS", 31),
    ("WaresBlade", "WaresBlade.toml", "WaresBlade", 13),
];

#[test]
fn b15_all_target_systems_full_pass() {
    let data_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("test/data");

    let mut total = 0usize;
    let mut passed = 0usize;
    let mut report = String::new();

    for (system, file, toml_system, expected) in SYSTEMS {
        let path: PathBuf = data_dir.join(file);
        let data = TestDataFile::load(&path)
            .unwrap_or_else(|e| panic!("{} must parse: {e}", path.display()));
        assert_eq!(
            data.tests.len(),
            *expected,
            "case count mismatch in {}",
            file
        );
        let mut sys_passed = 0usize;
        for (index, tc) in data.tests.iter().enumerate() {
            assert_eq!(tc.game_system, *toml_system, "unexpected system in {file}");
            total += 1;
            let mut reasons = Vec::new();
            let mut rng = SeededRandomizer::new(tc.rands.iter().map(|r| (r.value, r.sides)));
            match eval_command(&GameSystemId::new(*toml_system), &tc.input, &mut rng) {
                Err(e) => reasons.push(format!("eval error: {e}")),
                Ok(None) if !tc.expects_nil() => reasons.push("eval returned nil".into()),
                Ok(None) => {}
                Ok(Some(result)) => {
                    if tc.expects_nil() {
                        reasons.push(format!("expected nil, got {:?}", result.text));
                    } else if result.text != tc.output {
                        reasons.push(format!(
                            "output mismatch\n  expected: {:?}\n  actual:   {:?}",
                            tc.output, result.text
                        ));
                    }
                    for (name, exp, act) in [
                        ("secret", tc.secret, result.secret),
                        ("success", tc.success, result.success),
                        ("failure", tc.failure, result.failure),
                        ("critical", tc.critical, result.critical),
                        ("fumble", tc.fumble, result.fumble),
                    ] {
                        if exp != act {
                            reasons.push(format!(
                                "{name} flag mismatch: expected {exp}, actual {act}"
                            ));
                        }
                    }
                }
            }
            if !rng.is_empty() {
                reasons.push(format!("unconsumed rands remain ({})", rng.remaining()));
            }
            if reasons.is_empty() {
                sys_passed += 1;
                passed += 1;
            } else {
                report.push_str(&format!(
                    "FAIL {}:{}:{}\n  - {}\n",
                    system,
                    index + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }
        println!("{system}: {sys_passed}/{}", data.tests.len());
    }

    eprintln!("{report}");
    assert_eq!(total, 833, "total target cases");
    assert_eq!(passed, 833, "all target cases must pass");
}

#[test]
fn b15_no_eval_error_not_implemented() {
    // 8システムへの評価が SystemNotImplemented を返さないことの直接確認
    for (system, _, _, _) in SYSTEMS {
        let sys = GameSystemId::new(*system);
        let mut rng = SeededRandomizer::new(std::iter::empty());
        // 実在しないコマンドでも NotImplemented ではなく nil 系になることを期待
        if let Err(EvalError::NotImplemented) = eval_command(&sys, "ZZZ9", &mut rng) {
            panic!("{system}: eval_game_system_specific_command still NotImplemented");
        }
    }
}
