//! P4で手書き移植した `lib/bcdice/game_system/CharonSanctions.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `CharonSanctions#action_roll`（`nCSm>=t` の判定）
//! - `TABLES`（感情ワード（キャラクター）表 `ET` / 襲撃表 `RT`）

use crate::command_parser::{Parser, SuffixPosition};
use crate::dice_table::{D66RangeTable, RangeInc, RollableTable, Table};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::CharonSanctions`（ID: `CharonSanctions`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharonSanctions;

impl GameSystem for CharonSanctions {
    fn id(&self) -> &'static str {
        "CharonSanctions"
    }

    fn name(&self) -> &'static str {
        "カローン・サンクションズ"
    }

    fn sort_key(&self) -> &'static str {
        "かろおんさんくしおんす"
    }

    fn help_message(&self) -> &'static str {
        r"■ 判定
  nCSm>=t ［判定］を行う。成功／不完全成功を判定。（p.111）
  n: ダイス数
  m: ［難易度］（省略時 4）
  t: ［必要成功数］（省略時 1）

■ 各種表
  ET 感情ワード（キャラクター）表（p.120）
  RT 襲撃表（p.146）
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+CS", "ET", "RT"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `CharonSanctions#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(result) = action_roll(self, command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }

        Ok(roll_tables(command, rng)?.map(SpecificCommandOutput::text))
    }
}

/// Ruby `CharonSanctions#action_roll`。
fn action_roll(
    system: &CharonSanctions,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let parser = Parser::new(&["CS"], system.round_type())
        .has_prefix_number()
        .enable_suffix_number()
        .disable_modifier()
        .restrict_cmp_op_to(&[None, Some(CmpOp::Ge)]);
    let Some(cmd) = parser.parse(command) else {
        return Ok(None);
    };

    // Ruby: times = cmd.prefix_number（has_prefix_number なので必ず存在する）
    let times = cmd
        .prefix_number
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0);
    // Ruby: return nil if times < 1
    if times < 1 {
        return Ok(None);
    }

    // Ruby: (cmd.suffix_number || 4).clamp(2, 6)
    let required = cmd
        .suffix_number
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(4)
        .clamp(2, 6);
    let target = cmd.target_number.clone().unwrap_or(crate::Int::from(1));

    let dice_list = rng.roll_barabara(times, 6)?;
    let count = dice_list.iter().filter(|i| **i >= required).count();

    let mut result = if (count as i64) >= crate::randomizer::sat_i64(&target) {
        EvalResult::success("成功")
    } else {
        EvalResult::failure("不完全成功")
    };

    let dice_str = dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let sequence = [
        format!("({})", cmd.to_s(SuffixPosition::AfterCommand)),
        format!("({times}B6>={required}[>={target}])"),
        format!("[{dice_str}]"),
        format!("成功数:{count}"),
        result.text.clone(),
    ];

    result.text = sequence.join(" ＞ ");
    Ok(Some(result))
}

/// Ruby `Base#roll_tables(command, tables)`。
///
/// `ET` は `D66RangeTable`、`RT` は `Table` で型が違うので、`to_s` した文字列で揃える。
fn roll_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    match command {
        "ET" => Ok(Some(ET.roll(rng)?.to_string())),
        "RT" => Ok(Some(RT.roll(rng)?.to_string())),
        _ => Ok(None),
    }
}

/// Ruby `TABLES["ET"]`（感情ワード（キャラクター）表）の項目。
static ET_ITEMS: &[(RangeInc, &str)] = &[
    (RangeInc::new(11, 12), "愛情／あなたは、対象のキャラクターに愛情を抱いています。"),
    (RangeInc::new(13, 14), "家族／あなたは、対象のキャラクターをまるで家族のように感じています。"),
    (RangeInc::new(15, 16), "腐れ縁／あなたは、対象のキャラクターを腐れ縁だとを感じています（または、まるで長年の腐れ縁のように気が合う相手だと感じています）。"),
    (RangeInc::new(21, 22), "師弟／そのキャラクターとは、師弟のような関係だと感じています。"),
    (RangeInc::new(23, 24), "好敵手／そのキャラクターを、好敵手だと感じています。"),
    (RangeInc::new(25, 26), "親近感／あなたは、対象のキャラクターに、親近感を抱いています。"),
    (RangeInc::new(31, 32), "友情／あなたは、対象のキャラクターに、友情を抱いています。"),
    (RangeInc::new(33, 34), "尊敬／あなたは、対象のキャラクターを尊敬しています。"),
    (RangeInc::new(35, 36), "庇護／あなたは、対象のキャラクターを守らなければと感じています。"),
    (RangeInc::new(41, 42), "好感／あなたは、対象のキャラクターに好感を抱いています。"),
    (RangeInc::new(43, 44), "興味／あなたは、対象のキャラクターに興味を抱いています。"),
    (RangeInc::new(45, 46), "感銘／あなたは、対象のキャラクターに感銘を抱いています。"),
    (RangeInc::new(51, 52), "畏怖／あなたは、対象のキャラクターに畏怖を抱いています。"),
    (RangeInc::new(53, 54), "信頼／あなたは、対象のキャラクターに信頼を感じています。"),
    (RangeInc::new(55, 56), "不信感／あなたは、対象のキャラクターに不信感を抱いています。"),
    (RangeInc::new(61, 62), "劣等感／あなたは、対象のキャラクターの能力や容姿、実績などに対し劣等感を抱いています。"),
    (RangeInc::new(63, 64), "後悔／あなたは対象のキャラクターを見ると、後悔の念を思い出します（かつて失った人に似ている、など）。"),
    (RangeInc::new(65, 66), "無関心／あなたは、対象のキャラクターに対して無関心を装っています。しかし本当は無視できない存在であると感じています。"),
];

/// Ruby `DiceTable::D66RangeTable.new("感情ワード（キャラクター）表", …)`。
static ET: D66RangeTable = D66RangeTable::new("感情ワード（キャラクター）表", ET_ITEMS);

/// Ruby `TABLES["RT"]`（襲撃表）の項目。
static RT_ITEMS: &[&str] = &[
    "概要：今回の事件の黒幕が放ったものか、それとも何かで恨みを買ったのか、裏社会の暗殺者たちが襲撃を仕掛けてくる。迎え撃つ必要がある。\n判定：2人で判定。【機敏(SR+1)】〔†射撃〕／【身体(SR+1)】〔†白兵〕\n全員が完全成功：敵の迎撃に成功した。襲撃者を放ってきた相手もしばらくは動けないだろう。【AP】+1。\n1人でも不完全成功：何とか敵を迎撃したが、手傷を負ってしまった。PC全員は【HP】を1d+[SR]点消費する。",
    "概要：あまりにも目立ちすぎたせいか、自分に目を光らせている司法組織の捜査官が追ってきた。戦うわけにもいかない。誰かが囮になってうまく逃げるしかない。\n判定：1人で判定。【身体(SR+2)】〔運動〕\n全員が完全成功：うまく追手をまくことができた。しばらくは時間を稼ぐことができるだろう。【AP】+1。\n1人でも不完全成功：何とか逃げることには成功したが、体力を使い果たした。判定を行ったPCは、[調査フェイズ]終了時まで【身体】の判定で振るダイス数-1d。",
    "概要：偶然、一般市民に非合法の活動をしている場面を目撃される。彼らを巻き込まないためにも、何とか誤魔化した方がいいだろう。\n判定：全員で判定。【知性(SR)】〔交渉〕\n全員が完全成功：それらしい理屈をつけて、誤魔化すことができた。機転の利いた対応に、裏社会での評価も上がる。【畏敬】+2。\n1人でも不完全成功：なんとか誤魔化せたが、相手はいまいち納得できないようだ。今後、彼らの動向にも気を配る必要があるだろう。[不完全成功]だったPCは[調査フェイズ]終了まで【知性】の判定で振るダイス数-1d。",
    "概要：情報を整理しようと集まった瞬間、何者かに襲撃を受ける。出自は不明だが、裏社会の存在で間違いないだろう。迎撃の必要がある。\n判定：全員で判定。【機敏(SR)】〔†射撃〕／【身体(SR)】〔†白兵〕\n全員が完全成功：鮮やかに敵を撃退してみせた。【畏敬】+2。\n1人でも不完全成功：敵を撃退できたものの、手傷を負ってしまった。[不完全成功]だったPCは[調査フェイズ]終了まで【機敏】の判定で振るダイス数-1d。",
    "概要：君の活躍に目をつけ、闇の組織が急な情報収集の仕事を依頼してきた。やむにやまれぬ事情があり、無視することもできない。急いで片付けた方がいいだろう。\n判定：2人で判定。【知性(SR+1)】〔コンピュータ〕／【機敏(SR+1)】〔操縦〕\n全員が完全成功：コンピュータやフットワークを活かし、情報を素早く手に入れた。闇社会からの評価も上がる。【畏敬】+2。\n1人でも不完全成功：仕事は成功させたものの、少し雑なものとなって相手を落胆させてしまう。【畏敬】-1。",
    "概要：カローンのことを嗅ぎ回っているマスコミに、活動現場を見られてしまった。何とか言いくるめて、対処する必要がある。\n判定：1人で判定。【知性(SR+2)】〔交渉〕\n全員が完全成功：自分たちはカローンではないと言いくるめることができた。ついでに事件捜査の時間を稼ぐための偽情報をリークすることにも成功する。【AP】+1。\n1人でも不完全成功：何とか言いくるめることができたが、激しく気力を消耗してしまう。判定を行ったPCは、【EP】-1。",
];

/// Ruby `DiceTable::Table.new("襲撃表", "1D6", …)`。
static RT: Table = Table::from_dice("襲撃表", 1, 6, RT_ITEMS);

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "CharonSanctions",
            "CharonSanctions.toml",
            17,
        );
    }
}
