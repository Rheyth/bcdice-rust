# bcdice-rust

[BCDice](https://github.com/bcdice/BCDice) の Rust 実装です。本家 BCDice 3.17.0（コミット `8eced50f` 時点）を参照実装として、ダイスコマンドエンジン全体を Rust に移植しました。

## 実装状況

- **ゲームシステム 336 種**（本家 `lib/bcdice/game_system/*.rb` と 1:1）を全て移植済み
- コア（乱数 / 評価パイプライン / 加算ダイス・バラバラダイス等の共通コマンド / ダイス表 / 選択・カウント系コマンド）も移植済み
- 数値は `num_bigint::BigInt` で多倍長化しており、Ruby の `Integer` 相当の桁持ちをする（演算結果も出力も本家と一致）

### 検証

本家のテスト資産と Ruby 実装との出力突き合わせテストで、本家との一致を機械的に検証しています。

| 検証 | 内容 | 状態 |
|---|---|---|
| TOML ハーネス | 本家 `test/data/*.toml` 348 ファイル（全ケース）を実行 | 全パス |
| Ruby 出力突き合わせ | 本家を Docker 上で動かし 2,609 ケースの出力を比較 | **2,609 / 2,609 一致** |
| cargo test | ユニット + 統合テスト | 545 パス / 0 失敗 |
| clippy | `--all-targets -- -D warnings` | 警告 0 |

## 使い方

### テストの実行

```sh
cd rust
cargo test                        # 全テスト（TOML ハーネス含む）
cargo test --test toml_harness    # 本家 TOML 348 ファイルのみ
cargo clippy --all-targets -- -D warnings
```

### ライブラリとして使う

```rust
use bcdice::eval::eval_command;
use bcdice::game_system::GameSystemId;
use bcdice::randomizer::SeededRandomizer;

// Cthulhu7th で CC<=25 を評価する例
let system = GameSystemId::new("Cthulhu7th");
// 乱数は RandSource トレイトで注入する。テストでは出目列を固定できる
let mut rng = SeededRandomizer::new(vec![(1, 100)]);
let result = eval_command(&system, "CC<=25", &mut rng).unwrap();
println!("{}", result.map(|r| r.text).unwrap_or_default());
// => (1D100<=25) ボーナス・ペナルティダイス[0] ＞ 1 ＞ 1 ＞ クリティカル
```

- `eval_command` の戻り値は `Ok(None)`（本家で eval が nil を返したケース = コマンド未解釈）になりうる
- `RandSource` を実装すれば乱数を自由に注入・置換できる（オンセツール側の乱数源、決定論的テスト等）
- 登録済みシステムは `bcdice::game_system::game_systems()` / `game_system_class(id)` で列挙・取得できる

動作するサンプルは `rust/examples/api_check.rs` にあります。

```sh
cd rust && cargo run --example api_check
```

### Ruby 版との対応

Ruby 本家の構成要素はおおむね次のように対応しています。

| Ruby（本家） | Rust（本リポジトリ） |
|---|---|
| `BCDice::Base#eval` | `bcdice::eval::eval_command` |
| `BCDice::Randomizer` | `bcdice::randomizer::Randomizer`（乱数源は `RandSource` トレイトで分離） |
| `BCDice::GameSystem` | `bcdice::game_system::GameSystem` トレイト |
| `BCDice.game_system_class` | `bcdice::game_system::game_system_class` |
| `lib/bcdice/game_system/*.rb` | `rust/src/game_system/generated/`（本家メタデータから生成 + 個別移植） |
| `test/data/*.toml` | `bcdice::toml_test` ハーネスがそのまま読む |

コマンド文法の詳細は本家の [BCDiceコマンドガイド](https://docs.bcdice.org/) を参照してください（文法は本家と同一です）。

## リポジトリ構成

```
lib/           本家 Ruby 実装（参照実装・上流追従の比較基準）
test/data/     本家の TOML テスト（348 ファイル・そのまま使用）
rust/          Rust 実装
  src/         クレート本体
  tests/       TOML ハーネス・回帰テスト
  examples/    利用例
  tools/       メタデータ生成スクリプト等
docs/          移植計画・CI 計画など
reports/       Ruby 出力突き合わせテストの結果と既知差分の記録
plans/         バッチ分割の計画データ
```

## 上流（本家）との関係

本リポジトリは本家のフォークです。

- `upstream` リモートに本家 `bcdice/BCDice` を登録してあり、本家の更新は定期的に取り込む方針（`docs/rust_port_plan.md` の P6 節）
- 本家で新システムが追加・仕様変更された場合は、Rust 側も追従してテストを再パスさせる
- 本家由来の Ruby コード（`lib/`）と TOML（`test/data/`）は参照実装・テスト基準として保持している

## ライセンス

本家と同じ [BSD 3-Clause License](LICENSE)（Copyright (c) 2011, Faceless and たいたい竹流）を継承します。本リポジトリで追加・改変されたコード（`rust/` 以下）も同ライセンスで公開します。

## 関連リンク

- 本家: https://github.com/bcdice/BCDice
- コマンドガイド: https://docs.bcdice.org/
- ダイスボットの作り方（Ruby 版・概念は共通）: [docs/how_to_make_dicebot.md](docs/how_to_make_dicebot.md)
