//! `lib/bcdice/game_system/DesperateRun.rb` の手書き移植。

use crate::command_parser::Parser;
use crate::dice_table::{RollableTable, Table};
use crate::enums::{D66SortType, RoundType};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int as I;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DesperateRun;

impl GameSystem for DesperateRun {
    fn id(&self) -> &'static str {
        "DesperateRun"
    }
    fn name(&self) -> &'static str {
        "Desperate Run TRPG"
    }
    fn sort_key(&self) -> &'static str {
        "てすへれいとらんTRPG"
    }
    fn help_message(&self) -> &'static str {
        r"・難易度算出コマンド　DDC
・判定コマンド　RCx　or　RCx+y　or　RCx-y（x＝難易度、y=修正値（省略可能））
・アクシデント表　ACT
・初期アイテム表　ITEMT
・動機表　DOUKIT
・死亡フラグ台詞表　FLAGT
・途中参加動機表　ENTRYT
・途中参加道中表　ROADT
"
    }
    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "RC", "DDC", "ACT", "ITEMT", "DOUKIT", "FLAGT", "ENTRYT", "ROADT",
        ]
    }
    crate::impl_prefixes_pattern!();
    fn sort_add_dice(&self) -> bool {
        true
    }
    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(cmd) = Parser::new(&[r"RC\d+"], RoundType::Floor)
            .restrict_cmp_op_to(&[None])
            .parse(command)
        {
            let dice = rng.roll_barabara(2, 6)?;
            let d1 = dice[0];
            let d2 = dice[1];
            let total = d1 + d2 + cmd.modify_number.clone();
            let target = cmd.command[2..].parse::<i64>().unwrap_or(i64::MAX);
            let modifier = if cmd.modify_number == I::ZERO {
                String::new()
            } else {
                format!("　修正値：{}", cmd.modify_number)
            };
            let mut result = if d1 == d2 {
                EvalResult::critical("ゾロ目！【Critical】")
            } else if d1 + d2 == 7 {
                EvalResult::fumble("ダイスの出目が表裏！【Fumble】")
            } else if total >= crate::Int::from(target) {
                EvalResult::success(format!("{total}、難易度以上！【Success】"))
            } else {
                EvalResult::failure(format!("{total}、難易度未満！【Miss】"))
            };
            result.text = format!(
                "判定　難易度：{target}{modifier} ＞ 出目：{d1}、{d2} ＞ {}",
                result.text
            );
            return Ok(Some(SpecificCommandOutput::result(result)));
        }

        if command == "DDC" {
            let dice = rng.roll_barabara(2, 6)?;
            let d1 = dice[0];
            let d2 = dice[1];
            let smaller = d1.min(d2);
            let larger = d1.max(d2);
            let difference = larger - smaller;
            return Ok(Some(SpecificCommandOutput::text(format!(
                "難易度決定 ＞ 出目：{d1}、{d2} ＞ {larger}-{smaller}={difference} ＞ 難易度{}",
                5 + difference
            ))));
        }

        match TABLES.iter().find(|(key, _)| *key == command) {
            Some((_, table)) => Ok(Some(SpecificCommandOutput::text(
                table.roll(rng)?.to_string(),
            ))),
            None => Ok(None),
        }
    }
}

static ACT: Table = Table::from_dice(
    "アクシデント表",
    1,
    6,
    &[
        "PC全員、隊列を1D6で決める（12：前列、34：中列、56：後列）",
        "1ターン、モンスター側のみ行動する。",
        "この戦闘の行動順が、PC→モンスター、から、モンスター→PCに変わる。",
        "PC全員、アイテム1つを選んで失う。",
        "PC全員1D6を振る。この戦闘中、その出目はノープランになる。",
        "PC全員、Flagが1増加する。",
    ],
);
static ITEMT: Table = Table::from_dice("初期アイテム表", 2, 6, &[
    "ロケットランチャー　効果：（装備）マカブルLv+3　回数：攻撃1回",
    "キーピック　効果：（消費）ムーブシーンで使用可。判定の出目の合計+1　回数：2回",
    "レーダー　効果：（装備）ノープラン　装備時、「危険」を1減らすことが出来る　回数：危険減少2回",
    "食料　効果：（消費）ムーブシーンで使用可。Life3回復　回数：1回",
    "応急キット　効果：（装備）メディカルLv+1　回数：回復3回",
    "刃物　効果：（装備）アタックLv+1　回数：攻撃6回",
    "銃　効果：（装備）シュートLv+1　回数：攻撃6回",
    "ドラッグ　効果：（消費）いつでも使用可。Brave1回復　回数：1回",
    "金券　効果：（消費）アフタープレイで使用可。経験点+2　回数：1回",
    "プロテクター　効果：（装備）ガードLv+1　回数：防御5回",
    "トミーガン　効果：（装備）チェーンLv+5　回数：攻撃1回",
]);
static DOUKIT: Table = Table::from_dice("動機表", 1, 6, &[
    "隷属。何か弱みに付け込まれ、嫌々ながらも参加することとなる。後ろめたい事、借金、など。",
    "献身。この番組は誰かのために出ている。病気の家族を救うため、参加者の一人が恋人である、など。",
    "成行。望んでいないのに参加することとなってしまった。誰かが勝手に申し込んだ、紛れ込んでしまった、など。",
    "渇望。とにかく何かを欲して参加している。金、スリル、金で手に入るもの、など。",
    "奇人。好き好んでこの番組に出ている。殺人癖、ナルシスト、自殺願望、化け物マニア、など。",
    "仕事。この番組に出るのは仕事だからだ。賞金稼ぎ、芸能人、記者、番組スタッフ、など。",
]);
static FLAGT: Table = Table::from_dice("死亡フラグ台詞表", 1, 6, &[
    "希望。「これが終わったら、一緒に酒でも飲もうや」「もう何も怖くない」など。",
    "望郷。「くそっ、、、俺には、帰りを待つ人が、、、っ！」「ああ、故郷のマルゲリータをもう一度食べたかった、、、」など。",
    "狂乱。「お前を、殺せば、俺は、億万長者なんだよぉぉぉお！！」「ヒハッ、死ね死ね死ね死ねぇぇぇぇ」など。",
    "絶望。「もうだめだぁ、、、おしまいだぁ、、、」「く、くるなっ、くるなぁぁぁぁ！！」など。",
    "慢心。「なんだ、こんなやつ、俺だけで十分だ」「大丈夫だ、問題ない」など。",
    "犠牲。「ふふ、俺なら大丈夫だ、、、気にするな」「お前ら下がれっ！ここは俺に任せろ！」など。",
]);
static ENTRYT: Table = Table::from_dice("途中参加動機表", 1, 6, &[
    "遅刻。もともと参加する予定だったのだが遅れてしまった。今からでも走れと無理矢理参加。",
    "現住。もともとここに住んでいた。何？番組？え？あ、お金もらえんの？イイネ！",
    "突発。もともと視聴者としてスタジオに居たんだけど、司会にうまく乗せられて・・・",
    "乱入。何か目的があって乱入した。今回のステージが簡単に見えたり、参加者に大事な人がいたのかもしれない。",
    "神隠。今回より前の番組に参加していたが行方不明・死んだと思われていた。が。君はまだここにいる。",
    "職員。あれ、逃げ遅れました？あらあら、大変ですね、しょうがないから参加者さんと一緒に走ってください。",
]);
static ROADT: Table = Table::from_dice(
    "途中参加道中表",
    1,
    6,
    &[
        "死線。死ぬかと思った。Flag+1",
        "怪我。痛い。Life-1",
        "失意。どうしてこうなった。Brave-1。減らせない場合、Flag+1",
        "紛失。どっかいった。Itemをどれか1つ捨てる。捨てれない場合、Flag+1",
        "追尾。後ろ！後ろーっ！参加開始部屋の危険+1",
        "迷子。あれ、ここどこだ？途中参加道中表を振る回数+2",
    ],
);
static TABLES: [(&str, &Table); 6] = [
    ("ACT", &ACT),
    ("ITEMT", &ITEMT),
    ("DOUKIT", &DOUKIT),
    ("FLAGT", &FLAGT),
    ("ENTRYT", &ENTRYT),
    ("ROADT", &ROADT),
];

#[cfg(test)]
mod tests {
    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;
    use std::path::Path;

    #[test]
    fn all_toml_cases_pass() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("test/data/DesperateRun.toml");
        if !path.exists() {
            return;
        }
        let data = TestDataFile::load(&path).expect("DesperateRun.toml must parse");
        assert_eq!(
            data.tests.len(),
            57,
            "case count in test/data/DesperateRun.toml"
        );
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(tc.game_system, "DesperateRun");
            let mut src = SeededRandomizer::new(tc.rands.iter().map(|r| (r.value, r.sides)));
            let result = eval_command(&GameSystemId::new("DesperateRun"), &tc.input, &mut src)
                .unwrap_or_else(|e| panic!("case {} {}: {e}", i + 1, tc.input));
            if tc.expects_nil() {
                assert!(result.is_none(), "case {} {}", i + 1, tc.input);
            } else {
                let result = result.unwrap_or_else(|| panic!("case {} {}: nil", i + 1, tc.input));
                assert_eq!(result.text, tc.output, "case {} {} text", i + 1, tc.input);
                assert_eq!(
                    (
                        result.secret,
                        result.success,
                        result.failure,
                        result.critical,
                        result.fumble
                    ),
                    (tc.secret, tc.success, tc.failure, tc.critical, tc.fumble),
                    "case {} {} flags",
                    i + 1,
                    tc.input
                );
            }
            assert!(
                src.is_empty(),
                "case {} {}: {} rands remain",
                i + 1,
                tc.input,
                src.remaining()
            );
        }
    }
}
