# 差分ファズ レポート（fuzz_diff 生成物）

本ファイルは `fuzz_diff --report` の生成物。手で編集しない。
恒久的な既知不一致の記録は `reports/fuzz_known_diffs.md` 側に書く。

- 総ケース数: 2609
- 一致: 2609
- 不一致: 0

## 不一致フィールド別

なし（全件一致）

## 種別（kind）別

| kind | 一致 | 総数 |
|---|---:|---:|
| `add_dice` | 864 | 864 |
| `add_dice_divide` | 40 | 40 |
| `add_dice_expr` | 15 | 15 |
| `add_dice_expr_secret` | 15 | 15 |
| `add_dice_filter` | 92 | 92 |
| `add_dice_secret` | 144 | 144 |
| `barabara` | 108 | 108 |
| `barabara_multi` | 11 | 11 |
| `barabara_multi_secret` | 11 | 11 |
| `bignum` | 23 | 23 |
| `calc` | 45 | 45 |
| `calc_edge` | 7 | 7 |
| `calc_edge_secret` | 9 | 9 |
| `choice` | 45 | 45 |
| `choice_secret` | 45 | 45 |
| `cmp_op` | 61 | 61 |
| `cmp_op_question` | 6 | 6 |
| `d66` | 9 | 9 |
| `d66_secret` | 9 | 9 |
| `d66_sort` | 7 | 7 |
| `d66_sort_secret` | 7 | 7 |
| `degenerate` | 50 | 50 |
| `implicit_d` | 21 | 21 |
| `implicit_d_secret` | 21 | 21 |
| `infinite_roll_guard` | 9 | 9 |
| `limits` | 23 | 23 |
| `preprocess` | 22 | 22 |
| `rand_limit` | 4 | 4 |
| `repeat` | 18 | 18 |
| `repeat_edge` | 20 | 20 |
| `repeat_edge_secret` | 21 | 21 |
| `reroll` | 300 | 300 |
| `reroll_edge` | 13 | 13 |
| `reroll_edge_secret` | 13 | 13 |
| `tally` | 18 | 18 |
| `tally_expr` | 8 | 8 |
| `tally_expr_secret` | 8 | 8 |
| `upper` | 432 | 432 |
| `upper_edge` | 11 | 11 |
| `upper_edge_secret` | 12 | 12 |
| `version` | 6 | 6 |
| `version_secret` | 6 | 6 |

