//! 差分ファズ Rust側ランナー（P5・成果物3の前半）
//!
//! `bin/fuzz_patterns.rb` が生成した JSON Lines を読み、各入力を `DiceBot`（汎用）で
//! 評価して結果を JSON Lines に書き出す。Ruby側 `bin/fuzz_runner.rb` が同じ入力・
//! 同じ乱数源で同じ形式を書き、`fuzz_diff` が両者を突き合わせる。
//!
//! # 乱数源
//!
//! `LineupSource` は Ruby側 `LineupRandomizer` と同一式の決定的乱数源。
//! 入力1件ごとに新しいインスタンスを作るので、1件の不一致が以降のケースへ
//! 波及しない（各行が独立に再現可能）。

use std::io::{BufRead, BufWriter, Write};

use bcdice::eval::{eval_raw, EvalError};
use bcdice::game_system::{game_system_class, GameSystemId};
use bcdice::lineup_source::LineupSource;
use bcdice::randomizer::Randomizer;
use serde::{Deserialize, Serialize};

/// 入力JSONLの1行。`bin/fuzz_patterns.rb` の出力形式。
#[derive(Debug, Deserialize)]
struct InputRow {
    id: String,
    kind: String,
    input: String,
}

/// 出力JSONLの1行。Ruby側 `bin/fuzz_runner.rb` と同一のフィールド構成。
#[derive(Debug, Serialize)]
struct ResultRow {
    id: String,
    kind: String,
    input: String,
    /// eval が nil を返したか（Ruby `result.nil?` 相当）。例外時は false。
    nil_result: bool,
    output: Option<String>,
    secret: bool,
    success: bool,
    failure: bool,
    critical: bool,
    fumble: bool,
    rands: Vec<[i64; 2]>,
    /// 正規化した例外タグ。Ruby側と突き合わせるための閉じた集合。
    error: Option<&'static str>,
    /// 比較対象外の詳細（言語ごとに文言が違うため diff では表示のみ）。
    error_detail: Option<String>,
}

/// [`EvalError`] を Ruby側と共通のタグへ畳む。
fn error_tag(error: &EvalError) -> &'static str {
    match error {
        EvalError::TooManyRands => "TooManyRands",
        EvalError::ZeroDivision => "ZeroDivision",
        // Ruby側は NoMethodError / その他の StandardError を "Other" に畳む
        EvalError::NotImplemented
        | EvalError::UnrecognizedCommand
        | EvalError::BlankInput
        | EvalError::SystemNotImplemented
        | EvalError::FloatDomain
        | EvalError::Internal(_) => "Other",
        EvalError::RandSource(_) => "Panic",
    }
}

/// パニックのペイロードから人間可読なメッセージを取り出す。
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic payload".to_string()
    }
}

/// 1件を評価する。
fn evaluate(system: &GameSystemId, row: InputRow) -> ResultRow {
    let input = row.input.clone();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // DiceBotはレジストリに登録済みなのでNoneは来ない（念のためPanic扱いにする）
        let Some(game_system) = game_system_class(system.as_str()) else {
            return (Err(EvalError::SystemNotImplemented), Vec::new());
        };
        let mut source = LineupSource::new();
        let mut rng = Randomizer::new(&mut source);
        let result = eval_raw(game_system, &input, &mut rng);
        // 例外時も「そこまでに消費した出目」を返す（Ruby側 rand_results と同じ規約）
        let rands: Vec<[i64; 2]> = rng.rand_results().iter().map(|(v, s)| [*v, *s]).collect();
        (result, rands)
    }));

    let base = ResultRow {
        id: row.id,
        kind: row.kind,
        input: row.input,
        nil_result: false,
        output: None,
        secret: false,
        success: false,
        failure: false,
        critical: false,
        fumble: false,
        rands: Vec::new(),
        error: None,
        error_detail: None,
    };

    let (result, rands) = match outcome {
        Ok(pair) => pair,
        Err(payload) => {
            return ResultRow {
                error: Some("Panic"),
                error_detail: Some(panic_message(payload.as_ref())),
                ..base
            };
        }
    };

    match result {
        Ok(None) => ResultRow {
            nil_result: true,
            rands,
            ..base
        },
        Ok(Some(r)) => ResultRow {
            output: Some(r.text),
            secret: r.secret,
            success: r.success,
            failure: r.failure,
            critical: r.critical,
            fumble: r.fumble,
            rands,
            ..base
        },
        Err(e) => ResultRow {
            rands,
            error: Some(error_tag(&e)),
            error_detail: Some(e.to_string()),
            ..base
        },
    }
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let (Some(input_path), Some(output_path)) = (args.get(1), args.get(2)) else {
        eprintln!("Usage: fuzz_runner <inputs.jsonl> <results.jsonl>");
        std::process::exit(2);
    };

    // 意図的にパニックしうる入力を投げるので、既定のパニックメッセージは抑止する
    // （パニック内容は error_detail に記録される）
    std::panic::set_hook(Box::new(|_| {}));

    let system = GameSystemId::new("DiceBot");
    let file = std::fs::File::open(input_path)?;
    let reader = std::io::BufReader::new(file);

    if let Some(parent) = std::path::Path::new(output_path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let out = std::fs::File::create(output_path)?;
    let mut writer = BufWriter::new(out);

    let mut total = 0usize;
    let mut errors: std::collections::BTreeMap<&'static str, usize> = Default::default();

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let row: InputRow = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("invalid input JSONL line: {e}: {line}"));
        let result = evaluate(&system, row);
        total += 1;
        if let Some(tag) = result.error {
            *errors.entry(tag).or_default() += 1;
        }

        serde_json::to_writer(&mut writer, &result)?;
        writer.write_all(b"\n")?;
    }
    writer.flush()?;

    let _ = std::panic::take_hook();
    println!("fuzz_runner (rust): {total} cases -> {output_path}");
    for (tag, n) in &errors {
        println!("  error {tag:<14} {n:5}");
    }
    Ok(())
}
