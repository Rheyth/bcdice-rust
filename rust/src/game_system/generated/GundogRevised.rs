//! `lib/bcdice/game_system/GundogRevised.rb` の移植。

use crate::arithmetic;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::{CheckOutcome, EvalResult};
use crate::Int as I;
use regex::Regex;
use std::sync::OnceLock;

static DAMAGE_S: &[&str] = &[
    "対象は[死亡]",
    "[追加D]4D6/[出血]2D6/[重傷]-40％/[朦朧判定]15",
    "[追加D]3D6/[出血]2D6/[重傷]-30％/[朦朧判定]14",
    "[追加D]3D6/[出血]2D6/[重傷]-30％/[朦朧判定]13",
    "[追加D]3D6/[出血]1D6/[重傷]-20％/[朦朧判定]12",
    "[追加D]2D6/[出血]1D6/[重傷]-20％/[朦朧判定]11",
    "[追加D]2D6/[軽傷]-20％/[朦朧判定]11",
    "[追加D]2D6/[軽傷]-20％/[朦朧判定]10",
    "[追加D]2D6/[軽傷]-20％/[朦朧判定]8",
    "[追加D]2D6/[軽傷]-20％/[朦朧判定]6",
    "[追加D]2D6/[軽傷]-20％/[朦朧判定]4",
    "[追加D]2D6/[軽傷]-20％",
    "[追加D]1D6/[軽傷]-20％",
    "[追加D]1D6/[軽傷]-10％",
    "[ショック]-20％",
    "[ショック]-10％",
    "[不安定]",
    "手に持った武器を落とす。複数ある場合はランダム",
    "ペナルティー無し",
];
static DAMAGE_M: &[&str] = &[
    "対象は[死亡]",
    "[追加D]4D6/[出血]2D6/[重傷]-40％/[朦朧判定]15",
    "[追加D]3D6/[出血]2D6/[重傷]-30％/[朦朧判定]14",
    "[追加D]3D6/[出血]1D6/[重傷]-20％/[朦朧判定]14/[不安定]",
    "[追加D]2D6/[出血]1D6/[重傷]-20％/[朦朧判定]14",
    "[追加D]2D6/[重傷]-20％/[朦朧判定]12/[不安定]",
    "[追加D]2D6/[軽傷]-20％/[朦朧判定]11",
    "[追加D]2D6/[軽傷]-20％/[朦朧判定]10",
    "[追加D]2D6/[軽傷]-20％/[朦朧判定]8",
    "[追加D]2D6/[軽傷]-20％/[朦朧判定]6",
    "[追加D]1D6/[軽傷]-20％/[朦朧判定]6",
    "[追加D]1D6/[軽傷]-10％/[朦朧判定]6",
    "[追加D]1D6/[軽傷]-10％/[不安定]",
    "[追加D]1D6/[軽傷]-10％",
    "[ショック]-20％",
    "[ショック]-10％",
    "[不安定]",
    "手に持った武器を落とす。複数ある場合はランダム",
    "ペナルティー無し",
];
static DAMAGE_V: &[&str] = &[
    "[クラッシュ]する。[チェイス]から除外",
    "[車両D]4D6/[乗員D]3D6/[操作性]-40%/[スピン判定]",
    "[車両D]3D6/[乗員D]3D6/[操作性]-30%/[スピン判定]",
    "[乗員D]3D6/[操作性]-20%/[スピン判定]",
    "[車両D]3D6/[操作性]-20%/[スピン判定]",
    "[乗員D]3D6/[操作性]-10%/[スピン判定]",
    "[車両D]3D6/[操作性]-10%/[スピン判定]",
    "[乗員D]2D6/[スピード]-2/[スピン判定]",
    "[車両D]2D6/[スピード]-2/[スピン判定]",
    "[乗員D]2D6/[操縦判定]-20%/[スピン判定]",
    "[車両D]2D6/[操縦判定]-20%/[スピン判定]",
    "[乗員D]2D6/[操縦判定]-20%",
    "[車両D]2D6/[操縦判定]-20%",
    "[車両D]1D6/[操縦判定]-20%",
    "[車両D]1D6/[操縦判定]-10%",
    "攻撃が乗員をかすめる。ランダムな乗員1人に[ショック]-20％",
    "攻撃が乗員に当たりかける。ランダムな乗員1人に[ショック]-10％",
    "車両が蛇行。乗員全員は〈運動〉判定。失敗で[不安定]",
    "ペナルティー無し",
];
static DAMAGE_G: &[&str] = &[
    "対象は[死亡]",
    "[追加D]4D6/[出血]2D6/[重傷]-40％/[朦朧判定]15",
    "[追加D]3D6/[出血]2D6/[重傷]-30％/[朦朧判定]14",
    "[追加D]2D6/[出血]1D6/[重傷]-30％/[朦朧判定]13/[不安定]",
    "[追加D]2D6/[出血]1D6/[重傷]-30％/[朦朧判定]12",
    "[追加D]2D6/[重傷]-20％/[朦朧判定]12/[不安定]",
    "[追加D]1D6/[重傷]-20％/[朦朧判定]11",
    "[追加D]1D6/[軽傷]-30％/[朦朧判定]10",
    "[追加D]1D6/[軽傷]-30％/[朦朧判定]8",
    "[追加D]1D6/[軽傷]-30％/[朦朧判定]6",
    "[追加D]1D6/[軽傷]-20％/[朦朧判定]6",
    "[軽傷]-20％/[朦朧判定]6",
    "[軽傷]-20％/[不安定]",
    "[軽傷]-20％",
    "[軽傷]-10％",
    "[ショック]-20％",
    "[ショック]-10％",
    "[不安定]",
    "ペナルティー無し",
];
static FUMBLE_S: &[&str] = &[
    "銃器が暴発、自分に命中。[貫通D]。武装喪失",
    "銃器が暴発、自分に命中。[非貫通D]。武装喪失",
    "誤射。射線に最も近い味方に命中。[貫通D]",
    "誤射。射線に最も近い味方に命中。[非貫通D]",
    "銃器が完全に故障。直せない",
    "故障。30分かけて〈メカニック〉判定に成功するまで使用不可。",
    "故障。〈メカニック〉-20％の判定に成功するまで使用不可。",
    "故障。〈メカニック〉判定に成功するまで射撃不可",
    "作動不良。[アイテム使用]を2回行って修理するまで射撃不可",
    "作動不良。[アイテム使用]を行って修理するまで射撃不可",
    "足がもつれて倒れる。[転倒]",
    "無理な射撃姿勢で腕を痛める。[軽傷]-20％",
    "無理な射撃姿勢でどこかの筋を痛める。[軽傷]-10％",
    "武装を落とす。スリング（肩ひも）も切れる",
    "武装を落とす。スリング（肩ひも）があれば落とさない",
    "排莢された薬莢が服の中に。[ショック]-20％",
    "排莢された薬莢が顔に当たる。[ショック]-10％",
    "薬莢を踏んで態勢を崩す。[不安定]",
    "ペナルティー無し",
];
static FUMBLE_M: &[&str] = &[
    "自分に命中。[貫通D]",
    "自分に命中。[非貫通D]",
    "最も近い味方（射程内にいなければ自分）に[貫通D]",
    "最も近い味方（射程内にいなければ自分）に[非貫通D]",
    "頭を強く打ちつける。[朦朧]",
    "武装が壊れる。直せない。[格闘タイプ]なら[重傷]-20％",
    "武装がすっぽ抜ける。グレネードの誤差で落下先を決定",
    "武装が損傷。30分かけて〈手先〉判定に成功するまで使用不可。[格闘タイプ]なら[重傷]-10％",
    "武装がガタつく。〈手先〉判定（[格闘タイプ]なら〈強靭〉）に成功するまで使用不可。",
    "武装に違和感。[アイテム使用]を行って調整するまで、命中率-20％",
    "足がもつれる。[転倒]",
    "足がつる。2[ラウンド]の間、移動距離1/2",
    "無理な体勢で腕（あるいは脚）を痛める。[軽傷]-20％",
    "無理な体勢でどこかの筋を痛める。[軽傷]-10％",
    "武装を落とす",
    "武装で自分が負傷。[ショック]-20％",
    "武装の扱いを間違える。[ショック]-10％",
    "攻撃を避けられて体勢を崩す。[不安定]",
    "ペナルティー無し",
];
static FUMBLE_T: &[&str] = &[
    "勢いをつけすぎて転倒し、頭を打つ。[気絶]",
    "自分に命中。（手榴弾なら自分の足元に落ちる）[貫通D]",
    "自分に命中。（手榴弾なら自分の足元に落ちる）[非貫通D]",
    "暴投。射線に最も近い味方に命中。[貫通D]。手榴弾なら新たな中心点からさらに誤差が生じる",
    "暴投。射線に最も近い味方に命中。[非貫通D]。手榴弾なら新たな中心点からさらに誤差が生じる",
    "頭を強く打ちつける。[朦朧]",
    "肩の筋肉断裂。この腕を使う判定に、[重傷]-20％",
    "ヒジの筋肉断裂。この腕を使う判定に、[重傷]-10％",
    "肩の筋をひどく痛める。〈医療〉判定に成功するまで、この腕を使う判定に-20％",
    "肩の筋を痛める。[行動]を使って休めるまで、この腕を使う判定に-20％",
    "腰を痛める。[軽傷]-30％",
    "足がもつれて倒れる。[転倒]",
    "足がつる。2[ラウンド]の間、移動距離1/2",
    "無理な投擲体勢で腕（あるいは脚）を痛める。[軽傷]-20％",
    "無理な投擲体勢でどこかの筋を痛める。[軽傷]-10％",
    "肩に違和感。[ショック]-20％",
    "ヒジに違和感。[ショック]-10％",
    "つまずいて姿勢を崩す。[不安定]",
    "ペナルティー無し",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GundogRevised;

impl GameSystem for GundogRevised {
    fn id(&self) -> &'static str {
        "GundogRevised"
    }
    fn name(&self) -> &'static str {
        "ガンドッグ・リヴァイズド"
    }
    fn sort_key(&self) -> &'static str {
        "かんとつくりうあいすと"
    }
    fn help_message(&self) -> &'static str {
        r"失敗、成功、クリティカル、ファンブルとロールの達成値の自動判定を行います。
nD9ロールも対応。
・ダメージペナルティ表　　(～DPTx) (x:修正)
　射撃(SDPT)、格闘(MDPT)、車両(VDPT)、汎用(GDPT)の各表を引くことが出来ます。
　修正を後ろに書くことも出来ます。
・ファンブル表　　　　　　(～FTx)  (x:修正)
　射撃(SFT)、格闘(MFT)、投擲(TFT)の各表を引くことが出来ます。
　修正を後ろに書くことも出来ます。
"
    }
    fn prefixes(&self) -> &'static [&'static str] {
        &[".DPT", ".FT"]
    }
    crate::impl_prefixes_pattern!();
    fn enabled_d9(&self) -> bool {
        true
    }

    fn result_1d100(
        &self,
        total: crate::Int,
        _dice_total: i64,
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        if cmp_op != CmpOp::Le {
            return None;
        }
        if total >= I::from(100) {
            return Some(CheckOutcome::Result(Box::new(EvalResult::fumble(
                "ファンブル",
            ))));
        }
        if total <= I::ONE {
            return Some(CheckOutcome::Result(Box::new(EvalResult::critical(
                "ベアリー(達成値1+SL)",
            ))));
        }
        let Target::Number(target) = target else {
            return Some(CheckOutcome::Nothing);
        };
        if total > target {
            return Some(CheckOutcome::Result(Box::new(EvalResult::failure("失敗"))));
        }
        let ones = &total % 10;
        let result = if ones == I::ZERO {
            EvalResult::critical("クリティカル(達成値20+SL)")
        } else {
            EvalResult::success(format!("成功(達成値{}+SL)", (&total / 10) + ones))
        };
        Some(CheckOutcome::Result(Box::new(result)))
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        roll_table(command, rng)
    }
}

fn pattern_damage() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)([0-9A-Za-z_])DPT([+\-0-9]*)").unwrap())
}
fn pattern_fumble() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)([0-9A-Za-z_])FT([+\-0-9]*)").unwrap())
}
fn modifier(text: &str) -> Result<i64, EvalError> {
    Ok(arithmetic::eval(text, RoundType::Floor)?
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0))
}
fn damage(head: &str) -> (&'static str, &'static [&'static str]) {
    match head {
        "M" => ("格闘", DAMAGE_M),
        "V" => ("車両", DAMAGE_V),
        "G" => ("汎用", DAMAGE_G),
        _ => ("射撃", DAMAGE_S),
    }
}
fn fumble(head: &str) -> (&'static str, &'static [&'static str]) {
    match head {
        "M" => ("格闘", FUMBLE_M),
        "T" => ("投擲", FUMBLE_T),
        _ => ("射撃", FUMBLE_S),
    }
}
fn roll_table(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let command = command.to_uppercase();
    let mut selected = None;
    let mut table_type = "";
    let mut modify = 0;
    if let Some(m) = pattern_damage().captures(&command) {
        selected = Some(damage(&m[1]));
        table_type = "ダメージペナルティー";
        modify = modifier(&m[2])?;
    }
    if let Some(m) = pattern_fumble().captures(&command) {
        selected = Some(fumble(&m[1]));
        table_type = "ファンブル";
        modify = modifier(&m[2])?;
    }
    let Some((name, table)) = selected else {
        return Ok(Some(SpecificCommandOutput::text("1")));
    };
    let original = [rng.roll_once(10)?, rng.roll_once(10)?]
        .into_iter()
        .filter(|&die| die < 10)
        .sum::<i64>()
        + modify;
    Ok(Some(SpecificCommandOutput::text(format!(
        "{name}{table_type}表[{original}] ＞ {}",
        table[original.clamp(0, 18) as usize]
    ))))
}

#[cfg(test)]
mod tests {
    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;
    use std::path::{Path, PathBuf};

    fn toml_path() -> Option<PathBuf> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .join("test/data/GundogRevised.toml");
        path.exists().then_some(path)
    }
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else { return };
        let data = TestDataFile::load(&path).expect("GundogRevised.toml must parse");
        assert_eq!(
            data.tests.len(),
            36,
            "case count in test/data/GundogRevised.toml"
        );
        let mut failures = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(tc.game_system, "GundogRevised");
            let mut src = SeededRandomizer::new(tc.rands.iter().map(|r| (r.value, r.sides)));
            match eval_command(&GameSystemId::new("GundogRevised"), &tc.input, &mut src) {
                Ok(Some(result)) if !tc.expects_nil() => {
                    if result.text != tc.output
                        || result.secret != tc.secret
                        || result.success != tc.success
                        || result.failure != tc.failure
                        || result.critical != tc.critical
                        || result.fumble != tc.fumble
                    {
                        failures.push(format!(
                            "{}:{}\nexpected: {:?}\nactual: {:?}",
                            i + 1,
                            tc.input,
                            tc.output,
                            result
                        ));
                    }
                }
                Ok(None) if tc.expects_nil() => {}
                other => failures.push(format!("{}:{}: {other:?}", i + 1, tc.input)),
            }
            if !src.is_empty() {
                failures.push(format!(
                    "{}:{}: {} unconsumed rands",
                    i + 1,
                    tc.input,
                    src.remaining()
                ));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
