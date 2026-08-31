//! flow-controller検証: P4-B04 残タスク3システムのTOMLケース全件検証ランナー。
//! 一時的な検証用ファイル（検証後は削除する）。
use std::path::{Path, PathBuf};

use bcdice::eval::eval_command;
use bcdice::game_system::GameSystemId;
use bcdice::randomizer::SeededRandomizer;
use bcdice::toml_test::TestDataFile;

const B04_SYSTEMS: &[&str] = &["Elysion", "DivineCharger", "KanColle"];

#[test]
fn flow_check_b04_all_systems() {
    let data_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("test/data");

    let mut total = 0usize;
    let mut passed = 0usize;
    let mut fails: Vec<String> = Vec::new();

    for system in B04_SYSTEMS {
        let path: PathBuf = data_dir.join(format!("{system}.toml"));
        let data = match TestDataFile::load(&path) {
            Ok(d) => d,
            Err(e) => {
                fails.push(format!("{system}: load error {e}"));
                continue;
            }
        };
        let mut sys_passed = 0usize;
        for tc in data.tests.iter() {
            total += 1;
            let mut rng = SeededRandomizer::new(tc.rands.iter().map(|r| (r.value, r.sides)));
            match eval_command(&GameSystemId::new(*system), &tc.input, &mut rng) {
                Err(e) => fails.push(format!("{system}[{}]: eval error: {e}", tc.input)),
                Ok(None) if !tc.expects_nil() => fails.push(format!("{system}[{}]: nil", tc.input)),
                Ok(None) => sys_passed += 1,
                Ok(Some(result)) => {
                    if tc.expects_nil() {
                        fails.push(format!("{system}[{}]: expected nil", tc.input));
                    } else if result.text != tc.output {
                        fails.push(format!(
                            "{system}[{}]:\n    expected {:?}\n    got      {:?}",
                            tc.input, tc.output, result.text
                        ));
                    } else if result.secret != tc.secret {
                        fails.push(format!("{system}[{}]: secret flag mismatch", tc.input));
                    } else if result.success != tc.success
                        || result.failure != tc.failure
                        || result.critical != tc.critical
                        || result.fumble != tc.fumble
                    {
                        fails.push(format!("{system}[{}]: result flag mismatch", tc.input));
                    } else if !rng.is_empty() {
                        fails.push(format!(
                            "{system}[{}]: unconsumed rands ({})",
                            tc.input,
                            rng.remaining()
                        ));
                    } else {
                        sys_passed += 1;
                    }
                }
            }
        }
        println!("{system}: {sys_passed}/{}", data.tests.len());
        passed += sys_passed;
    }

    eprintln!("B04 TOTAL {passed}/{total}");
    assert!(total > 0, "no cases run");
    eprintln!("failures (first 40):");
    for f in fails.iter().take(40) {
        eprintln!("  {f}");
    }
    assert_eq!(passed, total, "B04 failures: {}", fails.len());
}
