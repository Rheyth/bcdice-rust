//! P4で手書き移植した `lib/bcdice/game_system/YuMyoKishi.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#roll_command`（克服判定 `YM+a>=b`）と `#sip_pat_a`（十八仔の判定）
//! - `TABLES`（代償表 `COT` / 転禍表 `TRT`）

use crate::command_parser::{Parser, SuffixPosition};
use crate::dice_table::{RollableTable, Table};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::sat_i64;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `TABLES["COT"]`（代償表）。
static COT: Table = Table::from_dice(
    "代償表",
    2,
    6,
    &[
        "不慮の出逢い",
        "深淵を覗くとき",
        "時間の消費",
        "奇妙な情報",
        "優柔不断",
        "注意散漫",
        "心身耗弱",
        "不穏な情報",
        "遺留品",
        "迫りくる危機",
        "正体の露見",
    ],
);

/// Ruby `TABLES["TRT"]`（転禍表）。
static TRT: Table = Table::from_dice(
    "転禍表",
    2,
    6,
    &[
        "○○と瓜二つ",
        "絶対絶命",
        "悪癖災う",
        "冷酷な指令",
        "おびえる視線",
        "絡みつく妖気",
        "容赦ない評定",
        "無力な市民",
        "未練阻む",
        "縁の枷",
        "邪悪な刻印",
    ],
);

/// Ruby `TABLES`。
static TABLES: &[(&str, &Table)] = &[("COT", &COT), ("TRT", &TRT)];

/// Ruby `#sip_pat_a` が返す状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SipStatus {
    /// `:yise`（一色）
    Yise,
    /// `:normal`
    Normal,
    /// `:wumian`（無面）
    Wumian,
}

/// Ruby `YuMyoKishi#sip_pat_a`（十八仔）。
///
/// Ruby は `Hash.new(0)` に数え上げるが、判定に使うのは
/// 「異なり数」「値が2のペアか」「keysの最大/合計」だけで挿入順に依存しないため、
/// 出目1〜6のカウント配列で同じ結果になる。
fn sip_pat_a(dice_list: &[i64]) -> (i64, SipStatus) {
    let mut counts = [0i64; 7];
    for &d in dice_list {
        if (1..=6).contains(&d) {
            counts[d as usize] += 1;
        }
    }
    let keys: Vec<i64> = (1..=6).filter(|&v| counts[v as usize] > 0).collect();

    match keys.len() {
        // 全てゾロ目
        1 => (20, SipStatus::Yise), // 一色
        2 => {
            if keys.iter().all(|&k| counts[k as usize] == 2) {
                // 同値のダイスが2つずつ
                (
                    keys.iter().copied().max().unwrap_or(0) * 2,
                    SipStatus::Normal,
                )
            } else {
                // 3つの同値と1つの目のダイス
                (keys.iter().sum(), SipStatus::Normal)
            }
        }
        // 2つの同値と1つずつの目のダイス
        3 => (
            keys.iter().filter(|&&k| counts[k as usize] == 1).sum(),
            SipStatus::Normal,
        ),
        // 全部バラバラ
        _ => (0, SipStatus::Wumian), // 無面
    }
}

/// Ruby `YuMyoKishi#roll_command`。
fn roll_command(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let parser =
        Parser::new(&["YM"], RoundType::Floor).restrict_cmp_op_to(&[Some(CmpOp::Ge), None]);
    let Some(cmd) = parser.parse(command) else {
        return Ok(None);
    };

    let dice_list = rng.roll_barabara(4, 6)?;
    let (value, status) = sip_pat_a(&dice_list);
    let achievement = if status == SipStatus::Wumian {
        crate::Int::ZERO
    } else {
        crate::Int::from(value) + cmd.modify_number.clone()
    };

    let mut parts = vec![
        cmd.to_s(SuffixPosition::AfterCommand),
        dice_list
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join(","),
        value.to_string(),
    ];
    if sat_i64(&achievement) != value {
        parts.push(achievement.to_string());
    }
    let roll_text = parts.join(" ＞ ");

    let result = match cmd.target_number {
        None => match status {
            SipStatus::Wumian => EvalResult::failure(roll_text),
            SipStatus::Yise => EvalResult::critical(roll_text),
            SipStatus::Normal => EvalResult::with_text(roll_text),
        },
        Some(target_number) => match status {
            SipStatus::Wumian => EvalResult::failure(format!("{roll_text} ＞ 可")),
            SipStatus::Yise => EvalResult::critical(format!("{roll_text} ＞ 優")),
            SipStatus::Normal if achievement >= target_number => {
                EvalResult::success(format!("{roll_text} ＞ 良"))
            }
            SipStatus::Normal => EvalResult::failure(format!("{roll_text} ＞ 可")),
        },
    };

    Ok(Some(result))
}

pub struct YuMyoKishi;

impl GameSystem for YuMyoKishi {
    fn id(&self) -> &'static str {
        "YuMyoKishi"
    }

    fn name(&self) -> &'static str {
        "幽冥鬼使"
    }

    fn sort_key(&self) -> &'static str {
        "ゆうみようきし"
    }

    fn help_message(&self) -> &'static str {
        r"■判定　YM+a>=b　a:技能値（省略可）　b:目標値（省略可）
　例：YM+4>=8: 技能値による修正が+4で、目標値8の克服判定を行う
　　　YM>=8  : 技能値による修正なしで、目標値8の克服判定を行う
　　　YM+6   : 技能値による修正が+6で、達成値を確認する

■代償表　COT
■転禍表　TRT
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["YM", "COT", "TRT"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(result) = roll_command(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }

        // Ruby: roll_tables(command, TABLES)
        for (key, table) in TABLES {
            if *key == command {
                return Ok(Some(SpecificCommandOutput::text(
                    table.roll(rng)?.to_string(),
                )));
            }
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "YuMyoKishi",
            "YuMyoKishi.toml",
            28,
        );
    }
}
