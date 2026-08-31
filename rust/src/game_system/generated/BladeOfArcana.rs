use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BladeOfArcana;

impl GameSystem for BladeOfArcana {
    fn id(&self) -> &'static str {
        "BladeOfArcana"
    }
    fn name(&self) -> &'static str {
        "ブレイド・オブ・アルカナ"
    }
    fn sort_key(&self) -> &'static str {
        "ふれいとおふあるかな"
    }
    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }
    fn prefixes(&self) -> &'static [&'static str] {
        PREFIXES
    }
    crate::impl_prefixes_pattern!();
    fn sort_add_dice(&self) -> bool {
        true
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        Ok(eval_specific(command, rng)?.map(SpecificCommandOutput::text))
    }
}

static HELP_MESSAGE: &str = r"■行為判定　nA[m][Cx][Fy]
　n：ダイス数　　m：判定値(省略時はクリティカル値と同じ)
　x：クリティカル値(省略時は1)　　y：ファンブル値(省略時は20)
　注）[m]、[Cx]、[Fy]は省略可能。
　　例）3A12C4F15→3個振り12以下で成功。C値4、F値は15。
　　例）3A12→3個振り12以下で成功。C値1、F値は20。

■各種表　(+：出目2～21に変更　-：出目0～19に変更)
●リインカーネイション
　因縁表　CTR[+/-]　　前世邂逅表　DJV[-]
　悪徳シーン表　AKST[+/-]
●The 3rd（第三版）
　因縁表　CT3[+/-]
　注）[]内は省略可能。
　　例）CTR→因縁表（R版）を出目1～20でロールする。
　　例）CTR+→因縁表（R版）を出目2～21でロールする。
";
static PREFIXES: &[&str] = &[
    r"\d+A",
    r"CT3[\+\-]?",
    r"CTR[\+\-]?",
    r"DJV\-?",
    r"AKST[\+\-]?",
];

fn action_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^(\d+)A(\d*)([CF]?)(\d*)([CF]?)(\d*)$").expect("valid regex")
    })
}

fn eval_specific(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let upper = command.to_ascii_uppercase();
    if let Some(m) = action_pattern().captures(&upper) {
        let option1 = &m[3];
        let argument1 = &m[4];
        let option2 = &m[5];
        let argument2 = &m[6];
        if option1.is_empty() != argument1.is_empty()
            || option2.is_empty() != argument2.is_empty()
            || (!option2.is_empty() && option1 == option2)
        {
            return Ok(None);
        }
        let (critical, fumble) = if option1 == "C" {
            (to_i(argument1), to_i(argument2))
        } else {
            (to_i(argument2), to_i(argument1))
        };
        return Ok(Some(roll_action(
            to_i(&m[1]),
            to_i(&m[2]),
            critical,
            fumble,
            rng,
        )?));
    }
    let table = if let Some(sign) = upper
        .strip_prefix("CT3")
        .filter(|s| matches!(*s, "" | "+" | "-"))
    {
        Some(("因縁表(The 3rd)　『BoA3』P292", CT3, sign))
    } else if let Some(sign) = upper
        .strip_prefix("CTR")
        .filter(|s| matches!(*s, "" | "+" | "-"))
    {
        Some(("因縁表(リインカーネイション)　『BAR』P51、299", CTR, sign))
    } else if let Some(sign) = upper.strip_prefix("DJV").filter(|s| matches!(*s, "" | "-")) {
        Some(("前世邂逅表（デジャブ）　『BAR』P235", DJV, sign))
    } else {
        upper
            .strip_prefix("AKST")
            .filter(|s| matches!(*s, "" | "+" | "-"))
            .map(|sign| ("悪徳シーン表　『GoV』P16、164", AKST, sign))
    };
    table
        .map(|(title, values, sign)| table_text(title, values, sign, rng))
        .transpose()
}

fn to_i(value: &str) -> i64 {
    value.parse().unwrap_or(0)
}

fn roll_action(
    mut count: i64,
    mut judgment: i64,
    mut critical: i64,
    mut fumble: i64,
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    critical = critical.max(1);
    if judgment <= 0 {
        judgment = critical;
    } else {
        critical = critical.min(judgment);
    }
    if fumble <= 0 {
        fumble = 20;
    }
    if count <= 0 {
        count = 1;
        fumble -= 5;
    }
    fumble = fumble.clamp(2, 20);
    let mut dice = rng.roll_barabara(count, 20)?;
    dice.sort_unstable();
    let list = dice
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let value = if count == 1 {
        dice.iter().sum()
    } else {
        dice[0]
    };
    let mut text = format!("({count}A{judgment}C{critical}F{fumble}) ＞ {list} ＞ ");
    if count != 1 {
        text.push_str(&format!("{value} ＞ "));
    }
    text.push_str(if value >= fumble {
        "ファンブル"
    } else if value <= critical {
        "クリティカル"
    } else if value > judgment {
        "失敗"
    } else {
        "成功"
    });
    Ok(text)
}

fn table_text(
    title: &str,
    table: &[&str],
    sign: &str,
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let number = rng.roll_once(20)?;
    let index = number
        + if sign == "+" {
            1
        } else if sign == "-" {
            -1
        } else {
            0
        };
    let suffix = if sign.is_empty() {
        String::new()
    } else {
        format!("[{number}{sign}1]")
    };
    Ok(format!(
        "{title} ＞ {index}{suffix} ＞ {}",
        table[index as usize]
    ))
}

static CT3: &[&str] = &[
    "【他生】",
    "【師弟】",
    "【忘却】",
    "【兄姉】",
    "【貸し】",
    "【慕情】",
    "【主従】",
    "【強敵】",
    "【秘密】",
    "【恩人】",
    "【告発】",
    "【友人】",
    "【仇敵】",
    "【父母】",
    "【借り】",
    "【信頼】",
    "【幼子】",
    "【取引】",
    "【地縁】",
    "【同志】",
    "【不審】",
    "【自身】",
];
static CTR: &[&str] = &[
    "【他生】",
    "【師弟】",
    "【忘却】",
    "【兄姉】",
    "【貸し】",
    "【憧憬】",
    "【主従】",
    "【強敵】",
    "【秘密】",
    "【恩人】",
    "【取引】",
    "【友人】",
    "【怨敵】",
    "【後援】",
    "【借り】",
    "【信頼】",
    "【弟妹】",
    "【商売】",
    "【奇縁】",
    "【同志】",
    "【有為】",
    "【自身】",
];
static DJV: &[&str] = &[
    "【鮮烈な風】\n風は懐かしい匂いを、香りを運んでくる。それは……。",
    "【薄暗い影】\nまるで時が止まってしまっているかのようだ。",
    "【操りの糸】\nそれはあなたを導く蜘蛛の糸。",
    "【天上の光】\n偉大なるものがもたらす、天上からの御しるし。",
    "【温もり】\n春のひなたのような温かさを感じる。",
    "【鋭いナイフ】\n鋭いナイフのような視線を感じる。これは……。",
    "【共鳴】\n同じ感覚を感じる、ふたりは通じ合っている。",
    "【城壁】\n厳しく高い城壁のように重く堅く厚い。",
    "【砕ける器】\n落ちれば砕ける。砕ければそれは器ではない。",
    "【陽炎】\n求めれば揺らいで消える。",
    "【終わりなき円環】\nそれはあなたを捉え巡る輪廻の輪。",
    "【天秤】\n揺れるバランス、揺れ続ける安定。",
    "【流れる水】\nひとつ所にとどまらず、姿を固めることはない",
    "【光る刃】\n鋭く光る刃のような、鋭いまなざし。",
    "【悪魔】\nあまりにも危険な魅力、それは悪魔的だった。",
    "【牙】\n獲物を引き裂く鋭く長い、牙。",
    "【輝く星】\n星は暗く小さい。だがそこに輝く。",
    "【冴え渡る月光】\n冷たさと安らかさが同居している。",
    "【照りつける太陽】\n暑い。",
    "【燃えさかる炎】\n炎はすべてを破壊し、すべてを滅ぼす。",
    "【世界】\nすべてはこの世界の中で起こり、終わる。",
    "【なし】",
];
static AKST: &[&str] = &[
    "▼ウェントス／止まない風\n【行動】殺戮者の狂気に当てられたのか、通り魔的殺人者が現れる。切り裂かれた人々の悲鳴が響き渡る。","▼エフェクトス／原初の力\n【行動】殺戮者の配下が無法を働く。店先で金品を要求したり、暴力を振るったりしている。","▼クレアータ／傀儡人形の王\n【行動】殺戮者の配下が人々の行動を監視している。違反した者には即座に罰が与えられる。","▼マーテル／生ける神\n【行動】殺戮者の配下が人々に殺戮者への信仰を告白し、忠誠を宣誓するように強要している。","▼コロナ／簒奪者\n【行動】嘆き悲しんでいる者がいる。殺戮者によって、財産、地位、家族あるいは、恋人を奪い取られたという。","▼フィニス／永遠の人\n【行動】怪物が人々を虐殺している。この地には人間が多すぎるのだという。それが彼らの主の決定だ。","▼エルス／無私なる愛\n【行動】殺戮者の配下が略奪を働いている。どうやら、殺戮者に献上するものを争っているようだ。","▼アダマス／万物の保護者\n【行動】反逆者と名指しされる。人々は君たちに接触しようとしない。情報を集めるにも苦労しそうだ。","▼アルドール／終わりなき戦い\n【行動】ならず者の集団が人々を襲っている。力を示さなければ切り捨てられるのは彼らなのだ。","▼ファンタスマ／謀略の渦\n【行動】人々は君を見るなり逃げ出した。どうやら恐ろしい殺人者だと思われているようだ。","▼アクシス／真理の探究者\n【行動】殺戮者の配下の手によって、人々が連れ去られている。誰ひとりもどってこない。","▼レクス／捕縛者\n【行動】殺戮者への恐怖に駆られた人々はその命令にしたがって徒党を組み、PCたちを捜索している。","▼アクア／澱んだ水\n【行動】人々は獣のように生きている。言葉は通じない。有効なのは力、暴力だけだ。","▼グラディウス／暗き死の刃\n【行動】殺戮者とその配下によって虐殺が行なわれている。見渡す限り、死者ばかりだ。","▼アングルス／純白の恐怖\n【行動】遊びとして人間狩りが行なわれている。人々は逃げ惑い、殺戮者は愉悦に笑う。","▼ディアボルス／悪魔の囁き\n【行動】殺戮者は少年少女を召し上げている。召し上げられた者たちは音信不通となってしまう。","▼フルキフェル／裏切り者\n【行動】人々は猜疑の目で君を見る。嘘を吐くのが普通の場所で真実を見いだせるだろうか。","▼ステラ／破滅への愛\n【行動】街や村落が破壊されている。焼け野原の中、人々は力なくうずくまる。ここには絶望だけがあった。","▼ルナ／奪う者\n【行動】君たちの目の前に略奪が繰り返される。略奪のために略奪を行なう殺戮者の配下たち。","▼デクストラ／邪悪な技\n【行動】殺戮者による非道な人体実験が繰り返されている。そのための実験材料が集められている。","▼イグニス／根源たる炎\n【行動】街や集落、あるいは店舗や住宅が焼き討ちに合う。人々は互いに陥れ、磔刑が行なわれている。","▼オービス／闇の鎖\n【行動】世界の完全なる破滅、人類の絶滅、無作為で広範囲な虐殺が行なわれる。",
];

#[cfg(test)]
mod tests {
    use crate::{
        eval::eval_command, game_system::GameSystemId, randomizer::SeededRandomizer,
        toml_test::TestDataFile,
    };
    use std::path::Path;
    #[test]
    fn all_toml_cases_pass() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("test/data/BladeOfArcana.toml");
        if !path.exists() {
            return;
        }
        let data = TestDataFile::load(&path).expect("BladeOfArcana.toml must parse");
        assert_eq!(data.tests.len(), 34);
        let mut failures = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            let mut rng = SeededRandomizer::new(tc.rands.iter().map(|r| (r.value, r.sides)));
            let result = eval_command(&GameSystemId::new("BladeOfArcana"), &tc.input, &mut rng);
            let matches = match &result {
                Ok(None) => tc.expects_nil(),
                Ok(Some(r)) => {
                    !tc.expects_nil()
                        && r.text == tc.output
                        && r.secret == tc.secret
                        && r.success == tc.success
                        && r.failure == tc.failure
                        && r.critical == tc.critical
                        && r.fumble == tc.fumble
                }
                Err(_) => false,
            };
            if !matches || (!tc.expects_nil() && !rng.is_empty()) {
                failures.push(format!(
                    "case {} {:?}: expected {:?}, got {:?}, remaining={}",
                    i + 1,
                    tc.input,
                    tc.output,
                    result,
                    rng.remaining()
                ));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
