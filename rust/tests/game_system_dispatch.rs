//! trait経由のゲームシステム振り分けの検証。
//!
//! P3-Batch1 の完了条件「ダミーのダイスボットを trait で登録し、TOMLハーネスから
//! 呼べること」を、ハーネスと同じ入口（TOML → `eval_command`）で確かめる。

use std::borrow::Cow;

use bcdice::eval::{eval_command, eval_raw, EvalError, EvalResult};
use bcdice::game_system::{
    game_system_class, game_systems, GameSystem, GameSystemId, SpecificCommandOutput,
};
use bcdice::randomizer::{Randomizer, SeededRandomizer};
use bcdice::toml_test::{TestCase, TestDataFile};

/// TOMLテストケースをハーネスと同じ手順で実行し、期待出力と一致するか調べる。
fn run_toml_case(tc: &TestCase) -> Result<(), String> {
    let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
    let mut randomizer = SeededRandomizer::new(rands);
    let system = GameSystemId::new(tc.game_system.clone());

    let outcome = eval_command(&system, &tc.input, &mut randomizer)
        .map_err(|e| format!("eval error: {e}"))?;

    match outcome {
        None if tc.expects_nil() => {}
        None => return Err(format!("expected {:?}, got nil", tc.output)),
        Some(result) => {
            if tc.expects_nil() {
                return Err(format!("expected nil, got {:?}", result.text));
            }
            if result.text != tc.output {
                return Err(format!("expected {:?}, got {:?}", tc.output, result.text));
            }
            if result.secret != tc.secret {
                return Err(format!("secret flag mismatch on {:?}", tc.input));
            }
            if result.success != tc.success {
                return Err(format!("success flag mismatch on {:?}", tc.input));
            }
        }
    }

    if !randomizer.is_empty() {
        return Err(format!("unconsumed rands ({})", randomizer.remaining()));
    }
    Ok(())
}

/// ダミーシステムを TOML 形式のテストデータから実行できること。
#[test]
fn dummy_system_runs_through_toml_harness_path() {
    const DATA: &str = r#"
[[ test ]]
game_system = "DummySystem"
input = "DUMT"
output = "ダミー表(4) ＞ ダミー4"
rands = [ { sides = 6, value = 4 } ]

[[ test ]]
game_system = "DummySystem"
input = "SDUMT"
output = "ダミー表(5) ＞ ダミー5"
secret = true
rands = [ { sides = 6, value = 5 } ]

[[ test ]]
game_system = "DummySystem"
input = "DUMC"
output = "ダミー判定 ＞ 成功"
success = true

# 固有コマンドが "1" を返した場合は nil に畳まれる
[[ test ]]
game_system = "DummySystem"
input = "DUM"
output = ""

# 接頭辞に一致しない入力は共通コマンドへ流れる（sort_barabara_dice = true）
[[ test ]]
game_system = "DummySystem"
input = "3B6"
output = "(3B6) ＞ 1,3,5"
rands = [
  { sides = 6, value = 5 },
  { sides = 6, value = 1 },
  { sides = 6, value = 3 },
]
"#;

    let data = TestDataFile::parse_str(std::path::Path::new("<inline>"), DATA).expect("parse");
    assert_eq!(data.tests.len(), 5);
    for (i, tc) in data.tests.iter().enumerate() {
        if let Err(reason) = run_toml_case(tc) {
            panic!("case {} ({:?}) failed: {reason}", i + 1, tc.input);
        }
    }
}

/// レジストリに無いIDは `SystemNotImplemented`（ハーネスがfail理由に使う）。
#[test]
fn unregistered_id_reports_system_not_implemented() {
    let mut src = SeededRandomizer::new(Vec::new());
    let err = eval_command(&GameSystemId::new("NoSuchGameSystem"), "1D6", &mut src)
        .expect_err("must not be registered");
    assert_eq!(err, EvalError::SystemNotImplemented);
}

/// 生成済みシステムの固有コマンドは、P4まで `NotImplemented` になる。
///
/// 既定の `Ok(None)` に落とすと未実装の固有コマンドが黙って汎用コマンドへ
/// フォールスルーし、誤った出力を返してしまう。そうなっていないことの確認。
#[test]
fn generated_system_specific_command_reports_not_implemented() {
    let mut src = SeededRandomizer::new(Vec::new());
    let err = eval_command(&GameSystemId::new("Cthulhu7th"), "CC<=50", &mut src)
        .expect_err("game system specific command is not ported yet");
    assert_eq!(err, EvalError::NotImplemented);
    assert!(err
        .to_string()
        .contains("game system specific command not implemented"));
}

/// レジストリの内容。
///
/// 336 = Ruby `BCDice.all_game_systems.size`（DiceBot を含む実測値）。
/// これに Batch1 のインフラ検証用 `DummySystem` を足した337件になる。
#[test]
fn registry_contains_expected_systems() {
    let systems = game_systems();
    assert_eq!(systems.len(), 337, "336 Ruby systems + DummySystem");

    let ids: Vec<&str> = systems.iter().map(|s| s.id()).collect();

    let mut sorted = ids.clone();
    sorted.sort_unstable();
    let mut unique = sorted.clone();
    unique.dedup();
    assert_eq!(sorted, unique, "game system ids must be unique");

    assert!(ids.contains(&"DiceBot"));
    assert!(ids.contains(&"DummySystem"));
    // 生成物側の代表。IDとクラス名が食い違うもの（Rust型名は SwordWorld2_5）も引ける
    assert!(ids.contains(&"SwordWorld2.5"));
    assert!(ids.contains(&"Arianrhod:Korean"));

    for id in &ids {
        assert_eq!(game_system_class(id).expect("registered").id(), *id);
    }
}

/// 全システムの定数が空でないこと（生成漏れ・空文字列の混入検出）。
#[test]
fn every_system_has_non_empty_constants() {
    for system in game_systems() {
        assert!(!system.id().is_empty(), "empty id");
        assert!(!system.name().is_empty(), "empty name for {}", system.id());
        assert!(
            !system.sort_key().is_empty(),
            "empty sort_key for {}",
            system.id()
        );
        assert!(
            !system.help_message().is_empty(),
            "empty help_message for {}",
            system.id()
        );
    }
}

/// 全システムの接頭辞が `regex` クレートで解釈できること。
///
/// `prefixes` は Ruby の正規表現断片なので、Oniguruma 固有の構文
/// （先読み・後方参照など）が入っていると `build_prefixes_pattern` がパニックする。
/// 3864件の接頭辞を一度に踏んで、そうなっていないことを確かめる。
#[test]
fn every_system_prefixes_pattern_compiles() {
    let mut with_prefixes = 0usize;
    let mut prefix_count = 0usize;
    for system in game_systems() {
        prefix_count += system.prefixes().len();
        match system.prefixes_pattern() {
            Some(re) => {
                with_prefixes += 1;
                assert!(
                    re.as_str().starts_with("(?i)^(S)?("),
                    "unexpected pattern shape for {}: {}",
                    system.id(),
                    re.as_str()
                );
            }
            // 接頭辞を持たないシステムは Ruby の `/(?!)/` に対応して None
            None => assert!(
                system.prefixes().is_empty(),
                "{} has prefixes but no pattern",
                system.id()
            ),
        }
    }
    // 実測値: Ruby 336システムのうち313が接頭辞を持つ。DummySystem を足して314。
    // (P4移植の進行に伴い 312 から 2 増加)
    assert_eq!(with_prefixes, 314, "systems with prefixes");
    // Ruby側の接頭辞は3865件（うち1件は Arianrhod:Korean の nil 由来の空文字列）。
    // DummySystem の "DUM" を足して3866。
    assert_eq!(prefix_count, 3866, "total prefixes (incl. DummySystem)");
}

// ---- prefixes_pattern のキャッシュがシステムごとに独立していることの検証 ----
//
// `impl_prefixes_pattern!` の `static OnceLock` を trait の既定実装本体に置くと、
// 全システムが最初に初期化された1つの正規表現を共有してしまう。
// 接頭辞の異なる2システムを別々に初期化し、取り違えが起きないことを確かめる。

struct AlphaSystem;
struct BetaSystem;

impl GameSystem for AlphaSystem {
    fn id(&self) -> &'static str {
        "AlphaSystem"
    }
    fn name(&self) -> &'static str {
        "Alpha"
    }
    fn sort_key(&self) -> &'static str {
        "あるふあ"
    }
    fn help_message(&self) -> &'static str {
        "ALPHA"
    }
    fn prefixes(&self) -> &'static [&'static str] {
        &["ALPHA"]
    }
    bcdice::impl_prefixes_pattern!();

    fn change_text<'a>(&self, text: &'a str) -> Cow<'a, str> {
        // change_text がゲームシステムごとに効くことも同時に確かめる
        Cow::Owned(text.replace('＠', "@"))
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        _rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        Ok(Some(SpecificCommandOutput::text(format!(
            "alpha:{command}"
        ))))
    }
}

impl GameSystem for BetaSystem {
    fn id(&self) -> &'static str {
        "BetaSystem"
    }
    fn name(&self) -> &'static str {
        "Beta"
    }
    fn sort_key(&self) -> &'static str {
        "へえた"
    }
    fn help_message(&self) -> &'static str {
        "BETA"
    }
    fn prefixes(&self) -> &'static [&'static str] {
        &["BETA"]
    }
    bcdice::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        _rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        Ok(Some(SpecificCommandOutput::text(format!("beta:{command}"))))
    }
}

fn eval_text(system: &dyn GameSystem, input: &str) -> Option<EvalResult> {
    let mut src = SeededRandomizer::new(Vec::new());
    let mut rng = Randomizer::new(&mut src);
    eval_raw(system, input, &mut rng).expect("no eval error")
}

#[test]
fn prefixes_pattern_is_cached_per_system() {
    // Alpha を先に初期化しても Beta が Alpha のパターンを使わないこと
    assert_eq!(
        eval_text(&AlphaSystem, "ALPHAX").map(|r| r.text),
        Some("alpha:ALPHAX".to_string())
    );
    assert!(eval_text(&AlphaSystem, "BETAX").is_none());

    assert_eq!(
        eval_text(&BetaSystem, "BETAX").map(|r| r.text),
        Some("beta:BETAX".to_string())
    );
    assert!(eval_text(&BetaSystem, "ALPHAX").is_none());

    assert_eq!(
        AlphaSystem.prefixes_pattern().map(|r| r.as_str()),
        Some("(?i)^(S)?(ALPHA)")
    );
    assert_eq!(
        BetaSystem.prefixes_pattern().map(|r| r.as_str()),
        Some("(?i)^(S)?(BETA)")
    );
}

#[test]
fn change_text_is_applied_per_system() {
    // Preprocessor と eval_common_command の両方で change_text が呼ばれる
    assert_eq!(
        eval_text(&AlphaSystem, "ALPHA＠1").map(|r| r.text),
        Some("alpha:ALPHA@1".to_string())
    );
}
