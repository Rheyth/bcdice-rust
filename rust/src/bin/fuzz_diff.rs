//! 差分ファズ 突き合わせツール（P5・成果物3の後半）
//!
//! Ruby側 `bin/fuzz_runner.rb` と Rust側 `fuzz_runner` が書いた JSON Lines を
//! 1行ずつ対応付けて比較し、不一致リストと種別（`kind`）ごとの集計を出す。
//! **Rubyを正**とし、`expected` は常にRuby側、`actual` は常にRust側の値。
//!
//! # 使い方
//!
//! ```sh
//! cd rust
//! cargo run --release --bin fuzz_diff -- ../reports/fuzz_ruby.jsonl ../reports/fuzz_rust.jsonl
//! cargo run --release --bin fuzz_diff -- ../reports/fuzz_ruby.jsonl ../reports/fuzz_rust.jsonl \
//!   --report ../reports/fuzz_diff_report.md --max 200
//! ```
//!
//! - `--report <path>`: Markdown の詳細レポート（入力・期待(Ruby)・実際(Rust)）を書く
//! - `--max <N>`: 標準出力に出す不一致の件数上限（既定 30。レポートには全件書く）
//!
//! 終了コードは全件一致で 0、不一致が1件でもあれば 1。
//!
//! # 比較対象
//!
//! `nil_result` / `output` / `secret` / `success` / `failure` / `critical` / `fumble` /
//! `rands` / `error`。`error_detail` は言語ごとに文言が異なるため**比較しない**
//! （表示のみ）。

use std::collections::BTreeMap;
use std::io::Write;

use serde::Deserialize;

/// 両ランナーが書く1行。
#[derive(Debug, Clone, Deserialize, PartialEq)]
struct ResultRow {
    id: String,
    kind: String,
    input: String,
    nil_result: bool,
    output: Option<String>,
    secret: bool,
    success: bool,
    failure: bool,
    critical: bool,
    fumble: bool,
    rands: Vec<[i64; 2]>,
    error: Option<String>,
    #[serde(default)]
    error_detail: Option<String>,
}

/// 1フィールドの不一致。
#[derive(Debug, Clone)]
struct FieldDiff {
    field: &'static str,
    expected: String,
    actual: String,
}

/// 1ケースの不一致。
#[derive(Debug, Clone)]
struct CaseDiff {
    id: String,
    kind: String,
    input: String,
    diffs: Vec<FieldDiff>,
    ruby_detail: Option<String>,
    rust_detail: Option<String>,
}

fn quote(s: &Option<String>) -> String {
    match s {
        None => "null".to_string(),
        Some(s) => format!("{s:?}"),
    }
}

/// 出目列の差分を、巨大配列でも読める形に要約する。
fn describe_rands(expected: &[[i64; 2]], actual: &[[i64; 2]]) -> (String, String) {
    let first_diff = expected
        .iter()
        .zip(actual.iter())
        .position(|(a, b)| a != b)
        .unwrap_or(expected.len().min(actual.len()));

    let render = |list: &[[i64; 2]]| {
        let start = first_diff.saturating_sub(2).min(list.len());
        let end = (first_diff + 3).min(list.len());
        let window: Vec<String> = list[start..end]
            .iter()
            .map(|r| format!("[{},{}]", r[0], r[1]))
            .collect();
        format!(
            "len={} first_diff@{} ...{}...",
            list.len(),
            first_diff,
            window.join(",")
        )
    };

    (render(expected), render(actual))
}

/// 1ケースを比較する。Rubyを正とする。
fn compare(ruby: &ResultRow, rust: &ResultRow) -> Vec<FieldDiff> {
    let mut diffs = Vec::new();

    macro_rules! cmp_bool {
        ($field:ident) => {
            if ruby.$field != rust.$field {
                diffs.push(FieldDiff {
                    field: stringify!($field),
                    expected: ruby.$field.to_string(),
                    actual: rust.$field.to_string(),
                });
            }
        };
    }

    cmp_bool!(nil_result);
    if ruby.output != rust.output {
        diffs.push(FieldDiff {
            field: "output",
            expected: quote(&ruby.output),
            actual: quote(&rust.output),
        });
    }
    cmp_bool!(secret);
    cmp_bool!(success);
    cmp_bool!(failure);
    cmp_bool!(critical);
    cmp_bool!(fumble);

    if ruby.rands != rust.rands {
        let (expected, actual) = describe_rands(&ruby.rands, &rust.rands);
        diffs.push(FieldDiff {
            field: "rands",
            expected,
            actual,
        });
    }

    if ruby.error != rust.error {
        diffs.push(FieldDiff {
            field: "error",
            expected: quote(&ruby.error),
            actual: quote(&rust.error),
        });
    }

    diffs
}

fn load(path: &str) -> std::io::Result<Vec<ResultRow>> {
    let text = std::fs::read_to_string(path)?;
    let mut rows = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<ResultRow>(line) {
            Ok(row) => rows.push(row),
            Err(e) => {
                return Err(std::io::Error::other(format!(
                    "{path}:{}: invalid JSONL: {e}",
                    i + 1
                )))
            }
        }
    }
    Ok(rows)
}

/// 種別ごとの集計。
#[derive(Debug, Default, Clone, Copy)]
struct KindStat {
    total: usize,
    mismatched: usize,
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let (Some(ruby_path), Some(rust_path)) = (args.get(1), args.get(2)) else {
        eprintln!("Usage: fuzz_diff <ruby.jsonl> <rust.jsonl> [--report <path.md>] [--max <N>]");
        std::process::exit(2);
    };

    let mut report_path: Option<String> = None;
    let mut max_shown = 30usize;
    let mut i = 3;
    while i < args.len() {
        match args[i].as_str() {
            "--report" => {
                report_path = args.get(i + 1).cloned();
                i += 2;
            }
            "--max" => {
                max_shown = args
                    .get(i + 1)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(max_shown);
                i += 2;
            }
            other => {
                eprintln!("unknown option: {other}");
                std::process::exit(2);
            }
        }
    }

    let ruby = load(ruby_path)?;
    let rust = load(rust_path)?;

    if ruby.len() != rust.len() {
        eprintln!(
            "line count mismatch: ruby={} rust={} (両ランナーを同じ入力で走らせ直すこと)",
            ruby.len(),
            rust.len()
        );
        std::process::exit(2);
    }

    let mut kinds: BTreeMap<String, KindStat> = BTreeMap::new();
    let mut field_counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut case_diffs: Vec<CaseDiff> = Vec::new();

    for (r, t) in ruby.iter().zip(rust.iter()) {
        if r.id != t.id || r.input != t.input {
            eprintln!(
                "row misalignment at id ruby={:?} rust={:?} (input ruby={:?} rust={:?})",
                r.id, t.id, r.input, t.input
            );
            std::process::exit(2);
        }

        let stat = kinds.entry(r.kind.clone()).or_default();
        stat.total += 1;

        let diffs = compare(r, t);
        if diffs.is_empty() {
            continue;
        }
        stat.mismatched += 1;
        for d in &diffs {
            *field_counts.entry(d.field).or_default() += 1;
        }
        case_diffs.push(CaseDiff {
            id: r.id.clone(),
            kind: r.kind.clone(),
            input: r.input.clone(),
            diffs,
            ruby_detail: r.error_detail.clone(),
            rust_detail: t.error_detail.clone(),
        });
    }

    let total = ruby.len();
    let mismatched = case_diffs.len();

    println!("fuzz_diff: {}/{} cases matched", total - mismatched, total);
    println!("  ruby: {ruby_path}");
    println!("  rust: {rust_path}");

    if mismatched == 0 {
        println!("\n全件一致");
    } else {
        println!("\n不一致フィールド別:");
        for (field, n) in &field_counts {
            println!("  {field:<12} {n:5}");
        }
        println!("\n種別（kind）別:");
        for (kind, stat) in &kinds {
            let mark = if stat.mismatched == 0 { ' ' } else { '!' };
            println!(
                "  {mark} {kind:<24} {:5}/{:<5} matched",
                stat.total - stat.mismatched,
                stat.total
            );
        }
        println!("\n不一致（Rubyが正）:");
        for case in case_diffs.iter().take(max_shown) {
            println!("\n  {} [{}] {:?}", case.id, case.kind, case.input);
            for d in &case.diffs {
                println!("    {}:", d.field);
                println!("      expected(ruby): {}", d.expected);
                println!("      actual  (rust): {}", d.actual);
            }
        }
        if mismatched > max_shown {
            println!(
                "\n  ... and {} more (--max で増やせる)",
                mismatched - max_shown
            );
        }
    }

    if let Some(path) = report_path {
        write_report(&path, total, &kinds, &field_counts, &case_diffs)?;
        println!("\nreport -> {path}");
    }

    std::process::exit(if mismatched == 0 { 0 } else { 1 });
}

fn write_report(
    path: &str,
    total: usize,
    kinds: &BTreeMap<String, KindStat>,
    field_counts: &BTreeMap<&'static str, usize>,
    case_diffs: &[CaseDiff],
) -> std::io::Result<()> {
    if let Some(parent) = std::path::Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let mut f = std::fs::File::create(path)?;

    writeln!(f, "# 差分ファズ レポート（fuzz_diff 生成物）\n")?;
    writeln!(
        f,
        "本ファイルは `fuzz_diff --report` の生成物。手で編集しない。"
    )?;
    writeln!(
        f,
        "恒久的な既知不一致の記録は `reports/fuzz_known_diffs.md` 側に書く。\n"
    )?;
    writeln!(
        f,
        "- 総ケース数: {total}\n- 一致: {}\n- 不一致: {}\n",
        total - case_diffs.len(),
        case_diffs.len()
    )?;

    writeln!(f, "## 不一致フィールド別\n")?;
    if field_counts.is_empty() {
        writeln!(f, "なし（全件一致）\n")?;
    } else {
        writeln!(f, "| フィールド | 件数 |\n|---|---:|")?;
        for (field, n) in field_counts {
            writeln!(f, "| `{field}` | {n} |")?;
        }
        writeln!(f)?;
    }

    writeln!(f, "## 種別（kind）別\n")?;
    writeln!(f, "| kind | 一致 | 総数 |\n|---|---:|---:|")?;
    for (kind, stat) in kinds {
        writeln!(
            f,
            "| `{kind}` | {} | {} |",
            stat.total - stat.mismatched,
            stat.total
        )?;
    }
    writeln!(f)?;

    if case_diffs.is_empty() {
        return Ok(());
    }

    writeln!(f, "## 不一致一覧（Rubyが正）\n")?;
    for case in case_diffs {
        writeln!(f, "### `{}` [{}]\n", case.id, case.kind)?;
        writeln!(f, "- 入力: `{}`", case.input.replace('`', "\\`"))?;
        for d in &case.diffs {
            writeln!(f, "- `{}`", d.field)?;
            writeln!(f, "  - 期待(Ruby): `{}`", d.expected.replace('`', "\\`"))?;
            writeln!(f, "  - 実際(Rust): `{}`", d.actual.replace('`', "\\`"))?;
        }
        if case.ruby_detail.is_some() || case.rust_detail.is_some() {
            writeln!(
                f,
                "- error_detail（比較対象外）: ruby={:?} / rust={:?}",
                case.ruby_detail, case.rust_detail
            )?;
        }
        writeln!(f)?;
    }
    Ok(())
}
