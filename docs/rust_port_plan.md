# bcdice-rust 移植計画（確定仕様 / 2026-08-30 G0ゲート通過済み）

## 目標
本家 BCDice 3.17.0（Ruby）と**全く同一挙動**の Rust 版を完成させる。

## G0で確定した仕様
| 項目 | 決定 |
|---|---|
| スコープ | **全348ゲームシステムを完遂してから完成**（部分完成は「完成」と呼ばない） |
| 検証基準 | ① TOMLテスト（test/data/ 348ファイル）全パス ② Ruby本家との差分ファズテスト（乱数固定・出力一致） |
| 形態 | 「BCDiceのRust版」のライブラリ実装に限定。APIサイドカー等は本プロジェクトのスコープ外 |
| 上流追従 | 移植中もTOMLテストは随時upstreamから取り込み、Rust側の仕様の真実とする |
| 参照実装 | 本リポジトリのRubyコードそのもの（同梱＝常に手元で正解を取れる） |

## 規模の実測（2026-08-30時点）
- `lib/bcdice` 総計 約88,600行Ruby
  - コア（command parser/common_command/arithmetic/randomizer/table/result/format）約3,000行
  - `game_system/` 5.1MB / 348システム。うち **240個が独自 `eval_game_system`** を持つ（個別移植必須）。残りはコマンド定義＋テーブル中心
- `test/data/` TOML 348ファイル（入力・期待出力・乱数固定 `rands` 付き）＝そのまま合否判定に使える

## 完了条件（Definition of Done）
1. `test/data/` 全TOMLテストをRustハーネスで実行し全パス（upstream同期を含む）
2. 差分ファズテスト: 同一シード乱数でRuby版とRust版に同一入力を与え、出力テキスト・success/critical/failureフラグが一致（システム×コマンドパターンを網羅する生成器）
3. 全348システムがロード・評価可能（`game_system_class` 相当の列挙API）
4. 上記をCIで自動化（Rubyリファレンス実行＋Rustテスト＋upstream差分チェック）

## フェーズ構成
### P0: 検証基盤（最優先・これなくして合否不明）
- Rust側: `SeededRandomizer`（TOMLの `rands` を注入）+ TOMLテストランナーハーネス
- Ruby側リファレンスランナー: 同TOMLをRubyで実行し期待出力を再生成できる基線
- CI骨子: upstream fetch → TOML同期 → Ruby基線 → Rust実行 → diff

### P1: コア移植（約3,000行→Rust推定8〜10千行）
- `parser.y`（Racc文法）→ lalrpop 等へ移植。**文法の優先順位・エラー復旧を原典どおりに**
- 算術評価器: **RubyのRational有理数演算を正確に再現**（`num-rational` + 除算/丸め/表示フォーマットの一致検証。`3/2U` 等の分母丸め挙動が山場）
- `Randomizer` / `Result` / フォーマット（全角記号、`＞` 区切り、ソートキー）
- D66/大 small→left 等、ダイス種別の細部仕様

### P2: common_command 移植（10種）
add_dice / barabara / calc / choice / d66 / repeat / reroll / tally / upper / version（node定義含め約2,000行）
- Ruby正規表現 → Rust `regex`/`fancy-regex` の差分（lookbehind等）に注意

### P3: GameSystem インフラ
- `trait GameSystem`: ID / NAME / SORT_KEY / HELP_MESSAGE / コマンド接頭辞 / eval
- HELP_MESSAGE・テーブルは**データ化して機械変換**（Ruby→Rust コード生成 or ビルド時取り込み）
- ゲームシステムの登録・列挙機構

#### G1で確定した生成方式
docker の `ruby:3.2` で BCDice を実際にロードし、定数・設定フラグ・メソッドのソースを
JSONへ抽出 → それを入力に **Rustソースを生成してコミット** する。
実行時ロード・ダイナミックディスパッチは行わず、ビルド時に全システムを確定させる。

#### 実装済み（P3-Batch1）
| 対象 | 場所 |
|---|---|
| `trait GameSystem`（設定アクセサ＋フック、既定実装は Ruby `Base` と1対1） | `rust/src/game_system/mod.rs` |
| レジストリ（`game_system_class` / `all_game_systems`） | `rust/src/game_system/registry.rs` |
| `DiceBot` | `rust/src/game_system/dice_bot.rs` |
| ダミーシステム（インフラ検証用。全システム登録時に削除） | `rust/src/game_system/dummy_system.rs` |
| DiceTable 基盤11種 | `rust/src/dice_table/` |

- 共通コマンド（`common_command/`）・`preprocessor`・`eval` はすべて `&dyn GameSystem` を受ける。
  `Base#check_result` が `result_1d100` などのフックを動的ディスパッチするため、
  「設定は struct、フックだけ trait」の二層には**できない**（共通コマンドも trait を受ける必要がある）。
- `GameSystemConfig` は同じ設定値を実行時に組み替えられる struct 版で、それ自身が
  `GameSystem` を実装する。既定値以外の設定を通る分岐の検証（`rust/tests/config_variants.rs`）専用。

#### 後続バッチの生成物ディレクトリ構造
```
rust/src/game_system/
  mod.rs            … trait / GameSystemConfig / build_prefixes_pattern（手書き・変更頻度低）
  registry.rs       … register_game_systems![...] にシステムを列挙（生成対象）
  dice_bot.rs       … 手書き
  generated/
    mod.rs          … pub mod <Id>; を並べる（生成対象）
    <Id>.rs         … 1システム1ファイル（生成対象）。例: Cthulhu7th.rs
```
- ファイル名・型名は Ruby のクラス名（`BCDice::GameSystem::<Name>`）をそのまま使う。
  ID に `:` `.` を含むシステム（`SwordWorld2.5` など）は Ruby 側の
  `id.tr(":.", "_")` と同じ規則でクラス名へ変換する（`BCDice.dynamic_load` 参照）。
- 生成コードは「既定値と異なるアクセサだけを上書きする」。既定値は trait 側にあるので、
  差分だけ書けば `Base` の挙動になる。
- **`RangeTable` は生成した表を全件 `validate()` に通すテストを置くこと**。
  Ruby は構築時に `RangeError` を投げる＝ロード時に必ず検出されるが、Rust側は
  `fetch` が該当なしを `None`（→空文字列）に畳むので、範囲の隙間・重なりが
  静かに通ってしまう。生成物ディレクトリ全体を回す1本のテストで代替する。
- **`SaiFicSkillTable::prefixes()` は `Vec<&'static str>` を返す**（Ruby の
  `(["RTT[1-6]?", "RCT", @rtt, @rct] + @rttn).compact` をそのまま移したもの）が、
  `GameSystem::prefixes()` は `&'static [&'static str]` を要求する。
  rtt / rct / rttn はコンパイル時に既知なので、**生成側が `prefixes()` 用の
  スライスをリテラルで書き出す**（`SaiFicSkillTable::prefixes()` は
  ヘルプ表示や照合用に残す）方針で繋ぐこと。
- **接頭辞パターンのキャッシュ**: `prefixes` を持つシステムには
  `crate::impl_prefixes_pattern!();` を1行入れる。`static OnceLock<Regex>` を
  trait の既定実装本体に置くと全システムで1つの `OnceLock` を共有してしまうため、
  必ずこのマクロ（＝各 impl 側）で定義すること。回帰テストは
  `rust/tests/game_system_dispatch.rs::prefixes_pattern_is_cached_per_system`。

#### 入力メタデータ（336システム分）の取得
- 先行タスクの成果物:
  `<bcdice-rust>/.worktrees/t_eab4e881/.scratch/game_systems.json`
  （`.scratch/` は untracked かつ root 所有。リポジトリには入れないので、
  worktree を消したら下記手順で作り直す）
- 抽出スクリプト（`.scratch/extract_meta.rb` として置く。これも untracked）:
  ```ruby
  # frozen_string_literal: true
  require "bcdice"
  require "json"

  out = BCDice.all_game_systems.map do |klass|
    {
      "id" => klass::ID,
      "name" => klass::NAME,
      "sort_key" => klass::SORT_KEY,
      "help_message" => klass::HELP_MESSAGE,
      "prefixes" => (klass.prefixes || []),
      "round_type" => (klass.new("").round_type rescue nil),
      "d66_sort_type" => (klass.new("").d66_sort_type.to_s rescue nil),
      "sort_add_dice" => (klass.new("").sort_add_dice? rescue nil),
      "sort_barabara_dice" => (klass.new("").sort_barabara_dice? rescue nil),
      "sides_implicit_d" => (klass.new("").sides_implicit_d rescue nil),
    }
  end
  File.write(ARGV[0], JSON.generate(out))
  puts out.size
  ```
- 再生成手順（docker `ruby:3.2`、リポジトリルートで実行。2026-08-30 実行確認済み）:
  ```sh
  docker run --rm -v "$PWD:/w" -w /w -u "$(id -u):$(id -g)" \
    -e HOME=/tmp -e GEM_HOME=/tmp/gems \
    -e PATH=/tmp/gems/bin:/usr/local/bin:/usr/bin:/bin \
    ruby:3.2 sh -c 'gem install racc --no-document >/dev/null &&
      gem install i18n -v "~> 1.8.5" --no-document >/dev/null &&
      for y in $(find lib -name "*.y"); do racc "$y" -o "${y%.y}.rb" --no-line-convert; done &&
      ruby -Ilib .scratch/extract_meta.rb .scratch/game_systems.json'
  # => 336
  ```
  ハマりどころ（いずれも上の手順で回避済み）:
  - **Racc生成物が必要**: `lib/bcdice/**/parser.rb` は `.y` からの生成物で
    `.gitignore` 済み。先に `racc` を回さないと `require "bcdice"` が LoadError になる。
    `rake racc` は Rakefile が rubocop を require するため素の `ruby:3.2` では失敗する
    ので、`racc` を直接叩く（オプションは Rakefile の rule と同じ `--no-line-convert`）。
  - **i18n のバージョン**: gemspec の `~> 1.8.5` を守ること。新しい i18n は
    YAML由来の配列を freeze するため、`BeginningIdol` のロードで `FrozenError` になる。
  - `-u`/`GEM_HOME` を指定しないと生成物が root 所有になる。
- 実測値（2026-08-30）:
  - `BCDice.all_game_systems.size` = **336**（`lib/bcdice/game_system/*.rb` も336）。
    本ドキュメント上部の「348システム」は `test/data/*.toml` の**ファイル数**であり、
    システム数ではない。P4のスコープ計算では336を使うこと。
  - `prefixes` を持つ = 311 / 持たない = 25
  - `round_type`: floor 315 / ceil 21
  - `d66_sort_type`: no_sort 288 / asc 48
  - `sides_implicit_d`: 6 が326、10 が8、100 と 12 が各1
- このメタデータでカバーできるのは定数と設定フラグまで。
  `eval_game_system_specific_command` の本体とダイス表の中身は
  Ruby ソースからの変換が別途必要（P4）。

### P4: 348システム移植（作業量の9割）
- テーブル中心の108システム: データ変換で自動化率高
- 独自evalの240システム: 個別移植。TOMLテスト充実度で優先度順に消化
- 1システム完了ごとに: TOML全パス＋該当システムの差分テスト

### P5: 差分ファズテスト運用
- システム×コマンドパターン生成器（正規文法から入力生成）
- Ruby/Rust同時実行・出力diff。不整合はRubyコードを正としてRust修正

### P6: 上流追従ルーチン（常設）
- 定期的に `git fetch upstream` → TOML/`lib` 差分をRust側へ反映
- 本家の新規システム・仕様変更もRustに移植して追従（完成後も継続）

## 主要リスク
1. **Rational/フォーマット細部差** — 表示1文字の違いでテスト全滅し得る。P1で徹底検証
2. **Ruby正規表現の互換** — Oniguruma由来の構文がRustクレートで表現できない場合、手書きパーサへの置換が必要
3. **ボリューム** — 240システムの個別ロジック。kanban分割で並行消化する
4. **テスト未網羅領域** — TOMLに無い挙動の検出は差分ファズのみが頼り。P5をP4と並行運用
5. **上流が動く標的** — 完成時点の本家HEADに追従し直してから「完成」とする

## 移植中の運用方針
- 作業はkanbanワークフローに委譲（P1〜P4をカード化、検証は survival-pj-verifier 的な決定論的検証カードと分離）
- 個人検証資産はgit管理対象外。リポジトリに追加するのはコード・テスト・本計画程度に留める
