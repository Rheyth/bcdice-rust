//! 既定値以外の [`GameSystemConfig`] を通る分岐の検証。
//!
//! TOMLテストで実行できるのは "DiceBot"（`Base` の既定設定そのまま）だけなので、
//! `enabled_d9` / `sort_add_dice` / `d66_sort_type` / `round_type` /
//! `default_cmp_op` などの分岐は327ケースを1つも通らない。
//! P4で348システムがこれらに乗るため、ここで先に固定しておく。

use bcdice::enums::{D66SortType, RoundType};
use bcdice::eval::{eval_raw, EvalResult};
use bcdice::game_system::GameSystemConfig;
use bcdice::normalize::CmpOp;
use bcdice::randomizer::{Randomizer, SeededRandomizer};

/// 設定と注入乱数を指定して評価し、乱数を使い切ったことも確かめる。
fn eval(config: &GameSystemConfig, input: &str, rands: &[(i64, i64)]) -> Option<EvalResult> {
    let mut src = SeededRandomizer::new(rands.to_vec());
    let result = {
        let mut rng = Randomizer::new(&mut src);
        eval_raw(config, input, &mut rng).expect("no eval error")
    };
    assert!(src.is_empty(), "unconsumed rands remain for {input:?}");
    result
}

fn text(config: &GameSystemConfig, input: &str, rands: &[(i64, i64)]) -> String {
    eval(config, input, rands)
        .expect("command must be recognized")
        .text
}

#[test]
fn enabled_d9_uses_roll_d9() {
    // AddDice::Randomizer#roll の `sides == 9 && enabled_d9?` 分岐。
    // roll_d9 は d10 を振って 1 を引く（面数10で記録される）。
    let config = GameSystemConfig {
        enabled_d9: true,
        ..GameSystemConfig::default()
    };
    assert_eq!(
        text(&config, "2D9", &[(10, 10), (1, 10)]),
        "(2D9) ＞ 9[9,0] ＞ 9"
    );

    // 無効なら通常のバラバラロール（面数9で記録される）
    let config = GameSystemConfig::default();
    assert_eq!(
        text(&config, "2D9", &[(9, 9), (1, 9)]),
        "(2D9) ＞ 10[9,1] ＞ 10"
    );
}

#[test]
fn sort_add_dice_sorts_dice_list() {
    let config = GameSystemConfig {
        sort_add_dice: true,
        ..GameSystemConfig::default()
    };
    assert_eq!(
        text(&config, "3D6", &[(5, 6), (1, 6), (3, 6)]),
        "(3D6) ＞ 9[1,3,5] ＞ 9"
    );
}

#[test]
fn d66_sort_type_applies_to_add_dice() {
    // AddDice::Randomizer#roll の `sides == 66` 分岐は
    // ゲームシステムの d66_sort_type を使う（D66Diceコマンドとは別経路）。
    let rands = [(6, 6), (1, 6), (2, 6), (5, 6)];

    let config = GameSystemConfig::default(); // NO_SORT
    assert_eq!(text(&config, "2D66", &rands), "(2D66) ＞ 86[61,25] ＞ 86");

    let config = GameSystemConfig {
        d66_sort_type: D66SortType::Asc,
        ..GameSystemConfig::default()
    };
    assert_eq!(text(&config, "2D66", &rands), "(2D66) ＞ 41[16,25] ＞ 41");

    let config = GameSystemConfig {
        d66_sort_type: D66SortType::Desc,
        ..GameSystemConfig::default()
    };
    assert_eq!(text(&config, "2D66", &rands), "(2D66) ＞ 113[61,52] ＞ 113");
}

#[test]
fn round_type_drives_default_division() {
    // 端数処理記号なしの除算は round_type で分岐する
    let rands = [(3, 6)];

    let config = GameSystemConfig::default(); // FLOOR
    assert_eq!(text(&config, "1D6/2", &rands), "(1D6/2) ＞ 3[3]/2 ＞ 1");

    let config = GameSystemConfig {
        round_type: RoundType::Ceil,
        ..GameSystemConfig::default()
    };
    assert_eq!(text(&config, "1D6/2", &rands), "(1D6/2) ＞ 3[3]/2 ＞ 2");

    let config = GameSystemConfig {
        round_type: RoundType::Round,
        ..GameSystemConfig::default()
    };
    assert_eq!(text(&config, "1D6/4", &rands), "(1D6/4) ＞ 3[3]/4 ＞ 1");
    assert_eq!(text(&config, "1D6/8", &rands), "(1D6/8) ＞ 3[3]/8 ＞ 0");
}

#[test]
fn sides_implicit_d_is_configurable() {
    let config = GameSystemConfig {
        sides_implicit_d: 10,
        ..GameSystemConfig::default()
    };
    assert_eq!(
        text(&config, "2D", &[(7, 10), (3, 10)]),
        "(2D10) ＞ 10[7,3] ＞ 10"
    );
}

#[test]
fn default_cmp_op_and_target_number_fill_barabara_dice() {
    let config = GameSystemConfig {
        default_cmp_op: Some(CmpOp::Ge),
        default_target_number: Some(4),
        ..GameSystemConfig::default()
    };
    assert_eq!(
        text(&config, "2B6", &[(5, 6), (3, 6)]),
        "(2B6>=4) ＞ 5,3 ＞ 成功数1"
    );
    // 明示指定があればそちらが優先される
    assert_eq!(
        text(&config, "2B6<=3", &[(5, 6), (3, 6)]),
        "(2B6<=3) ＞ 5,3 ＞ 成功数1"
    );
}

#[test]
fn default_target_number_zero_is_not_replaced() {
    // Rubyの `x&.eval || default` は 0 が truthy なので、0 は既定値で潰されない
    let config = GameSystemConfig {
        default_target_number: Some(99),
        ..GameSystemConfig::default()
    };
    assert_eq!(
        text(&config, "2B6>=0", &[(5, 6), (3, 6)]),
        "(2B6>=0) ＞ 5,3 ＞ 成功数2"
    );
}

#[test]
fn sort_barabara_dice_sorts_each_group() {
    let config = GameSystemConfig {
        sort_barabara_dice: true,
        ..GameSystemConfig::default()
    };
    assert_eq!(
        text(&config, "3B6", &[(5, 6), (1, 6), (3, 6)]),
        "(3B6) ＞ 1,3,5"
    );
}

#[test]
fn reroll_dice_reroll_threshold_default() {
    let config = GameSystemConfig {
        reroll_dice_reroll_threshold: Some(5),
        ..GameSystemConfig::default()
    };
    // 閾値未指定でも既定値が入るので「条件が間違っています」にならない
    assert_eq!(
        text(&config, "2R6", &[(5, 6), (2, 6), (3, 6)]),
        "(2R6[>=5]) ＞ 5,2 + 3 ＞ 成功数0"
    );
}

#[test]
fn upper_dice_reroll_threshold_default() {
    let config = GameSystemConfig {
        upper_dice_reroll_threshold: Some(5),
        ..GameSystemConfig::default()
    };
    assert_eq!(
        text(&config, "1U6", &[(5, 6), (2, 6)]),
        "(1U6[5]) ＞ 7[5,2] ＞ 7/7(最大/合計)"
    );

    // 既定値も無ければ 0 になり、条件エラーになる
    let config = GameSystemConfig::default();
    assert_eq!(
        text(&config, "1U6", &[]),
        "(1U6[0]) ＞ 無限ロールの条件がまちがっています"
    );
}

#[test]
fn tally_dice_sort_barabara_dice_affects_listing() {
    let config = GameSystemConfig {
        sort_barabara_dice: true,
        ..GameSystemConfig::default()
    };
    assert_eq!(
        text(&config, "3TY6", &[(5, 6), (1, 6), (5, 6)]),
        "(3TY6) ＞ 1,5,5 ＞ [1]×1, [5]×2"
    );
}
