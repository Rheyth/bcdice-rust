# frozen_string_literal: true

# rust/tools/extract_game_systems.rb が出力したJSONから、
# rust/src/game_system/generated/ 以下のRustソースを生成する。
#
#   ruby rust/tools/generate_game_systems.rb .scratch/game_systems.json [--limit N]
#
# 生成するもの:
#   - generated/<ClassName>.rs   … 1システム1ファイル
#   - generated/mod.rs の目印コメントより下 … `pub mod` の列挙と
#     `GENERATED_GAME_SYSTEMS` スライス
#
# 生成しないもの:
#   - DiceBot（手書き実装 rust/src/game_system/dice_bot.rs があるので除外）
#   - `eval_game_system_specific_command` の本体・ダイス表（P4で個別移植）
#
# Rustの構文規則に関わる注意:
#   - 文字列は `"` `\` 改行を含むなら raw string で出す。終端 `"###` と衝突しない
#     `#` の個数を内容から計算する（ヘルプ文に `"#` が現れても壊れないように）。
#   - Rustのソースは裸のCR（`\r`）を許さないので、含まれていたら生成を中断する。

require "json"
require "fileutils"

ROOT = File.expand_path("../..", __dir__)
GENERATED_DIR = File.join(ROOT, "rust/src/game_system/generated")
MOD_RS = File.join(GENERATED_DIR, "mod.rs")
MARKER = "// ============================================================================\n" \
         "// ここから下は rust/tools/generate_game_systems.rb が生成する。手で編集しない。\n" \
         "// ============================================================================\n"

# 手書き実装があるので生成対象から外すシステム。
HANDWRITTEN_IDS = ["DiceBot"].freeze

# Ruby `Base#initialize` の既定値。これと同じ設定は出力しない。
BASE_DEFAULTS = {
  "sort_add_dice" => false,
  "sort_barabara_dice" => false,
  "d66_sort_type" => "no_sort",
  "enabled_d9" => false,
  "round_type" => "floor",
  "sides_implicit_d" => 6,
  "upper_dice_reroll_threshold" => nil,
  "reroll_dice_reroll_threshold" => nil,
  "default_cmp_op" => nil,
  "default_target_number" => nil,
  "enabled_upcase_input" => true,
}.freeze

# `__generated_system_settings!` の省略可能グループは順序どおりにしか照合できない。
SETTING_ORDER = BASE_DEFAULTS.keys.freeze

D66_SORT_TYPE = {
  "no_sort" => "$crate::enums::D66SortType::NoSort",
  "asc" => "$crate::enums::D66SortType::Asc",
  "desc" => "$crate::enums::D66SortType::Desc",
}.freeze

ROUND_TYPE = {
  "floor" => "$crate::enums::RoundType::Floor",
  "ceil" => "$crate::enums::RoundType::Ceil",
  "round" => "$crate::enums::RoundType::Round",
}.freeze

CMP_OP = {
  ">" => "$crate::normalize::CmpOp::Gt",
  ">=" => "$crate::normalize::CmpOp::Ge",
  "<" => "$crate::normalize::CmpOp::Lt",
  "<=" => "$crate::normalize::CmpOp::Le",
  "==" => "$crate::normalize::CmpOp::Eq",
  "!=" => "$crate::normalize::CmpOp::Ne",
}.freeze

# マクロ本体では `$crate` だが、生成ファイル側の呼び出しでは `crate` を使う。
def macro_path(spec)
  spec.sub("$crate", "crate")
end

# Rustの文字列リテラルを組み立てる。
#
# `"` `\` 改行のいずれも含まなければ通常の `"..."`、含むなら raw string。
# raw string の `#` の個数は「内容中の `"` に続く `#` の最大連続数 + 1」にする
# （そうすれば終端記号 `"###...` が内容中に現れない）。
def rust_string(str)
  # Rustのソースは裸のCRを許さない。タブと改行以外の制御文字も想定外なので弾く。
  bad = str.each_char.find { |c| c.ord < 0x20 && c != "\n" && c != "\t" }
  raise ArgumentError, "control char #{bad.inspect} in #{str[0, 40].inspect}" if bad

  # エスケープが要らないなら通常の文字列リテラル（日本語はそのまま出す）
  return "\"#{str}\"" if !str.include?('"') && !str.include?("\\") && !str.include?("\n")

  hashes = str.scan(/"(#*)/).map { |m| m[0].length }.max
  n = hashes.nil? ? 0 : hashes + 1
  marks = "#" * n
  "r#{marks}\"#{str}\"#{marks}"
end

# 既定値と異なる設定だけを `settings: { ... }` の中身として並べる。
def settings_body(entry)
  lines = SETTING_ORDER.filter_map do |key|
    value = entry[key]
    next if value == BASE_DEFAULTS[key]

    rendered =
      case key
      when "d66_sort_type"
        macro_path(D66_SORT_TYPE.fetch(value))
      when "round_type"
        macro_path(ROUND_TYPE.fetch(value))
      when "default_cmp_op"
        macro_path(CMP_OP.fetch(value))
      when "sort_add_dice", "sort_barabara_dice", "enabled_d9", "enabled_upcase_input"
        value ? "true" : "false"
      else
        value.to_s
      end

    "        #{key}: #{rendered},"
  end

  lines
end

def prefixes_literal(prefixes)
  # Ruby の `register_prefix_from_super_class` は、親が接頭辞を持たないと
  # nil を1件登録してしまう（Arianrhod:Korean）。Ruby側の
  # `@prefixes.join('|')` は nil を空文字列にするので、それに合わせる。
  values = prefixes.map { |p| rust_string(p.to_s) }
  return "[]" if values.empty?

  "[\n#{values.map { |v| "        #{v}," }.join("\n")}\n    ]"
end

def render_system(entry)
  class_name = entry.fetch("class_name")
  settings = settings_body(entry)
  settings_block =
    if settings.empty?
      "settings: {},"
    else
      "settings: {\n#{settings.join("\n")}\n    },"
    end

  <<~RUST
    //! 自動生成: `lib/bcdice/game_system/#{class_name}.rb` のメタデータから生成した。
    //!
    //! 手で編集しないこと（`rust/tools/generate_game_systems.rb` が再生成する）。
    //! 固有コマンドの中身は P4 で個別移植する。

    crate::impl_generated_system! {
        #{class_name},
        id: #{rust_string(entry.fetch('id'))},
        name: #{rust_string(entry.fetch('name'))},
        sort_key: #{rust_string(entry.fetch('sort_key'))},
        help_message: #{rust_string(entry.fetch('help_message'))},
        prefixes: #{prefixes_literal(entry.fetch('prefixes'))},
        #{settings_block}
    }
  RUST
end

def render_mod_tail(entries)
  mods = entries.map { |e| "pub mod #{e.fetch('class_name')};" }.join("\n")
  items = entries.map { |e| "    &#{e.fetch('class_name')}::#{e.fetch('class_name')}," }.join("\n")

  <<~RUST
    #{mods}

    /// 生成された全ゲームシステム（クラス名の昇順、#{entries.size}件）。
    ///
    /// 手書き実装（`DiceBot` / `DummySystem`）は含まない。
    /// レジストリ（`crate::game_system::registry`）がこのスライスと手書き分を連結する。
    pub static GENERATED_GAME_SYSTEMS: &[&'static dyn crate::game_system::GameSystem] = &[
    #{items}
    ];
  RUST
end

json_path = ARGV[0] or abort("usage: generate_game_systems.rb <metadata.json> [--limit N]")
limit = (ARGV[ARGV.index("--limit") + 1].to_i if ARGV.include?("--limit"))

entries = JSON.parse(File.read(json_path))
entries.reject! { |e| HANDWRITTEN_IDS.include?(e["id"]) }
entries.sort_by! { |e| e.fetch("class_name") }
entries = entries.first(limit) if limit

FileUtils.mkdir_p(GENERATED_DIR)

# 前回の生成物のうち、今回の一覧に無いものを消す（システムが減った場合に残らないように）
keep = entries.map { |e| "#{e.fetch('class_name')}.rs" } << "mod.rs"
Dir.glob(File.join(GENERATED_DIR, "*.rs")).each do |path|
  File.delete(path) unless keep.include?(File.basename(path))
end

entries.each do |entry|
  File.write(File.join(GENERATED_DIR, "#{entry.fetch('class_name')}.rs"), render_system(entry))
end

head = File.read(MOD_RS).split(MARKER).first
abort("marker not found in #{MOD_RS}") if head.nil? || head == File.read(MOD_RS)
File.write(MOD_RS, head + MARKER + "\n" + render_mod_tail(entries))

warn "generated #{entries.size} game systems into #{GENERATED_DIR}"
