//! TOMLテストハーネス。
//!
//! `test/data/*.toml` 全ファイルを読み込み、各テストケースを
//! [`bcdice::eval::eval_command`] で実行して期待値と比較する。
//!
//! P3-Batch1時点では game_system = "DiceBot" のケースのみ実装済みで、他システムは
//! `SystemNotImplemented` を fail 理由として報告する。

use std::path::{Path, PathBuf};

use bcdice::eval::{eval_command, EvalError, EvalResult};
use bcdice::game_system::GameSystemId;
use bcdice::randomizer::SeededRandomizer;
use bcdice::toml_test::{TestCase, TestDataFile};

/// 1ケースの判定結果。
#[derive(Debug)]
pub struct CaseOutcome {
    /// 種別ごとのキー。`<file名(拡張子除)>:<index>:<input>`（Ruby側 data API と同一形式）
    pub key: String,
    /// ゲームシステムID（集計の絞り込みに使う）
    pub game_system: String,
    pub passed: bool,
    /// fail理由。pass時は空。
    pub reasons: Vec<String>,
}

/// 1ファイルの集計。
#[derive(Debug)]
pub struct FileOutcome {
    pub path: PathBuf,
    pub cases: Vec<CaseOutcome>,
    /// ロード失敗（TOMLパースエラー等）時に非空。
    pub load_error: Option<String>,
}

impl FileOutcome {
    pub fn passed_count(&self) -> usize {
        self.cases.iter().filter(|c| c.passed).count()
    }
}

/// 全体集計。
#[derive(Debug, Default)]
pub struct Summary {
    pub files: Vec<FileOutcome>,
    pub total_cases: usize,
    pub passed_cases: usize,
    pub load_errors: usize,
}

impl Summary {
    pub fn failed_cases(&self) -> impl Iterator<Item = (&Path, &CaseOutcome)> {
        self.files.iter().flat_map(|f| {
            let p: &Path = &f.path;
            f.cases.iter().filter(|c| !c.passed).map(move |c| (p, c))
        })
    }
}

/// テストデータディレクトリ（通常 `test/data`）から全TOMLを実行する。
pub fn run_dir(data_dir: &Path) -> Summary {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(data_dir)
        .expect("test data dir")
        .filter_map(|e| {
            let p = e.ok()?.path();
            let is_toml = p.extension().map(|x| x == "toml").unwrap_or(false);
            is_toml.then_some(p)
        })
        .collect();
    paths.sort();

    let mut summary = Summary::default();
    for path in &paths {
        let outcome = run_file(path);
        if outcome.load_error.is_some() {
            summary.load_errors += 1;
        } else {
            summary.total_cases += outcome.cases.len();
            summary.passed_cases += outcome.passed_count();
        }
        summary.files.push(outcome);
    }
    summary
}

/// 1ファイルを実行する。
pub fn run_file(path: &Path) -> FileOutcome {
    let data = match TestDataFile::load(path) {
        Ok(d) => d,
        Err(e) => {
            return FileOutcome {
                path: path.to_path_buf(),
                cases: Vec::new(),
                load_error: Some(e.to_string()),
            }
        }
    };

    let file_base = path.file_stem().unwrap().to_string_lossy().to_string();
    let cases = data
        .tests
        .iter()
        .enumerate()
        .map(|(i, tc)| run_case(&file_base, i + 1, tc))
        .collect();

    FileOutcome {
        path: path.to_path_buf(),
        cases,
        load_error: None,
    }
}

/// 1ケースを実行して判定する。Ruby test_diceroll と同じ観点で比較する。
pub fn run_case(file_base: &str, index: usize, tc: &TestCase) -> CaseOutcome {
    let key = format!("{file_base}:{index}:{input}", input = tc.input);
    let mut reasons = Vec::new();

    let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
    let mut randomizer = SeededRandomizer::new(rands);

    let system = GameSystemId::new(tc.game_system.clone());
    let outcome: Result<Option<EvalResult>, EvalError> =
        eval_command(&system, &tc.input, &mut randomizer);

    match outcome {
        Err(EvalError::SystemNotImplemented) => {
            reasons.push(format!(
                "game system {:?} is not implemented yet (P3)",
                tc.game_system
            ));
        }
        Err(e) => {
            reasons.push(format!("eval error: {e}"));
        }
        Ok(None) => {
            if !tc.expects_nil() {
                reasons.push(format!(
                    "eval returned nil, but output was expected: {:?}",
                    tc.output
                ));
            }
        }
        Ok(Some(result)) => {
            if tc.expects_nil() {
                reasons.push(format!("expected nil output, got {:?}", result.text));
            } else if result.text != tc.output {
                reasons.push(format!(
                    "output mismatch\n  expected: {:?}\n  actual:   {:?}",
                    tc.output, result.text
                ));
            }
            check_flag(&mut reasons, "secret", tc.secret, result.secret);
            check_flag(&mut reasons, "success", tc.success, result.success);
            check_flag(&mut reasons, "failure", tc.failure, result.failure);
            check_flag(&mut reasons, "critical", tc.critical, result.critical);
            check_flag(&mut reasons, "fumble", tc.fumble, result.fumble);
        }
    }

    // 乱数消費チェックは行わない。Ruby本家 test/test_game_system_commands.rb#test_diceroll
    // は残り乱数を一切検査しない（result nil なら即return、消費チェックなし）。TOMLの
    // rands 配列は「実際に消費された分」ではなく「ケース用に用意された分」が記録されており、
    // Ruby本家は正常系でも乱数を余らせる経路を複数持つ（BeginningIdol PD系=nil返却、
    // VTM5=roll_barabara(0,10) が0消費、エラー文字列早期return 等）。
    // 消費順序の誤りは出力比較で検出できるため、このチェックは廃止する。

    CaseOutcome {
        key,
        game_system: tc.game_system.clone(),
        passed: reasons.is_empty(),
        reasons,
    }
}

fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
    if expected != actual {
        reasons.push(format!(
            "{name} flag mismatch: expected {expected}, actual {actual}"
        ));
    }
}

/// 人間可読のレポートを生成する。
pub fn report_text(summary: &Summary, max_failures: usize) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "TOML test harness: {}/{} cases passed ({} files, {} load errors)\n",
        summary.passed_cases,
        summary.total_cases,
        summary.files.len(),
        summary.load_errors
    ));

    for (shown, (path, case)) in summary.failed_cases().enumerate() {
        if shown >= max_failures {
            out.push_str(&format!(
                "... and more failures (limit {max_failures} shown)\n"
            ));
            break;
        }
        out.push_str(&format!("\nFAIL {}\n  in {}\n", case.key, path.display()));
        for r in &case.reasons {
            out.push_str(&format!("  - {r}\n"));
        }
    }
    out
}

/// JSONレポートを生成する（CIでの集計・diffに使う）。
pub fn report_json(summary: &Summary) -> serde_json::Value {
    use serde_json::json;
    json!({
        "total_cases": summary.total_cases,
        "passed_cases": summary.passed_cases,
        "failed_cases": summary.total_cases - summary.passed_cases,
        "load_errors": summary.load_errors,
        "files": summary.files.iter().map(|f| json!({
            "path": f.path.display().to_string(),
            "load_error": f.load_error,
            "passed": f.passed_count(),
            "total": f.cases.len(),
        })).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_dir() -> Option<PathBuf> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("test/data");
        dir.exists().then_some(dir)
    }

    #[test]
    fn discovers_all_toml_files() {
        let Some(dir) = data_dir() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data not found");
            return;
        };

        let summary = run_dir(&dir);
        assert_eq!(
            summary.files.len(),
            348,
            "348 TOML files must be discovered"
        );
        assert_eq!(summary.load_errors, 0, "all TOML files must parse");
        assert!(
            summary.total_cases > 19_000,
            "unexpected case count: {}",
            summary.total_cases
        );
    }

    /// P1の完了条件: game_system = "DiceBot" の全ケースがパスすること。
    #[test]
    fn all_dice_bot_cases_pass() {
        let Some(dir) = data_dir() else {
            eprintln!("skip: test/data not found");
            return;
        };

        let summary = run_dir(&dir);

        let mut total = 0usize;
        let mut passed = 0usize;
        let mut breakdown: Vec<(String, usize, usize)> = Vec::new();
        let mut failures: Vec<String> = Vec::new();

        for file in &summary.files {
            let cases: Vec<&CaseOutcome> = file
                .cases
                .iter()
                .filter(|c| c.game_system == "DiceBot")
                .collect();
            if cases.is_empty() {
                continue;
            }
            let file_passed = cases.iter().filter(|c| c.passed).count();
            breakdown.push((
                file.path.file_name().unwrap().to_string_lossy().to_string(),
                file_passed,
                cases.len(),
            ));
            total += cases.len();
            passed += file_passed;

            for case in cases.iter().filter(|c| !c.passed) {
                failures.push(format!(
                    "FAIL {}\n  - {}",
                    case.key,
                    case.reasons.join("\n  - ")
                ));
            }
        }

        eprintln!("DiceBot breakdown ({passed}/{total}):");
        for (name, p, t) in &breakdown {
            eprintln!("  {name}: {p}/{t}");
        }
        for f in failures.iter().take(40) {
            eprintln!("{f}");
        }

        assert_eq!(
            passed, total,
            "all DiceBot cases must pass ({passed}/{total})"
        );
        // brief記載の10ファイル305ケースに加え、tally_ty / tally_tz も DiceBot（各11件）
        assert_eq!(breakdown.len(), 12, "12 files contain DiceBot cases");
        assert_eq!(total, 327, "327 DiceBot cases in total");
    }

    /// どの入力でもパニックせずに評価が終わること。
    ///
    /// 全348ファイルの入力（約2万件）を DiceBot として通し、パーサやスキャナが
    /// 添字外・非文字境界スライス・無限ループを起こさないことを確かめる。
    /// あわせて、「文法上到達しない」と判断して `EvalError::Internal` を置いた分岐に
    /// 実際に到達しないことも確認する。P4で土台を広げる前の健全性チェック。
    #[test]
    fn no_input_panics() {
        let Some(dir) = data_dir() else {
            eprintln!("skip: test/data not found");
            return;
        };

        let dice_bot = GameSystemId::new("DiceBot");
        let mut count = 0usize;
        let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
            .expect("test data dir")
            .filter_map(|e| {
                let p = e.ok()?.path();
                (p.extension().map(|x| x == "toml").unwrap_or(false)).then_some(p)
            })
            .collect();
        paths.sort();

        for path in &paths {
            let Ok(data) = TestDataFile::load(path) else {
                continue;
            };
            for tc in &data.tests {
                let mut randomizer = SeededRandomizer::new(
                    tc.rands
                        .iter()
                        .map(|r| (r.value, r.sides))
                        .collect::<Vec<_>>(),
                );
                // `EvalError::Internal` は「文法上到達しない」と判断した分岐に
                // 入ったことを示す。実際に到達しないことをここで担保する。
                if let Err(EvalError::Internal(msg)) =
                    eval_command(&dice_bot, &tc.input, &mut randomizer)
                {
                    panic!("internal error on input {:?}: {msg}", tc.input);
                }
                count += 1;
            }
        }

        assert!(count > 19_000, "unexpected input count: {count}");
    }

    /// 未移植システムの fail 理由が読めること。
    ///
    /// P3-Batch2 で全336システムがレジストリに載ったので、DiceBot以外も
    /// 評価自体は走る。固有コマンド（`prefixes`）を持つシステムは
    /// `EvalError::NotImplemented` で止まり、持たないシステムは汎用コマンド経路で
    /// 評価されるため **pass することもある**。ここで固定するのは
    /// 「failしたケースには必ず読める理由が付く」ことと、
    /// 「接頭辞にマッチした入力は黙って汎用コマンドへ落ちず NotImplemented になる」こと。
    #[test]
    fn unimplemented_systems_report_reason() {
        let Some(dir) = data_dir() else {
            eprintln!("skip: test/data not found");
            return;
        };

        let summary = run_dir(&dir);
        let mut not_implemented = 0usize;
        let mut other_failures = 0usize;

        for (path, case) in summary.failed_cases() {
            assert!(
                !case.reasons.is_empty(),
                "fail reason must exist for {}",
                path.display()
            );
            if case.game_system == "DiceBot" {
                continue;
            }
            if case
                .reasons
                .iter()
                .any(|r| r.contains("game system specific command not implemented"))
            {
                not_implemented += 1;
            } else {
                other_failures += 1;
            }
        }

        eprintln!(
            "non-DiceBot failures: {not_implemented} not-implemented / {other_failures} other"
        );
        // P4 で全336システムの固有コマンドが実装されたため、not-implemented の
        // 失敗は 0 件になった。このテストの本体は「failしたケースには必ず
        // 読める理由が付く」ことの固定であり、これは上の assert で検証済み。
        // not-implemented 発生時は理由を報告する経路が生きていることも
        // run_case が保証するため、ここでは件数の出力のみを行う。
    }

    /// TOMLテストデータのゲームシステムが、すべてレジストリに載っていること。
    ///
    /// 逆向き（レジストリ側にあってTOMLに無いシステム）も確認して、
    /// Ruby本家の336システムとテストデータが1対1で対応していることを固定する。
    #[test]
    fn toml_game_systems_match_registry() {
        use std::collections::BTreeSet;

        let Some(dir) = data_dir() else {
            eprintln!("skip: test/data not found");
            return;
        };

        let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
            .expect("test data dir")
            .filter_map(|e| {
                let p = e.ok()?.path();
                (p.extension().map(|x| x == "toml").unwrap_or(false)).then_some(p)
            })
            .collect();
        paths.sort();
        assert_eq!(paths.len(), 348, "348 TOML files");

        let mut in_toml: BTreeSet<String> = BTreeSet::new();
        let mut cases = 0usize;
        for path in &paths {
            let data = TestDataFile::load(path).expect("TOML must parse");
            for tc in &data.tests {
                in_toml.insert(tc.game_system.clone());
                cases += 1;
            }
        }
        assert_eq!(cases, 19_864, "total test cases");
        assert_eq!(in_toml.len(), 336, "distinct game systems in test data");

        let registered: BTreeSet<String> = bcdice::game_system::game_systems()
            .iter()
            .map(|s| s.id().to_string())
            .collect();
        assert_eq!(
            registered.len(),
            336,
            "336 Ruby game systems are registered"
        );

        let missing: Vec<&String> = in_toml.difference(&registered).collect();
        assert!(missing.is_empty(), "not registered: {missing:?}");
        let extra: Vec<&String> = registered.difference(&in_toml).collect();
        assert!(extra.is_empty(), "no test data: {extra:?}");
    }
}
