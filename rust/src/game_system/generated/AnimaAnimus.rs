//! P4で手書き移植した `lib/bcdice/game_system/AnimaAnimus.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `AnimaAnimus#eval_game_system_specific_command`（各種表 → `xAN<=y±z`）
//! - `AnimaAnimus#check_action`
//!
//! 表データは `i18n/AnimaAnimus/ja_jp.yml` から機械的に書き出したもので、値は1文字も変えていない。
//! ロケール差のあるデータは [`SystemTables`] に束ね、
//! `AnimaAnimus_Korean`（`ko_kr`）が同じ関数群を使い回す。

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::dice_table::{RangeInc, RangeTable, RollableTable, Table};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

static JA_IGT_ITEMS: &[&str] = &[
    "ストリートファイト/<格闘>/「俺に勝てたら教えてやるよ」情報を知る魂願者から勝負を挑まれた。生き延びるためにもこの勝負、負けるわけにはいかない。",
    "追跡！/<追跡／逃走>/有益な情報を持っている人間を見つけたが、こちらの顔を見るなり逃げ出した。どうにかして捕まえなくてはならない。",
    "脅し/<威圧>/ならず者たちが集まるバーにやってきた。裏社会に生きる彼らを脅せば有益な情報が手に入るはずだ。",
    "インターネット/<コンピュータ>/SNSやニュースなど、インターネット上の情報を調査する。デマには騙されないようにしなくては。",
    "瀕死の情報提供者/<医学>/情報を知る人物がいると聞いてやってきたら、その人物が瀕死の重傷を負っていた。なんとかして蘇生させなくては。",
    "潜入捜査/<隠密>/敵対する魂願者たちのグループに潜り込んでの調査活動。リスクは高いが、有益な情報が手に入る確率は高い。",
    "情報交換/<交渉>/友好的な関係にある魂願者との情報交換。うまく話を聞き出すことができるとよいが。",
    "魔宴の情報屋/<調達>/魔宴の情報屋に接触して情報を聞き出すことにした。一筋縄ではいかない相手らしいが、はたして……？",
    "違法調査/<犯罪>/法に触れるやり方で情報を集めることにした。ハッキング、窃盗、恐喝、どんな手段を選ぼうか。",
    "聞き込み/<自我>/街ゆく人びとに聞き込みを行なう。地道な活動こそが目標にたどり着くための最短の方法だ。",
];
static JA_IGT: Table = Table::from_dice("情報収集表", 1, 10, JA_IGT_ITEMS);

static JA_LT_ITEMS: &[(RangeInc, &str)] = &[
    (RangeInc::new(1, 2), "存在/存在が希薄になり、知り合いや友人に自分の存在を忘れられてしまう。いずれ大切なパートナーの記憶からも消え、この世界でひとりぼっちになる。\nあなたの出自を消去すること。"),
    (RangeInc::new(3, 4), "記憶/自分の大切な記憶をひとつ失なう。これからは力を使うたびに記憶をひとつ失なうことになり、最後には大切なパートナーのことも思い出せなくなってしまう。\nあなたのメモリアをひとつ選択して消去すること。シナリオメモリアは選択できない。"),
    (RangeInc::new(5, 6), "容姿/だんだんと以前とはかけ離れた姿に変わっていく。いずれ誰も自分のことを自分だと気づかなくなるのだろう。\nあなたの特徴的な外見を失なう。内容をふさわしいものに書き換えること(特徴的な外見が美しい髪であれば醜い髪など)。"),
    (RangeInc::new(7, 8), "感情/喜怒哀楽の感情のうち、いずれかひとつを失なう。力を使うたびに他の感情も失っていき、最後にはただ生き残るために戦う機械となる。\nポジティブかネガティブのどちらかを選択する。選択した感情をすべてのメモリアから消去する。消去した結果、表出感情がなくなってしまった場合、残った感情を表出感情にすること。なお、新しくメモリアを取得した場合も、選んだ感情を得ることはできない。"),
    (RangeInc::new(9, 10), "五感/少しずつ五感が鈍くなる。今までできていたはずのことができなくなってしまう。\nあなたの特技をひとつ選択する。選択した特技に×をつけること。×が付いた技能で判定を行なうことはできず、判定を求められた場合は自動的に失敗となる。"),
];
static JA_LT: RangeTable = RangeTable::from_dice("喪失表", 1, 10, JA_LT_ITEMS);

/// 1ロケール分の表と定型文。
pub(crate) struct SystemTables {
    /// Ruby `TABLES["IGT"]`（情報収集表）
    pub(crate) igt: &'static Table,
    /// Ruby `TABLES["LT"]`（喪失表。`RangeTable` なので個別に持つ）
    pub(crate) lt: &'static RangeTable,
    /// i18n `AnimaAnimus.achievement_value`
    pub(crate) achievement_value: &'static str,
    /// i18n `AnimaAnimus.critical`
    pub(crate) critical: &'static str,
    /// i18n `success`
    pub(crate) success: &'static str,
    /// i18n `failure`
    pub(crate) failure: &'static str,
}

pub(crate) static JA_SYSTEM: SystemTables = SystemTables {
    igt: &JA_IGT,
    lt: &JA_LT,
    achievement_value: "達成値",
    critical: "クリティカル発生",
    success: "成功",
    failure: "失敗",
};

/// Ruby `/(\d+)AN<=(\d+([+-]\d+)*)/i`。
fn action_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(\d+)AN<=(\d+([+-]\d+)*)").expect("valid regex"))
}

/// Ruby `Base#roll_tables`（`TABLES.key?(command)` の分岐込み）。
fn roll_tables(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    match command {
        "IGT" => Ok(Some(sys.igt.roll(rng)?.to_string())),
        "LT" => Ok(Some(sys.lt.roll(rng)?.to_string())),
        _ => Ok(None),
    }
}

/// Ruby `AnimaAnimus#check_action`。
fn check_action(
    sys: &SystemTables,
    dice_cnt_expr: &str,
    target_expr: &str,
    rng: &mut Randomizer,
) -> Result<EvalResult, EvalError> {
    // Ruby: Arithmetic.eval(…, RoundType::FLOOR)。
    // 正規表現が数字と `+`/`-` しか通さないので nil にはならない。
    let dice_cnt = arithmetic::eval(dice_cnt_expr, RoundType::Floor)?
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0);
    let target = arithmetic::eval(target_expr, RoundType::Floor)?
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0);

    let dice_arr = rng.roll_barabara(dice_cnt, 10)?;
    let dice_str = dice_arr
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let suc_cnt = dice_arr.iter().filter(|&&x| x <= target).count() as i64;
    let has_critical = dice_arr.contains(&1);
    let result = if has_critical { suc_cnt + 2 } else { suc_cnt };
    let success = result > 0;

    let mut text = format!(
        "({dice_cnt}B10<={target}) ＞ {dice_str} ＞ {}({}:{result})",
        if success { sys.success } else { sys.failure },
        sys.achievement_value
    );
    if has_critical {
        text.push_str(&format!(" ({})", sys.critical));
    }

    Ok(EvalResult {
        text,
        critical: has_critical,
        success,
        failure: !success,
        ..EvalResult::default()
    })
}

/// Ruby `AnimaAnimus#eval_game_system_specific_command`。
pub(crate) fn eval_specific_command(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    // Ruby: TABLES.key?(command) を先に見る
    if let Some(text) = roll_tables(sys, command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    let Some(caps) = action_pattern().captures(command) else {
        return Ok(None);
    };
    let result = check_action(sys, &caps[1], &caps[2], rng)?;
    Ok(Some(SpecificCommandOutput::result(result)))
}

/// Ruby `BCDice::GameSystem::AnimaAnimus`（ID: `AnimaAnimus`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimaAnimus;

impl GameSystem for AnimaAnimus {
    fn id(&self) -> &'static str {
        "AnimaAnimus"
    }

    fn name(&self) -> &'static str {
        "アニマアニムス"
    }

    fn sort_key(&self) -> &'static str {
        "あにまあにむす"
    }

    fn help_message(&self) -> &'static str {
        r"・行為判定(xAN<=y±z)
　十面ダイスをx個振って判定します。達成値が算出されます(クリティカル発生時は2増加)。
　x：振るダイスの数。魂魄値や攻撃値。
　y：成功値。
　z：成功値への補正。省略可能。
　(例) 2AN<=3+1 5AN<=7
・各種表
　情報収集表　IGT/喪失表　LT
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+AN<=", "IGT", "LT"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `AnimaAnimus#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&JA_SYSTEM, command, rng)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "AnimaAnimus",
            "AnimaAnimus.toml",
            23,
        );
    }
}
