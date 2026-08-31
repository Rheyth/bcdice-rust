# frozen_string_literal: true

# BCDice（Ruby本家）をロードして、全ゲームシステムのメタデータをJSONへ抽出する。
#
# P3のコード生成方式（G1決定）の入力を作る。生成側は
# rust/tools/generate_game_systems.rb がこのJSONを読む。
#
# 実行方法は docs/rust_port_plan.md の「P3: GameSystem インフラ」節を参照
# （docker ruby:3.2 / racc の事前生成 / i18n は "~> 1.8.5" ピン）。
#
#   ruby -Ilib rust/tools/extract_game_systems.rb <出力先.json>
#
# 抽出する値は Rust 側 `trait GameSystem` のアクセサと1対1に対応させる。
# `nil` は「その値を取れなかった」ことを表し、生成側では Base の既定値を使う。

require "bcdice"
require "json"

# Ruby `BCDice::Base#initialize` の既定値。生成側で「既定値と異なるものだけ上書き」
# を判定するために、抽出時点で同じ表を持っておく。
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

# インスタンスから設定値を取り出す。
#
# `d66_sort_type` / `round_type` / `default_cmp_op` は Ruby ではシンボルなので
# 文字列にして返す（`:>=` → ">="）。
def extract_settings(instance)
  {
    "sort_add_dice" => instance.sort_add_dice?,
    "sort_barabara_dice" => instance.sort_barabara_dice?,
    "d66_sort_type" => instance.d66_sort_type.to_s,
    "enabled_d9" => instance.enabled_d9?,
    "round_type" => instance.round_type.to_s,
    "sides_implicit_d" => instance.sides_implicit_d,
    "upper_dice_reroll_threshold" => instance.upper_dice_reroll_threshold,
    "reroll_dice_reroll_threshold" => instance.reroll_dice_reroll_threshold,
    "default_cmp_op" => instance.default_cmp_op&.to_s,
    "default_target_number" => instance.default_target_number,
    # `@enabled_upcase_input` には attr_reader が無いので直接読む
    # （Elysion / GoldenSkyStories / Paranoia が false にしている）
    "enabled_upcase_input" => instance.instance_variable_get(:@enabled_upcase_input),
  }
end

out = BCDice.all_game_systems.map do |klass|
  entry = {
    # Rustの型名・ファイル名に使う。IDと一致しない場合がある
    # （ID "SwordWorld2.5" → クラス名 "SwordWorld2_5"）ので必ず実測値を使う。
    "class_name" => klass.name.split("::").last,
    "id" => klass::ID,
    "name" => klass::NAME,
    "sort_key" => klass::SORT_KEY,
    "help_message" => klass::HELP_MESSAGE,
    "prefixes" => (klass.prefixes || []),
  }

  begin
    entry.merge!(extract_settings(klass.new("")))
    entry["instantiation_error"] = nil
  rescue StandardError, ScriptError => e
    # インスタンス化できない場合は Base 既定値とみなし、その事実を記録する。
    entry.merge!(BASE_DEFAULTS)
    entry["instantiation_error"] = "#{e.class}: #{e.message}"
  end

  entry
end

out.sort_by! { |e| e["class_name"] }

File.write(ARGV[0], JSON.pretty_generate(out))

warn "extracted #{out.size} game systems"
warn "instantiation errors: #{out.count { |e| e['instantiation_error'] }}"
