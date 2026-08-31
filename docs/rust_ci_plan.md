# Rust移植 CI骨子（設計メモ・P0）

実インフラの構築は範囲外。この文書は実行手順と依存関係を確定したもの。
対応する実装:

- Rustハーネス: `rust/`（`cargo test` で `test/data/*.toml` 全件実行）
- Ruby基線ランナー: `bin/baseline_runner.rb`（verify / --regen）

## パイプライン全体像

```
upstream fetch → TOML同期 → racc生成 → Ruby基線 → Rustハーネス → 差分レポート
```

### 1. 環境準備

- Ruby: 3.2+（ruby:3.2 コンテナまたはローカル）。`bundle install` 済みであること
- Rust: stable toolchain（cargo 1.96+ で動作確認済み）
- 注意: `lib/bcdice/**/parser.rb` は racc 生成物で git 管理外。
  **racc 生成（`bundle exec rake racc`）を必ず最初に実行する**。
  これを忘れると `require "bcdice"` が LoadError で落ちる

### 2. upstream同期（P6で常設化）

```sh
git fetch upstream
git merge upstream/master   # TOML (test/data/*.toml) と lib/ の差分を取り込む
```

TOMLは「Rust側の仕様の真実」なので、TOML差分はそのままRustテストの期待値更新になる。

### 3. Ruby基線（正の再確認）

```sh
bundle exec rake racc
bundle exec ruby bin/baseline_runner.rb            # verify: 19864/19864 が期待値
bundle exec ruby bin/baseline_runner.rb --regen    # 本家更新後、TOMLを再生成して commit
```

- verify が全パス → Ruby本家実装とTOMLが一致している（土台が正常）
- merge 後に fail が出た場合、まず `--regen` でTOMLを本家に追従させ、
  差分を確認してから commit する（TOMLとlibの不整合をCIで止めないため）

### 4. Rustハーネス

```sh
cd rust
cargo test                # unit test + TOMLハーネス統合テスト
```

- `tests/toml_harness.rs` が `test/data/*.toml` 348ファイル・約19,864ケースを実行する
- P0時点: コア未実装のため全fail（fail理由 "core not implemented"）。これが正しい状態
- P1以降: パス数が単調に増えていく。**パス数の後退（リグレッション）をCIで検知する**
  （JSONレポート: `report_json()` → `total_cases / passed_cases / failed_cases`）

### 5. 差分ファズ（P5、将来拡張）

- Ruby側: `bin/baseline_runner.rb` を入力生成器に接続し、生成入力＋固定乱数で
  実行結果をJSONに吐く（`--dump-json` 相当を将来追加）
- Rust側: 同一入力＋同一乱数で `eval_command()` を実行し、出力テキスト・フラグを比較
- 不整合はRubyを正としてRustを修正する

## 成果物とCIゲートの対応

| ゲート | コマンド | 合格条件 |
|---|---|---|
| Ruby基線 | `bin/baseline_runner.rb` | 全パス（exit 0） |
| Rustビルド | `cargo build` | エラーなし |
| Rustハーネス | `cargo test` | パス数が前回以上（リグレッション検知） |
| TOML整合 | `--regen --dry-run` が差分なし | 本家とTOMLが一致 |

## 実装メモ（検証済みの事実）

- docker `ruby:3.2` で `bundle install` → `rake racc` → 基線ランナー動作を確認済み
  （BUNDLE_PATH をリポジトリ外 or gitignore 対象にすること。`.bundle-gems/` を追加済み）
- 本家348システムのうち `BCDice.all_game_systems.size` は 336 を返す
  （TOML 348ファイル ≠ 登録クラス数。TOML側に同一システムを複数ファイルで
  テストしているケースがあるため、Rustハーネスは「ファイル×ケース数」で集計する）
- TOMLテスト総数: 19,864ケース（randsフィールド付きは19,864、`[[test]]` ブロック数と一致）
