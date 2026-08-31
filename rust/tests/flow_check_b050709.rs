//! flow-controller検証: B05/B07/B09 対象システムのTOMLケース全件検証ランナー。
use std::path::{Path, PathBuf};

use bcdice::eval::eval_command;
use bcdice::game_system::GameSystemId;
use bcdice::randomizer::SeededRandomizer;
use bcdice::toml_test::TestDataFile;

/// 主要対象システムのスポット検証（B09: PersonaO系 / B05: ScreamHighSchool / B07: EtrianOdysseySRS）
const SPOT_SYSTEMS: &[&str] = &[
    "PersonaO",
    "AngelGear",
    "DemonSpike",
    "DiceOfTheDead",
    "Paradiso",
    "Strave",
    "TrailOfCthulhu",
    "WitchQuest",
    "SevenFortressMobius",
    "ScreamHighSchool",
    "DarkSouls",
    "EtrianOdysseySRS",
];

#[test]
fn flow_check_spot_systems() {
    let data_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("test/data");

    let mut total = 0usize;
    let mut passed = 0usize;
    let mut fails: Vec<String> = Vec::new();

    for system in SPOT_SYSTEMS {
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
                            "{system}[{}]: expected {:?} got {:?}",
                            tc.input, tc.output, result.text
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

    eprintln!("TOTAL {passed}/{total}");
    assert!(total > 0, "no cases run");
    eprintln!("failures (first 40):");
    for f in fails.iter().take(40) {
        eprintln!("  {f}");
    }
    assert_eq!(passed, total, "spot check failures: {}", fails.len());
}
