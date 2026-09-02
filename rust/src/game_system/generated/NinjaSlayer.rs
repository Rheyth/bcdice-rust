//! `lib/bcdice/game_system/NinjaSlayer.rb` の手書き移植。

use std::borrow::Cow;
use std::sync::OnceLock;

use regex::Regex;

use crate::dice_table::{RollableTable, Table};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NinjaSlayer;

impl GameSystem for NinjaSlayer {
    fn id(&self) -> &'static str {
        "NinjaSlayer"
    }
    fn name(&self) -> &'static str {
        "ニンジャスレイヤーTRPG"
    }
    fn sort_key(&self) -> &'static str {
        "にんしやすれいやあTRPG"
    }
    fn help_message(&self) -> &'static str {
        r"・通常判定　NJ
　NJx[y] or NJx@y or NJx
　x=判定ダイス y=難易度 省略時はNORMAL(4)
　例:NJ4@H 難易度HARD、判定ダイス4で判定
・回避判定　EV
　EVx[y]/z or EVx@y/z or EVx/z or EVx[y] or EVx@y or EVx
　x=判定ダイス y=難易度 z=攻撃側の成功数(省略可) 難易度を省略時はNORMAL(4)
　攻撃側の成功数を指定した場合、カウンターカラテ発生時には表示
　例:EV5/3 難易度NORMAL(省略時)、判定ダイス5、攻撃側の成功数3で判定
・近接攻撃　AT
　ATx[y] or ATx@y or ATx
　x=判定ダイス y=難易度 省略時はNORMAL(4) サツバツ！発生時には表示
　例:AT6[H] 難易度HARD,判定ダイス5で近接攻撃の判定
・サツバツ判定　SB
・電子戦　EL
　ELx[y] or ELx@y or ELx
　x=判定ダイス y=難易度 省略時はNORMAL(4)
　例:EL6[H] 難易度HARD,判定ダイス5で電子戦の判定

・難易度
　KIDS=K,EASY=E,NORMAL=N,HARD=H,ULTRA HARD=UH 数字にも対応

※上記コマンド群は『ニンジャスレイヤーTRPG コア・ルールブック』に対応していません。コア・ルールブックで遊ぶ場合には『ニンジャスレイヤーTRPG 2版』のコマンドを利用してください。
"
    }
    fn prefixes(&self) -> &'static [&'static str] {
        &["NJ", "EV", "AT", "EL", "SB"]
    }
    crate::impl_prefixes_pattern!();
    fn default_cmp_op(&self) -> Option<CmpOp> {
        Some(CmpOp::Ge)
    }
    fn default_target_number(&self) -> Option<i64> {
        Some(4)
    }

    fn change_text<'a>(&self, text: &'a str) -> Cow<'a, str> {
        static RE: OnceLock<Regex> = OnceLock::new();
        let re = RE.get_or_init(|| {
            Regex::new(r"(?i)\A(S)?NJ(\d+)(?:\[(UH|[2-6KENH])\]|@(UH|[2-6KENH]))?\z").unwrap()
        });
        match re.captures(text) {
            Some(c) => Cow::Owned(format!(
                "{}{}B6>={}",
                c.get(1).map_or("", |m| m.as_str()),
                &c[2],
                difficulty(c.get(3).or_else(|| c.get(4)).map(|m| m.as_str()))
            )),
            None => Cow::Borrowed(text),
        }
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if command == "SB" {
            return Ok(Some(SpecificCommandOutput::text(SB.roll(rng)?.to_string())));
        }
        static RE: OnceLock<Regex> = OnceLock::new();
        let re = RE.get_or_init(|| {
            Regex::new(r"(?i)\A(EV|AT|EL)(\d+)(?:\[(UH|[2-6KENH])\]|@(UH|[2-6KENH]))?(?:/(\d+))?\z")
                .unwrap()
        });
        let Some(c) = re.captures(command) else {
            return Ok(None);
        };
        let kind = &c[1];
        if kind != "EV" && c.get(5).is_some() {
            return Ok(None);
        }
        let num = to_i(&c[2]);
        let difficulty = difficulty(c.get(3).or_else(|| c.get(4)).map(|m| m.as_str()));
        let dice = rng.roll_barabara(num, 6)?;
        let successes = dice.iter().filter(|d| **d >= difficulty).count() as i64;
        let maxes = dice.iter().filter(|d| **d == 6).count() as i64;
        let mut text = format!(
            "({num}B6>={difficulty}) ＞ {} ＞ 成功数{successes}",
            dice.iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(",")
        );
        match kind {
            "EV" if c.get(5).is_some_and(|m| successes > to_i(m.as_str())) => {
                text.push_str(" ＞ カウンターカラテ!!")
            }
            "AT" if maxes >= 2 => text.push_str(" ＞ サツバツ!!"),
            "EL" if maxes >= 1 => text.push_str(&format!(" + {maxes} ＞ {}", successes + maxes)),
            _ => {}
        }
        Ok(Some(SpecificCommandOutput::text(text)))
    }
}

fn difficulty(value: Option<&str>) -> i64 {
    match value.map(str::to_ascii_uppercase).as_deref() {
        None | Some("N") => 4,
        Some("K") => 2,
        Some("E") => 3,
        Some("H") => 5,
        Some("UH") => 6,
        Some(n) => to_i(n),
    }
}
fn to_i(value: &str) -> i64 {
    value.parse().unwrap_or(i64::MAX)
}

static SB: Table = Table::from_dice("サツバツ表", 1, 6, &[
    "「死ねーッ！」腹部に強烈な一撃！　敵はくの字に折れ曲がり、ワイヤーアクションめいて吹っ飛んだ！：本来のダメージ+1ダメージを与える。敵は後方の壁または障害物に向かって、何マスでもまっすぐ弾き飛ばされる（他のキャラのいるマスは通過する）。壁または障害物に接触した時点で、敵はさらに1ダメージを受ける。敵はこの激突ダメージに対して改めて『回避判定』を行っても良い。",
    "「イヤーッ！」頭部への痛烈なカラテ！　眼球破壊もしくは激しい脳震盪が敵を襲う！：本来のダメージを与える。さらに敵の【ニューロン】と【ワザマエ】がそれぞれ1ずつ減少する（これによる最低値は1）。残虐ボーナスにより【万札】がD3発生。この攻撃を【カルマ：善】のキャラに対して行ってしまった場合、【DKK】がD3上昇する。",
    "「苦しみ抜いて死ぬがいい」急所を情け容赦なく破壊！：本来のダメージ+1ダメージを与える。耐え難い苦痛により、敵は【精神力】が-2され、【ニューロン】が1減少する（これによる最低値は1）。残虐ボーナスにより【万札】がD3発生。この攻撃を【カルマ：善】のキャラに対して行ってしまった場合、【DKK】がD3上昇する。",
    "「逃げられるものなら逃げてみよ」敵の脚を粉砕！：本来のダメージを与える。さらに敵の【脚力】がD3減少する（最低値は1）。残虐ボーナスにより【万札】がD3発生。この攻撃を【カルマ：善】のキャラに対して行ってしまった場合、【DKK】がD3上昇する。",
    "「これで手も足も出まい！」敵の両腕を切り飛ばした！　鮮血がスプリンクラーめいて噴き出す！：本来のダメージ+1ダメージを与える。さらに敵の【ワザマエ】と【カラテ】がそれぞれ2減少する（最低値は1）。残虐ボーナスにより【万札】がD3発生。この攻撃を【カルマ：善】のキャラに対して行ってしまった場合、【DKK】がD3上昇する。",
    "「イイイヤアアアアーーーーッ！」ヤリめいたチョップが敵の胸を貫通！　さらに心臓を掴み取り、握りつぶした！　ナムアミダブツ！：敵は残り【体力】に関係なく即死する。残虐ボーナスにより【万札】がD6発生。この攻撃を【カルマ：善】のキャラに対して行ってしまった場合、【DKK】がD6上昇する。",
]);

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "NinjaSlayer",
            "NinjaSlayer.toml",
            55,
        );
    }
}
