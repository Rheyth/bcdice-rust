//! `lib/bcdice/game_system/TokyoNova.rb` の手書き移植。

use crate::game_system::GameSystem;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokyoNova;

impl GameSystem for TokyoNova {
    fn id(&self) -> &'static str {
        "TokyoNova"
    }

    fn name(&self) -> &'static str {
        "トーキョーN◎VA"
    }

    fn sort_key(&self) -> &'static str {
        "とおきよおのは"
    }

    fn help_message(&self) -> &'static str {
        "※このダイスボットは部屋のシステム名表示用となります。\n"
    }
}

#[cfg(test)]
pub(crate) fn assert_toml_cases(system: &str, file: &str, expected_count: usize) {
    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;
    use std::path::Path;

    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("test/data")
        .join(file);
    if !path.exists() {
        eprintln!("skip: {} not found", path.display());
        return;
    }

    let data = TestDataFile::load(&path).unwrap_or_else(|e| panic!("{file} must parse: {e}"));
    assert_eq!(data.tests.len(), expected_count, "case count in {file}");
    let mut failures = Vec::new();
    for (index, tc) in data.tests.iter().enumerate() {
        assert_eq!(tc.game_system, system, "unexpected game system in {file}");
        let mut reasons = Vec::new();
        let mut src = SeededRandomizer::new(tc.rands.iter().map(|r| (r.value, r.sides)));
        match eval_command(&GameSystemId::new(system), &tc.input, &mut src) {
            Err(error) => reasons.push(format!("eval error: {error}")),
            Ok(None) if !tc.expects_nil() => reasons.push("eval returned nil".to_string()),
            Ok(None) => {}
            Ok(Some(result)) => {
                if tc.expects_nil() {
                    reasons.push(format!("expected nil, got {:?}", result.text));
                } else if result.text != tc.output {
                    reasons.push(format!(
                        "output mismatch\n    expected: {:?}\n    actual:   {:?}",
                        tc.output, result.text
                    ));
                }
                for (name, expected, actual) in [
                    ("secret", tc.secret, result.secret),
                    ("success", tc.success, result.success),
                    ("failure", tc.failure, result.failure),
                    ("critical", tc.critical, result.critical),
                    ("fumble", tc.fumble, result.fumble),
                ] {
                    if expected != actual {
                        reasons.push(format!(
                            "{name} flag mismatch: expected {expected}, actual {actual}"
                        ));
                    }
                }
            }
        }
        if !src.is_empty() {
            reasons.push(format!("unconsumed rands remain ({})", src.remaining()));
        }
        if !reasons.is_empty() {
            failures.push(format!(
                "FAIL {system}:{}:{}\n  - {}",
                index + 1,
                tc.input,
                reasons.join("\n  - ")
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{}/{} {system} cases failed:\n{}",
        failures.len(),
        data.tests.len(),
        failures.join("\n")
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        super::assert_toml_cases("TokyoNova", "TokyoNova.toml", 8);
    }
}
