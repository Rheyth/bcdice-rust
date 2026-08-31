// flow-controller spot verification for B05/B07 remaining systems (auto-generated header check via runtime eval)
use bcdice::eval::eval_command;
use bcdice::game_system::GameSystemId;
use bcdice::randomizer::SeededRandomizer;
use bcdice::toml_test::TestDataFile;
use std::path::{Path, PathBuf};

const B05B07: &[&str] = &[
    "ScreamHighSchool",
    "GoblinSlayer",
    "Airgetlamh",
    "DesperateRun",
    "TrinitySeven",
    "DarkDaysDrive",
    "LiverLabyrinth",
    "UnsungDuet",
    "NinjaSlayer",
    "SamsaraBallad",
    "VisionConnect",
    "GardenOrderReEdit",
    "Garako",
    "NegikureNegimaki",
    "YankeeYogSothoth",
    "LostRoyal",
    "NightmareHunterDeep",
    "EtrianOdysseySRS",
    "NinjaSlayer2",
    "StrangerOfSwordCity",
    "GranCrest",
    "AniMalus",
    "Postman",
    "TwilightGunsmoke",
    "BBN",
    "NSSQ",
    "YuMyoKishi",
    "TokyoGhostResearch",
    "LiveraDoll",
    "SajinsenkiAGuS",
];

#[test]
fn flow_check_b05b07() {
    let data_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("test/data");
    let mut total = 0usize;
    let mut passed = 0usize;
    let mut fails: Vec<String> = Vec::new();
    for system in B05B07 {
        let path: PathBuf = data_dir.join(format!("{system}.toml"));
        let data = match TestDataFile::load(&path) {
            Ok(d) => d,
            Err(e) => {
                fails.push(format!("{system}: load {e}"));
                continue;
            }
        };
        let mut sp = 0usize;
        for tc in data.tests.iter() {
            total += 1;
            let mut rng = SeededRandomizer::new(tc.rands.iter().map(|r| (r.value, r.sides)));
            match eval_command(&GameSystemId::new(*system), &tc.input, &mut rng) {
                Err(e) => fails.push(format!("{system}[{}]: err {e}", tc.input)),
                Ok(None) if !tc.expects_nil() => fails.push(format!("{system}[{}]: nil", tc.input)),
                Ok(None) => sp += 1,
                Ok(Some(r)) => {
                    if tc.expects_nil() {
                        fails.push(format!("{system}[{}]: want nil", tc.input));
                    } else if r.text != tc.output {
                        fails.push(format!(
                            "{system}[{}]: want {:?} got {:?}",
                            tc.input, tc.output, r.text
                        ));
                    } else {
                        sp += 1;
                    }
                }
            }
        }
        println!("{system}: {sp}/{}", data.tests.len());
        passed += sp;
    }
    eprintln!("B05/B07 TOTAL {passed}/{total}");
    for f in fails.iter().take(50) {
        eprintln!("  {f}");
    }
    assert_eq!(passed, total, "b05b07 failures: {}", fails.len());
}
