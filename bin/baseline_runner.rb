#!/usr/bin/env ruby
# frozen_string_literal: true

# 基線ランナー（P0）: test/data/*.toml を本家Ruby実装で実行し、
# 期待出力の検証・再生成を行う。
#
# 使い方（リポジトリルートで実行）:
#
#   # 検証モード（既定）: TOMLの期待値と実際の評価結果を比較し、結果を表示する
#   bundle exec ruby bin/baseline_runner.rb
#   bundle exec ruby bin/baseline_runner.rb --target AFF2e,CryingDreams
#
#   # 再生成モード: TOMLの output/flags を実際の評価結果で上書きする
#   # （TOML作成時のtypo検出や、本家更新時の基線更新に使う）
#   bundle exec ruby bin/baseline_runner.rb --regen
#
#   # 差分確認のみ（ファイルは書き換えない）
#   bundle exec ruby bin/baseline_runner.rb --regen --dry-run
#
# 出力は test/data のTOMLを真実とし、このランナーは本家実装（lib/bcdice）を
# 正として動く。全件パスしない場合はTOML側かlib側のどちらかに不整合がある。
#
# CI骨子（docs/rust_ci_plan.md の「Ruby基線」ステップ）から呼び出されることを想定。

require "optparse"
require "tomlrb"
require "bcdice"
require "bcdice/game_system"
require_relative "../test/randomizer_mock"

target = nil
regen = false
dry_run = false

OptionParser.new do |opts|
  opts.banner = "Usage: ruby bin/baseline_runner.rb [options]"
  opts.on("--target LIST", "Comma-separated TOML targets (e.g. AFF2e,CryingDreams)") { |v| target = v }
  opts.on("--regen", "Rewrite TOML files with actual results") { regen = true }
  opts.on("--dry-run", "With --regen: show diffs without rewriting") { dry_run = true }
end.parse!

files = if target
  targets = target.split(",").map do |t|
    t = "#{t}.toml" unless t.end_with?(".toml")
    "test/data/#{t}"
  end
  targets.reject { |path| File.exist?(path) && warn("Unknown target: #{path}") }
else
  Dir.glob("test/data/*.toml").sort
end

if files.empty?
  warn("No target found!")
  exit(1)
end

# RubyのTest::Unitと同じ正規化（test_game_system_commands.rb:54-61 と同一）
def normalize!(test_case)
  test_case[:output] = nil if test_case[:output].empty? # TOMLではnilを表現できないので空文字で代用
  test_case[:secret] ||= false
  test_case[:success] ||= false
  test_case[:failure] ||= false
  test_case[:critical] ||= false
  test_case[:fumble] ||= false
end

# eval結果をTOMLのrands形式（出現順）へ詰め替える
def rand_list(test_case)
  (test_case[:rands] || []).map { |r| [r[:value], r[:sides]] }
end

FLAG_KEYS = %i[success failure critical fumble secret].freeze

def evaluate(test_case)
  klass = BCDice.game_system_class(test_case[:game_system])
  raise "Unknown game system: #{test_case[:game_system]}" unless klass

  game_system = klass.new(test_case[:input])
  game_system.randomizer = RandomizerMock.new(rand_list(test_case))
  result = game_system.eval

  result
end

def format_flags(result)
  return "" if result.nil?

  flags = FLAG_KEYS.select { |k| result.public_send("#{k}?") }
  flags.map { |k| "#{k} = true" }.join("\n")
end

total = 0
passed = 0
failures = []

files.each do |filename|
  data = Tomlrb.load_file(filename, symbolize_keys: true)
  changed = false
  buffer = data[:test].dup

  buffer.each_with_index do |test_case, index|
    normalize!(test_case)
    total += 1

    begin
      result = evaluate(test_case)
    rescue StandardError => e
      failures << [filename, index + 1, test_case[:input], "exception: #{e.class}: #{e.message}"]
      next
    end

    if result.nil?
      if test_case[:output].nil?
        passed += 1
      else
        failures << [filename, index + 1, test_case[:input], "expected output #{test_case[:output].inspect}, but eval returned nil"]
      end
      next
    end

    diffs = []
    diffs << "output: expected #{test_case[:output].inspect}, actual #{result.text.inspect}" unless test_case[:output] == result.text
    FLAG_KEYS.each do |key|
      expected = test_case[key]
      actual = result.public_send("#{key}?")
      diffs << "#{key}: expected #{expected}, actual #{actual}" unless expected == actual
    end

    if diffs.empty?
      passed += 1
    else
      failures << [filename, index + 1, test_case[:input], diffs.join(", ")]
    end

    if regen
      new_output = result.text
      new_flags = FLAG_KEYS.index_with { |k| result.public_send("#{k}?") }
      unless test_case[:output] == new_output && FLAG_KEYS.all? { |k| test_case[k] == new_flags[k] }
        test_case[:output] = new_output
        FLAG_KEYS.each { |k| test_case[k] = new_flags[k] }
        changed = true
      end
    end
  end

  next unless regen && changed && !dry_run

  File.open(filename, "w") do |f|
    f.puts("# 生成物ではない。本家実装の挙動を記録した基線。編集は baseline_runner.rb --regen で行う")
    buffer.each do |test_case|
      f.puts("[[ test ]]")
      f.puts(%(game_system = "#{test_case[:game_system]}"))
      f.puts(%(input = #{test_case[:input].dump}))
      f.puts(%(output = #{(test_case[:output] || "").dump}))
      FLAG_KEYS.each do |k|
        f.puts("#{k} = true") if test_case[k]
      end
      if test_case[:rands] && !test_case[:rands].empty?
        f.puts("rands = [")
        test_case[:rands].each do |r|
          f.puts("  { value = #{r[:value]}, sides = #{r[:sides]} },")
        end
        f.puts("]")
      end
      f.puts
    end
  end
end

mode = regen ? (dry_run ? "regen(dry-run)" : "regen") : "verify"
puts "Baseline runner (#{mode}): #{passed}/#{total} passed, #{failures.size} failures, #{files.size} files"

failures.first(30).each do |(filename, index, input, message)|
  puts "FAIL #{filename}:#{index}: #{input.inspect}"
  puts "  #{message}"
end
puts "... and #{failures.size - 30} more" if failures.size > 30

exit(failures.empty? ? 0 : 1)
