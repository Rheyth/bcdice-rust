//! P4で手書き移植した `lib/bcdice/game_system/UnsungDuet.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `UnsungDuet#eval_game_system_specific_command`
//!   （`ALIAS` の解決 → `shifter`/`UDS` を `1D10`、`binder`/`UDB` を `2D6` に置換して
//!   `CommonCommand::AddDice` に委譲 → 変異表）
//! - `UnsungDuet#roll_replaced_command_if_match`
//!
//! 表データは `i18n/UnsungDuet/ja_jp.yml` から機械的に書き出したもので、値は1文字も変えていない。
//! ロケール差のあるデータは [`SystemTables`] に束ね、
//! `UnsungDuet_Korean`（`ko_kr`）が同じ関数群を使い回す。

use std::sync::OnceLock;

use regex::Regex;

use crate::common_command::add_dice;
use crate::dice_table::{RollableTable, Table};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

static JA_HIN_ITEMS: &[&str] = &[
    "顔の傷 → 顔にできた傷。じわりと血がにじむ",
    "大きな怪我 → 早く手当てをしないと命に関わる",
    "痛みのない傷 → 大きな傷なのに、なぜか痛くない",
    "喪失 → 身体のどこかが消えてしまった",
    "文字のような傷跡 → 読めない文字のような傷",
    "模様を描くアザ → 模様のような、身体にできたアザ",
];
static JA_HIN: Table = Table::from_dice("変異表：外傷", 1, 6, JA_HIN_ITEMS);

static JA_HPH_ITEMS: &[&str] = &[
    "視界がぼやける → 目の焦点が合わない",
    "耳鳴り → ずっと甲高い音が鳴り続けている気がする",
    "異様な寒気 → 凍えそうなほどに寒く感じる",
    "発汗 → 暑いわけでもないのに、汗がだらだらと",
    "幻覚 → それが本物か幻か、区別がつかない",
    "走馬灯 → 過去のことを次々と思い出してしまう",
];
static JA_HPH: Table = Table::from_dice("変異表：体調の変化", 1, 6, JA_HPH_ITEMS);

static JA_HFE_ITEMS: &[&str] = &[
    "不安 → 漠然とした不安が心を蝕む",
    "狭い場所が怖い → 狭い場所に入りたくない",
    "震えが止まらない → どうしても落ち着かない",
    "物音が怖い → ほんの小さな物音にも怯えてしまう",
    "暗い場所が怖い → 灯りのない場所がひどく恐ろしい",
    "誰かがついてくる → 誰かが後ろにいる気がする……",
];
static JA_HFE: Table = Table::from_dice("変異表：恐怖", 1, 6, JA_HFE_ITEMS);

static JA_HFA_ITEMS: &[&str] = &[
    "硝子化 → 身体の一部がガラスのように透明に",
    "羽毛化 → 身体のどこかから羽毛が生えてくる",
    "植物化 → ツタや葉が身体から生えてくる",
    "動物の瞳 → 瞳の形が動物のそれに変わってしまう",
    "有角化 → 額や側頭部から角が生えてくる",
    "陶器化 → 皮膚が陶器のようなものに変わっていく",
];
static JA_HFA: Table = Table::from_dice("変異表：幻想化", 1, 6, JA_HFA_ITEMS);

static JA_HMI_ITEMS: &[&str] = &[
    "記憶の混乱 → ここはどこ、どうしてこんなところに?",
    "幼少期の記憶 → 口調や態度が幼くなってしまう",
    "素直 → 思ったことを全部言ってしまう",
    "蛮勇 → パートナーを守るために無茶ばかりする",
    "疑心暗鬼 → 何もかもが悪い方向にしか考えられない",
    "食べちゃいたい → パートナーをかじりたくなる",
];
static JA_HMI: Table = Table::from_dice("変異表：精神", 1, 6, JA_HMI_ITEMS);

static JA_HOT_ITEMS: &[&str] = &[
    "影絵化 → 身体の一部が影のようになる",
    "水槽化 → 身体の一部が水槽のようなものになる",
    "涙が止まらない → なぜか涙が流れ続ける",
    "鉤爪 → 手や足が、獣のような鉤爪になる",
    "未来視 → 未来が見えてしまう。本当かどうかは不明",
    "帰りたくない → 現実に帰りたくないと微かに思う",
];
static JA_HOT: Table = Table::from_dice("変異表：そのほか", 1, 6, JA_HOT_ITEMS);

/// Ruby `TABLES`（`translate_tables(:ja_jp)`）。
pub(crate) static JA_TABLES: &[(&str, &Table)] = &[
    ("HIN", &JA_HIN),
    ("HPH", &JA_HPH),
    ("HFE", &JA_HFE),
    ("HFA", &JA_HFA),
    ("HMI", &JA_HMI),
    ("HOT", &JA_HOT),
];

/// 1ロケール分の表と定型文。
pub(crate) struct SystemTables {
    /// Ruby `TABLES`（変異表）
    pub(crate) tables: &'static [(&'static str, &'static Table)],
    /// i18n `success`（`Base#result_ndx` が使う）
    pub(crate) success: &'static str,
    /// i18n `failure`
    pub(crate) failure: &'static str,
}

pub(crate) static JA_SYSTEM: SystemTables = SystemTables {
    tables: JA_TABLES,
    success: "成功",
    failure: "失敗",
};

/// Ruby `ALIAS`（`transform_keys(&:upcase)` 済み）。
const ALIAS: &[(&str, &str)] = &[
    ("HINJURY", "HIN"),
    ("HPHYSICAL", "HPH"),
    ("HFEAR", "HFE"),
    ("HFANTASY", "HFA"),
    ("HMIND", "HMI"),
    ("HOTHER", "HOT"),
];

/// Ruby `SHIFTER_ALIAS_REG = /^shifter|UDS/i`（`^` は最初の選択肢にだけ掛かる）。
fn shifter_alias_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^shifter|UDS").expect("valid regex"))
}

/// Ruby `BINDER_ALIAS_REG = /^binder|UDB/i`。
fn binder_alias_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^binder|UDB").expect("valid regex"))
}

/// Ruby `UnsungDuet#roll_replaced_command_if_match`。
///
/// 正規表現に一致したら最初の一致箇所を `dist` に置換（Ruby `String#sub`）して
/// `CommonCommand::AddDice.eval` に渡す。一致しなければ `nil`。
fn roll_replaced_command_if_match(
    command: &str,
    pattern: &Regex,
    dist: &str,
    game_system: &dyn GameSystem,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    if !pattern.is_match(command) {
        return Ok(None);
    }
    let replaced = pattern.replacen(command, 1, dist);
    add_dice::eval(&replaced, game_system, rng)
}

/// Ruby `Base#roll_tables`。
fn roll_tables(
    tables: &'static [(&'static str, &'static Table)],
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    match tables.iter().find(|(key, _)| *key == command) {
        None => Ok(None),
        Some((_, table)) => Ok(Some(table.roll(rng)?.to_string())),
    }
}

/// Ruby `UnsungDuet#eval_game_system_specific_command`。
///
/// `game_system` は `AddDice.eval(…, self, @randomizer)` の `self` に対応する
/// （`sort_add_dice?` や `result_ndx` の定型文をどのシステムから引くかが決まる）。
pub(crate) fn eval_specific_command(
    sys: &SystemTables,
    game_system: &dyn GameSystem,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    // Ruby: command = ALIAS[command] || command
    let command = ALIAS
        .iter()
        .find(|(alias, _)| *alias == command)
        .map_or(command, |(_, key)| *key);

    if let Some(result) =
        roll_replaced_command_if_match(command, shifter_alias_pattern(), "1D10", game_system, rng)?
    {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    if let Some(result) =
        roll_replaced_command_if_match(command, binder_alias_pattern(), "2D6", game_system, rng)?
    {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    Ok(roll_tables(sys.tables, command, rng)?.map(SpecificCommandOutput::text))
}

/// Ruby `Base#result_ndx`（ロケールの定型文で）。
///
/// `shifter>=5` のような判定は `AddDice` 経由でここに来るので、
/// `UnsungDuet_Korean` は `ko_kr` の `성공` / `실패` を返すために上書きする。
pub(crate) fn result_ndx_localized(
    sys: &SystemTables,
    total: crate::Int,
    cmp_op: CmpOp,
    target: Target,
) -> Option<EvalResult> {
    // Ruby: return nil if target.is_a?(String)（目標値 "?"）
    let Target::Number(target) = target else {
        return None;
    };
    if cmp_op.apply(&total, &target) {
        Some(EvalResult::success(sys.success))
    } else {
        Some(EvalResult::failure(sys.failure))
    }
}

/// Ruby `BCDice::GameSystem::UnsungDuet`（ID: `UnsungDuet`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnsungDuet;

impl GameSystem for UnsungDuet {
    fn id(&self) -> &'static str {
        "UnsungDuet"
    }

    fn name(&self) -> &'static str {
        "アンサング・デュエット"
    }

    fn sort_key(&self) -> &'static str {
        "あんさんくてゆえつと"
    }

    fn help_message(&self) -> &'static str {
        r"■ シフター用判定 (shifter, UDS)
  1D10をダイスロールして判定を行います。
  例） shifter, UDS, shifter>=5, shifter+1>=6

■ バインダー用判定 (binder, UDB)
  2D6をダイスロールして判定を行います。
  例） binder, UDB, binder>=5, binder+1>=6

■ 変異表
  ・外傷 (HIN, HInjury)
  ・体調の変化 (HPH, HPhysical)
  ・恐怖 (HFE, HFear)
  ・幻想化 (HFA, HFantasy)
  ・精神 (HMI, HMind)
  ・そのほか (HOT, HOther)
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "shifter",
            "UDS",
            "binder",
            "UDB",
            "HINJURY",
            "HPHYSICAL",
            "HFEAR",
            "HFANTASY",
            "HMIND",
            "HOTHER",
            "HIN",
            "HPH",
            "HFE",
            "HFA",
            "HMI",
            "HOT",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `UnsungDuet#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&JA_SYSTEM, self, command, rng)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "UnsungDuet",
            "UnsungDuet.toml",
            28,
        );
    }
}
