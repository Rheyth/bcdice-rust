//! P4で手書き移植した `lib/bcdice/game_system/Satasupe.rb` と
//! `lib/bcdice/game_system/satasupe/tables.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `checkRoll` / `get_roll_params` / `get_judge_info` / `check_roll_loop`
//!   （判定コマンド `nR>=x[y,z,c]`）
//! - `check_seigou`（性業値コマンド `SRx`）
//! - `rollTableCommand` と、その分岐先 `getTagTableResult` / `getCreateSatasupeResult` /
//!   `getNpcTableResult` / `getAnotherTableResult` / `getTableIndex`
//!
//! # 表データ
//!
//! `satasupe/tables.rb` の `TAG_TABLE` / `CREATE_ARMS_ACCESSORY_TABLE` / `NPC_*_TABLE` /
//! `TABLES` / `ALIASES` を `static` として直接持つ。値は同ファイルから機械的に書き出した
//! もので、1文字も変えていない。`TABLES` と `ALIASES` のキーは Ruby 側の
//! `transform_keys(&:upcase)` に合わせて大文字化済み。
//!
//! # Ruby との差異（意図的なもの）
//!
//! - `checkRoll` の `while target > 12 do target -= 1; fumble += 1 end` は差分を
//!   まとめて足す形にした（結果は同じで、巨大な難易度でも停止する）。
//! - Ruby は多倍長整数なので桁あふれしないが、こちらは `i64` の飽和演算にしている。
//!   直後に上限で丸められる箇所ばかりなので、現実的な入力では差が出ない。

use std::sync::OnceLock;

use regex::Regex;

use crate::command_parser::Parser;
use crate::dice_table::{D66GridTable, RollableTable, Table};
use crate::enums::{D66SortType, RoundType};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

// ---------------------------------------------------------------------------
// ゲームシステム
// ---------------------------------------------------------------------------

/// Ruby `BCDice::GameSystem::Satasupe`（ID: `Satasupe`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Satasupe;

impl GameSystem for Satasupe {
    fn id(&self) -> &'static str {
        "Satasupe"
    }

    fn name(&self) -> &'static str {
        "サタスペ"
    }

    fn sort_key(&self) -> &'static str {
        "さたすへ"
    }

    fn help_message(&self) -> &'static str {
        r"・判定コマンド　(nR>=x[y,z,c] or nR>=x or nR>=[,,c] etc)
　nが最大ロール回数、xが難易度、yが目標成功度、zがファンブル値、cが必殺値。
　y と z と c は省略可能です。(省略時、y＝無制限、z＝1、c=13(なし))
　c の後ろにSを記述すると必殺が出た時点で判定を終了します。
　例）5R>=5[10,2,7S]
・性業値コマンド(SRx or SRx+y or SRx-y x=性業値 y=修正値)
・各種表 ： コマンド末尾に数字を入れると複数回の一括実行が可能　例）TAGT3
　・タグ決定表(TAGT)
　・命中判定ファンブル表(FumbleT)、致命傷表(FatalT)、
　　　乗物致命傷表(FatalVT)
　・ロマンスファンブル表(RomanceFT)
　・アクシデント表(AccidentT)、汎用アクシデント表(GeneralAT)
　・その後表　(AfterT)、臭い飯表(KusaiMT)、登場表(EnterT)、
　　　落とし前表(PayT)、時間切れ表(TimeUT)、バッドトリップ表(BudTT)
　・報酬表(Get〜) ： ガラクタ(GetgT)、実用品(GetzT)、値打ち物(GetnT)、
　　　奇天烈(GetkT)
　・NPCの年齢と好みを一括出力(NPCT)
　・「サタスペ」のベースとアクセサリを出力(GETSSTx　xはアクセサリ数、省略時１)
・以下のコマンドは +,- でダイス目修正、=でダイス目指定が可能
　例）CrimeIET+1　CrimeIET-1　CrimeIET=7
　・情報イベント表(〜IET) ： 犯罪表(CrimeIET)、生活表(LifeIET)、
　　　恋愛表(LoveIET)、教養表(CultureIET)、戦闘表(CombatIET)
　・情報ハプニング表(〜IHT) ： 犯罪表(CrimeIHT)、生活表(LifeIHT)、
　　　恋愛表(LoveIHT)、教養表(CultureIHT)、戦闘表(CombatIHT)
　・遭遇表(～RET)：ミナミ遭遇表(MinamiRET)、中華街遭遇表(ChinatownRET)、
　　　軍艦島遭遇表(WarshipLandRET)、官庁街遭遇表(CivicCenterRET)、
　　　十三遭遇表(DowntownRET)、沙京遭遇表(ShaokinRET)、
　　　らぶらぶ遭遇表(LoveLoveRET)、アジト遭遇表(AjitoRET)、
　　　地獄湯遭遇表(JigokuSpaRET)、JAIL HOUSE遭遇表(JailHouseRET)
　・イベント表(～IT)：治療イベント表(TreatmentIT)、大学イベント表(CollegeIT)
・D66ダイスあり
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            r"\d+R",
            "SR",
            "TAGT",
            "GETSST",
            "NPCT",
            "CRIMEIET",
            "LIFEIET",
            "LOVEIET",
            "CULTUREIET",
            "COMBATIET",
            "CRIMEIHT",
            "LIFEIHT",
            "LOVEIHT",
            "CULTUREIHT",
            "COMBATIHT",
            "GENERALACCIDENTT",
            "ROMANCEFUMBLET",
            "FUMBLET",
            "FATALT",
            "ACCIDENTT",
            "AFTERT",
            "KUSAIMT",
            "ENTERT",
            "BUDTT",
            "GETGT",
            "GETZT",
            "GETNT",
            "GETKT",
            "PAYT",
            "MINAMIRET",
            "CHINATOWNRET",
            "WARSHIPLANDRET",
            "CIVICCENTERRET",
            "DOWNTOWNRET",
            "SHAOKINRET",
            "LOVELOVERET",
            "AJITORET",
            "JIGOKUSPARET",
            "JAILHOUSERET",
            "TREATMENTIT",
            "COLLEGEIT",
            "FATALVT",
            "TIMEUT",
            "RFT",
            "GAT",
            "ROMANCEFT",
            "GENERALAT",
            "RFUMBLET",
            "GACCIDENTT",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `initialize`: `@sort_add_dice = true`
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `initialize`: `@d66_sort_type = D66SortType::ASC`
    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }

    /// Ruby `Satasupe#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: result = checkRoll(command); return result unless result.nil?
        if let Some(result) = check_roll(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }

        // Ruby: result = check_seigou(command); return result unless result.empty?
        let seigou = check_seigou(command, rng)?;
        if !seigou.is_empty() {
            return Ok(Some(SpecificCommandOutput::text(seigou)));
        }

        // Ruby: return rollTableCommand(command)
        // 該当表が無いときは空文字列（Ruby では空配列）になり、
        // `Base#dice_command` がそれを nil に畳む。
        Ok(Some(SpecificCommandOutput::text(roll_table_command(
            command, rng,
        )?)))
    }
}

// ---------------------------------------------------------------------------
// 判定コマンド
// ---------------------------------------------------------------------------

/// Ruby `Satasupe#get_roll_params` が返す4値。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RollParams {
    /// 目標成功度。0は「無制限」。
    min_suc: i64,
    /// ファンブル値
    fumble: i64,
    /// 必殺値。13は「なし」。
    critical: i64,
    /// 必殺が出た時点で判定を打ち切るか（必殺値の後ろの `S`）
    is_critical_stop: bool,
}

/// Ruby `Satasupe#check_roll_loop` が返す4値。
struct RollLoop {
    dice_str: String,
    total_suc: i64,
    is_critical: bool,
    is_fumble: bool,
}

/// Ruby `Satasupe#checkRoll`（判定コマンド `nR>=x[y,z,c]`）。
fn check_roll(string: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    // Ruby: /^(\d+)R>=(\d+)(\[(\d+)?(,|,\d+)?(,\d+(S)?)?\])?$/i
    // `Preprocessor` が最初の空白より前しか残さないので改行は来ない（`(?m)` は付けない）。
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)^(\d+)R>=(\d+)(\[(\d+)?(,|,\d+)?(,\d+(S)?)?\])?$").expect("valid regex")
    });
    let Some(m) = re.captures(string) else {
        return Ok(None);
    };

    let roll_times = ruby_to_i(&m[1]);
    let mut target = ruby_to_i(&m[2]);
    // Ruby: params = m[3]（角カッコを含む部分文字列）
    let mut params = get_roll_params(m.get(3).map(|g| g.as_str()));

    let mut result = String::new();

    if target > 12 {
        result +=
            &format!("【{string}】 ＞ 難易度が12を超えたため、超過分、ファンブル率が上昇！\n");
        // Ruby: while target > 12 do target -= 1; fumble += 1 end
        params.fumble = params.fumble.saturating_add(target - 12);
        target = 12;
    }

    if (params.critical < 1) || (params.critical > 12) {
        params.critical = 13;
    }

    if params.fumble >= 6 {
        result += &format!(
            "{} ＞ ファンブル率が6を超えたため自動失敗！",
            judge_info(target, &params)
        );
        return Ok(Some(EvalResult::failure(result)));
    }

    if target < 5 {
        result +=
            &format!("【{string}】 ＞ あらゆる難易度は5未満にはならないため、難易度は5になる！\n");
        target = 5;
    }

    let roll = check_roll_loop(roll_times, target, &params, rng)?;

    result += &format!(
        "{} ＞ {} ＞ 成功度{}",
        judge_info(target, &params),
        roll.dice_str,
        roll.total_suc
    );

    if roll.is_fumble {
        result += " ＞ ファンブル";
    }

    if roll.is_critical && (roll.total_suc > 0) {
        result += " ＞ 必殺発動可能！";
    }

    // Ruby: Result.new.tap { |r| ... }（success と failure が同時に立つことはない）
    let mut r = EvalResult::with_text(result);
    r.success = !roll.is_fumble && params.min_suc > 0 && roll.total_suc >= params.min_suc;
    r.failure = roll.is_fumble;
    r.critical = roll.is_critical;
    r.fumble = roll.is_fumble;

    Ok(Some(r))
}

/// Ruby `Satasupe#get_roll_params`。`params` は `"[x,y,cS]"` の形。
fn get_roll_params(params: Option<&str>) -> RollParams {
    let mut out = RollParams {
        min_suc: 0,
        fumble: 1,
        critical: 13,
        is_critical_stop: false,
    };

    let Some(params) = params else {
        return out;
    };

    static RE: OnceLock<Regex> = OnceLock::new();
    // Ruby: /\[(\d*)(,(\d*)?)?(,(\d*)(S)?)?\]/
    // 原典に `/i` は無いので `S` は大文字のみ。`Base#dice_command` が入力を大文字化した
    // 後に呼ばれるため、`5R>=5[10,2,7s]` のような小文字入力もここでは `S` になっている。
    let re =
        RE.get_or_init(|| Regex::new(r"\[(\d*)(,(\d*)?)?(,(\d*)(S)?)?\]").expect("valid regex"));
    let Some(m) = re.captures(params) else {
        return out;
    };

    out.min_suc = ruby_to_i(&m[1]);
    // Ruby: fumble = m[3].to_i if m[3].to_i != 0（`nil.to_i` は 0）
    let fumble = m.get(3).map_or(0, |g| ruby_to_i(g.as_str()));
    if fumble != 0 {
        out.fumble = fumble;
    }
    // Ruby: critical = m[5].to_i if m[4]（`[x,y,]` なら m[5] は空文字列＝0）
    if m.get(4).is_some() {
        out.critical = m.get(5).map_or(0, |g| ruby_to_i(g.as_str()));
    }
    out.is_critical_stop = m.get(6).is_some();

    out
}

/// Ruby `Satasupe#get_judge_info`。
fn judge_info(target: i64, params: &RollParams) -> String {
    let critical = if params.critical == 13 {
        "なし".to_owned()
    } else {
        params.critical.to_string()
    };
    format!(
        "【難易度{target}、ファンブル率{}、必殺{critical}】",
        params.fumble
    )
}

/// Ruby `Satasupe#check_roll_loop`。
fn check_roll_loop(
    roll_times: i64,
    target: i64,
    params: &RollParams,
    rng: &mut Randomizer,
) -> Result<RollLoop, EvalError> {
    let mut out = RollLoop {
        dice_str: String::new(),
        total_suc: 0,
        is_critical: false,
        is_fumble: false,
    };

    for _ in 0..roll_times {
        if params.min_suc != 0 && (out.total_suc >= params.min_suc) {
            break;
        }

        let d1 = rng.roll_once(6)?;
        let d2 = rng.roll_once(6)?;

        // Ruby: dice_suc = 0; dice_suc = 1 if target <= (d1 + d2)
        let dice_suc = i64::from(target <= d1 + d2);
        if !out.dice_str.is_empty() {
            out.dice_str.push('+');
        }
        out.dice_str += &format!("{dice_suc}[{d1},{d2}]");
        out.total_suc += dice_suc;

        if params.critical <= d1 + d2 {
            out.is_critical = true;
            out.dice_str += "『必殺！』";
        }

        // ファンブルの確認
        if (d1 == d2) && (d1 <= params.fumble) {
            out.is_fumble = true;
            out.is_critical = false;
            break;
        }

        // 必殺止めの確認
        if out.is_critical && params.is_critical_stop {
            break;
        }
    }

    Ok(out)
}

// ---------------------------------------------------------------------------
// 性業値コマンド
// ---------------------------------------------------------------------------

/// Ruby `Satasupe#check_seigou`（性業値コマンド `SRx`）。
///
/// 該当しない場合は Ruby と同じく空文字列を返す（呼び出し元が `empty?` で見る）。
fn check_seigou(string: &str, rng: &mut Randomizer) -> Result<String, EvalError> {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    // Ruby: Command::Parser.new("SR", round_type: round_type)
    //           .has_suffix_number.restrict_cmp_op_to(nil)
    //       `round_type` は Base の既定（:floor）のまま。
    let parser = PARSER.get_or_init(|| {
        Parser::new(&["SR"], RoundType::Floor)
            .has_suffix_number()
            .restrict_cmp_op_to(&[None])
    });
    let Some(cmd) = parser.parse(string) else {
        return Ok(String::new());
    };

    let dice = rng.roll_sum(2, 6)?;
    let dice_total = dice + cmd.modify_number.clone();
    // `has_suffix_number` なのでパース成功時は必ず入っている。
    let target = cmd
        .suffix_number
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0);

    let seigou = if target < crate::randomizer::sat_i64(&dice_total) {
        "「激」"
    } else if target > crate::randomizer::sat_i64(&dice_total) {
        "「律」"
    } else {
        // target == dice_total
        "「迷」"
    };

    let modify = cmd.modify_number;
    let mut result = format!(
        "〔性業値〕{target}、「修正値」{modify} ＞ ダイス結果：（{dice}） ＞ {dice}＋（{modify}）＝{dice_total} ＞ {seigou}"
    );

    if dice == 2 {
        result += " ＞ 1ゾロのため〔性業値〕が1点上昇！";
    }
    if dice == 12 {
        result += " ＞ 6ゾロのため〔性業値〕が1点減少！";
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// 各種表
// ---------------------------------------------------------------------------

/// Ruby `Satasupe#rollTableCommand`。
fn roll_table_command(command: &str, rng: &mut Randomizer) -> Result<String, EvalError> {
    // Ruby: command = command.upcase（`Base#dice_command` で大文字化済みだが原典どおり）
    let command = command.to_uppercase();

    static RE: OnceLock<Regex> = OnceLock::new();
    // Ruby: /([A-Za-z]+)(\d+)?(([+]|-|=)(\d+))?/（アンカー無し）
    let re =
        RE.get_or_init(|| Regex::new(r"([A-Za-z]+)(\d+)?(([+]|-|=)(\d+))?").expect("valid regex"));
    let Some(m) = re.captures(&command) else {
        return Ok(String::new());
    };

    let name = &m[1];
    // Ruby: counts = 1; counts = m[2].to_i if m[2]
    let counts = m.get(2).map_or(1, |g| ruby_to_i(g.as_str()));
    let operator = m.get(4).map(|g| g.as_str());
    let value = m.get(5).map_or(0, |g| ruby_to_i(g.as_str()));

    let result = match name {
        "TAGT" => tag_table_result(counts, rng)?,
        "GETSST" => create_satasupe_result(counts, rng)?,
        "NPCT" => npc_table_result(counts, rng)?,
        _ => another_table_result(name, counts, operator, value, rng)?,
    };

    Ok(result.join("\n"))
}

/// Ruby `Satasupe#getTagTableResult`（タグ決定表 `TAGT`）。
fn tag_table_result(counts: i64, rng: &mut Randomizer) -> Result<Vec<String>, EvalError> {
    let mut result = Vec::new();

    for _ in 0..counts {
        let roll_result = TAG_TABLE.roll(rng)?;
        result.push(format!(
            "{}:{}:{}",
            roll_result.table_name(),
            roll_result.value(),
            roll_result.body()
        ));
    }

    Ok(result)
}

/// Ruby `Satasupe#getNpcTableResult`（NPC表 `NPCT`）。
fn npc_table_result(counts: i64, rng: &mut Randomizer) -> Result<Vec<String>, EvalError> {
    let name = "NPC表:";
    let mut result = Vec::new();

    for _ in 0..counts {
        // Ruby: age, agen_const, agen_times = NPC_AGE_TABLE[@randomizer.roll_index(6)]
        let index = rng.roll_index(6)?;
        let (age, agen_const, agen_times) = *pick(NPC_AGE_TABLE, index)?;
        let ysold = rng.roll_sum(agen_times, 6)? + agen_const;

        let index = rng.roll_index(6)?;
        let lmod_value = pick(NPC_LMOOD_TABLE, index)?;
        let index = rng.roll_index(3)?;
        let lage_value = pick(NPC_LAGE_TABLE, index)?;

        result.push(format!("{name}{age}({ysold}歳):{lmod_value}{lage_value}"));
    }

    Ok(result)
}

/// Ruby `Satasupe#getCreateSatasupeResult`（「サタスペ」作成 `GETSSTx`）。
fn create_satasupe_result(counts: i64, rng: &mut Randomizer) -> Result<Vec<String>, EvalError> {
    let name = "サタスペ作成";

    // Ruby: case @randomizer.roll_once(6) ... CREATE_ARMS_STRUCT.new(...)
    //       1〜6以外だと Ruby は nil になって直後に NoMethodError で落ちる。
    let index = rng.roll_once(6)? - 1;
    let base = pick(CREATE_ARMS_BASE, index)?;
    let mut arm = Arm {
        base_parts: base.base_parts,
        accessory_parts: Vec::new(),
        parts_effect: vec![base.parts_effect],
        hit: base.hit,
        damage: base.damage,
        life: base.life,
        kutibeni: 0,
        kiba: 0,
        abilities: base.abilities.to_vec(),
    };

    for _ in 0..counts {
        // Ruby: part, effect, modifier = CREATE_ARMS_ACCESSORY_TABLE[roll_d66(D66SortType::ASC)]
        let key = rng.roll_d66(D66SortType::Asc)?;
        let accessory = CREATE_ARMS_ACCESSORY_TABLE
            .iter()
            .find(|a| a.key == key)
            // 昇順D66は必ず表に載っているので到達しない。
            .ok_or(EvalError::Internal("Satasupe: unknown accessory D66"))?;
        arm.accessory_parts.push(accessory.part);
        arm.parts_effect.push(accessory.effect);
        accessory.modifier.apply(&mut arm, rng)?;
    }

    let mut result = vec![
        format!(
            "{name}：ベース部品：{}  アクセサリ部品：{}",
            arm.base_parts,
            arm.accessory_parts.concat()
        ),
        format!("部品効果：{}", arm.parts_effect.concat()),
    ];

    let mut text = format!(
        "完成品：サタスペ  （ダメージ＋{}・命中{}・射撃、",
        arm.damage, arm.hit
    );
    if arm.kutibeni > 0 {
        text += &format!("「（判定前宣言）{}回だけ、必殺10」", arm.kutibeni);
    }
    if arm.kiba > 0 {
        text += &format!("「（判定前宣言）{}回だけ、ダメージ＋２」", arm.kiba);
    }

    // Ruby: arm.abilities.sort.uniq.join
    // Ruby の `String#<=>` はバイト比較なので Rust の `Ord` と同じ順序になる。
    let mut abilities = arm.abilities.clone();
    abilities.sort_unstable();
    abilities.dedup();
    text += &abilities.concat();

    text += &format!("「サタスペ{counts}」「耐久度{}」）", arm.life);

    result.push(text);

    Ok(result)
}

/// Ruby `Satasupe#getAnotherTableResult`（`TABLES` / `ALIASES` 経由の各種表）。
fn another_table_result(
    command: &str,
    counts: i64,
    operator: Option<&str>,
    value: i64,
    rng: &mut Randomizer,
) -> Result<Vec<String>, EvalError> {
    let mut result = Vec::new();

    // Ruby: table_name = ALIASES[command] || command; table = TABLES[table_name]
    let table_name = ALIASES
        .iter()
        .find(|(key, _)| *key == command)
        .map_or(command, |(_, target)| *target);
    let Some((_, table)) = TABLES.iter().find(|(key, _)| *key == table_name) else {
        return Ok(result);
    };

    for _ in 0..counts {
        // Ruby: getTableIndex(operator, value, 2, 6)
        //       表の種別によらず 2D6 固定（`TABLES` は全38表とも 2D6）。
        let index = table_index(operator, value, 2, 6, rng)?;

        let info = table.choice(index);
        result.push(format!(
            "{}:{}:{}",
            info.table_name(),
            info.value(),
            info.body()
        ));
    }

    Ok(result)
}

/// Ruby `Satasupe#getTableIndex`。
///
/// Ruby は `[modify, index]` を返すが、呼び出し元は `index` しか使わない。
fn table_index(
    operator: Option<&str>,
    value: i64,
    dice_count: i64,
    dice_type: i64,
    rng: &mut Randomizer,
) -> Result<i64, EvalError> {
    let mut index = None;
    let mut modify = 0;

    match operator {
        Some("+") => modify = value,
        Some("-") => modify = -value,
        Some("=") => index = Some(value),
        _ => {}
    }

    let mut index = match index {
        Some(index) => index,
        None => rng.roll_sum(dice_count, dice_type)?.saturating_add(modify),
    };

    index = index.max(dice_count);
    index = index.min(dice_count * dice_type);

    Ok(index)
}

/// Ruby の `TABLE[roll_index(n)]`。
///
/// 添字が範囲外だと Ruby は `nil` を返し、直後の演算で例外になって落ちる。
/// 移植では [`EvalError::Internal`] にする（`roll_index` が正常なら到達しない）。
fn pick<T>(table: &'static [T], index: i64) -> Result<&'static T, EvalError> {
    usize::try_from(index)
        .ok()
        .and_then(|i| table.get(i))
        .ok_or(EvalError::Internal("Satasupe: table index out of range"))
}

/// Ruby `String#to_i`（先頭の十進数だけを読み、無ければ 0）。
///
/// ここに渡るのは `\d*` / `\d+` に一致した部分文字列なので符号や空白は現れない。
/// 桁あふれは Ruby だと多倍長整数になる。`i64` に収まらない場合は飽和させる。
fn ruby_to_i(s: &str) -> i64 {
    let digits: String = s.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        // Ruby: "".to_i == 0
        return 0;
    }
    digits.parse().unwrap_or(i64::MAX)
}

// ---------------------------------------------------------------------------
// 「サタスペ」作成用のデータ構造
// ---------------------------------------------------------------------------

/// Ruby `CREATE_ARMS_STRUCT`。
struct Arm {
    base_parts: &'static str,
    accessory_parts: Vec<&'static str>,
    parts_effect: Vec<&'static str>,
    hit: i64,
    damage: i64,
    life: i64,
    kutibeni: i64,
    kiba: i64,
    abilities: Vec<&'static str>,
}

/// Ruby `getCreateSatasupeResult` の `case @randomizer.roll_once(6)` 1件分。
struct BaseArm {
    base_parts: &'static str,
    /// `CREATE_ARMS_STRUCT.new` に渡される `parts_effect` の初期要素
    parts_effect: &'static str,
    hit: i64,
    damage: i64,
    life: i64,
    abilities: &'static [&'static str],
}

/// Ruby `CREATE_ARMS_ACCESSORY_TABLE` の値（第3要素の lambda）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccessoryModifier {
    /// `arm.abilities << "…"`
    Ability(&'static str),
    /// `arm.kutibeni += 1`
    Kutibeni,
    /// `arm.damage += 1`
    Damage,
    /// `arm.life += 1`
    Life,
    /// `arm.hit -= 1`
    Hit,
    /// `arm.kiba = r.roll_once(6)`
    Kiba,
}

impl AccessoryModifier {
    /// Ruby `modifier.call(arm, @randomizer)`。
    fn apply(self, arm: &mut Arm, rng: &mut Randomizer) -> Result<(), EvalError> {
        match self {
            Self::Ability(name) => arm.abilities.push(name),
            Self::Kutibeni => arm.kutibeni += 1,
            Self::Damage => arm.damage += 1,
            Self::Life => arm.life += 1,
            Self::Hit => arm.hit -= 1,
            Self::Kiba => arm.kiba = rng.roll_once(6)?,
        }
        Ok(())
    }
}

/// Ruby `CREATE_ARMS_ACCESSORY_TABLE` の1件（キーと `[部品, 効果, lambda]`）。
struct Accessory {
    key: i64,
    part: &'static str,
    effect: &'static str,
    modifier: AccessoryModifier,
}

// ---------------------------------------------------------------------------
// 表データ（lib/bcdice/game_system/satasupe/tables.rb から機械的に書き出したもの）
// ---------------------------------------------------------------------------

/// Ruby `TAG_TABLE` の項目（6行×6列）。
static TAG_TABLE_ROW_1: &[&str] = &[
    "情報イベント",
    "エクストリーム(サ)",
    "カワイイ(サ)",
    "トンデモ(サ)",
    "マニア(サ)",
    "ヲタク(サ)",
];
static TAG_TABLE_ROW_2: &[&str] = &[
    "音楽(ア)",
    "好きなタグ",
    "トレンド(ア)",
    "読書(ア)",
    "パフォーマンス(ア)",
    "美術(ア)",
];
static TAG_TABLE_ROW_3: &[&str] = &[
    "アラサガシ(マ)",
    "おせっかい(マ)",
    "好きなタグ",
    "家事(マ)",
    "ガリ勉(マ)",
    "健康(マ)",
];
static TAG_TABLE_ROW_4: &[&str] = &[
    "アウトドア(休)",
    "工作(休)",
    "スポーツ(休)",
    "同一タグ",
    "ハイソ(休)",
    "旅行(休)",
];
static TAG_TABLE_ROW_5: &[&str] = &[
    "育成(イ)",
    "サビシガリヤ(イ)",
    "ヒマツブシ(イ)",
    "宗教(イ)",
    "同一タグ",
    "ワビサビ(イ)",
];
static TAG_TABLE_ROW_6: &[&str] = &[
    "アダルト(風)",
    "飲食(風)",
    "ギャンブル(風)",
    "ゴシップ(風)",
    "ファッション(風)",
    "情報ハプニング",
];

static TAG_TABLE_ITEMS: &[&[&str]] = &[
    TAG_TABLE_ROW_1,
    TAG_TABLE_ROW_2,
    TAG_TABLE_ROW_3,
    TAG_TABLE_ROW_4,
    TAG_TABLE_ROW_5,
    TAG_TABLE_ROW_6,
];

/// Ruby `TAG_TABLE`（タグ決定表、D66）。
static TAG_TABLE: D66GridTable = D66GridTable::new("タグ決定表", TAG_TABLE_ITEMS);

/// Ruby `getCreateSatasupeResult` の `case @randomizer.roll_once(6)`（出目1〜6の順）。
static CREATE_ARMS_BASE: &[BaseArm] = &[
    BaseArm {
        base_parts: "「紙製の筒」",
        parts_effect: "「命中：10、ダメージ：3、耐久度1」",
        hit: 10,
        damage: 3,
        life: 1,
        abilities: &[],
    },
    BaseArm {
        base_parts: "「木製の筒」",
        parts_effect: "「命中：9、ダメージ：3、耐久度2」",
        hit: 9,
        damage: 3,
        life: 2,
        abilities: &[],
    },
    BaseArm {
        base_parts: "「小型のプラスチック製の筒」",
        parts_effect: "「命中：9、ダメージ：4、耐久度2」",
        hit: 9,
        damage: 4,
        life: 2,
        abilities: &[],
    },
    BaseArm {
        base_parts: "「大型のプラスチック製の筒」",
        parts_effect: "「命中：8、ダメージ：3、耐久度2、両手」",
        hit: 8,
        damage: 3,
        life: 2,
        abilities: &["「両手」"],
    },
    BaseArm {
        base_parts: "「小型の金属製の筒」",
        parts_effect: "「命中：9、ダメージ：4、耐久度3」",
        hit: 9,
        damage: 4,
        life: 3,
        abilities: &[],
    },
    BaseArm {
        base_parts: "「大型の金属製の筒」",
        parts_effect: "「命中：8、ダメージ：5、耐久度3、両手」",
        hit: 8,
        damage: 5,
        life: 3,
        abilities: &["「両手」"],
    },
];

/// Ruby `CREATE_ARMS_ACCESSORY_TABLE`（昇順D66で引く）。
static CREATE_ARMS_ACCESSORY_TABLE: &[Accessory] = &[
    Accessory {
        key: 11,
        part: "「パチンコ玉」",
        effect: "「武器破壊」",
        modifier: AccessoryModifier::Ability("「武器破壊」"),
    },
    Accessory {
        key: 12,
        part: "「釘や画鋲、針」",
        effect: "「毒」",
        modifier: AccessoryModifier::Ability("「毒」"),
    },
    Accessory {
        key: 13,
        part: "「砂利や小石、ガラスの破片」",
        effect: "「散弾」",
        modifier: AccessoryModifier::Ability("「散弾」"),
    },
    Accessory {
        key: 14,
        part: "「口紅」",
        effect: "「（判定前宣言）一度だけ必殺10」",
        modifier: AccessoryModifier::Kutibeni,
    },
    Accessory {
        key: 15,
        part: "「バネやゼンマイ」",
        effect: "「フル」",
        modifier: AccessoryModifier::Ability("「フル」"),
    },
    Accessory {
        key: 16,
        part: "「捻子やビス」",
        effect: "「ダメージ＋１」",
        modifier: AccessoryModifier::Damage,
    },
    Accessory {
        key: 22,
        part: "「生ゴミ」",
        effect: "「衝撃」",
        modifier: AccessoryModifier::Ability("「衝撃」"),
    },
    Accessory {
        key: 23,
        part: "「ゴム」",
        effect: "「ダメージ＋１」",
        modifier: AccessoryModifier::Damage,
    },
    Accessory {
        key: 24,
        part: "「歯車」",
        effect: "「リボルバー」",
        modifier: AccessoryModifier::Ability("「リボルバー」"),
    },
    Accessory {
        key: 25,
        part: "「歯や牙、骨」",
        effect: "「（判定前宣言）1D6回、ダメージ＋２」",
        modifier: AccessoryModifier::Kiba,
    },
    Accessory {
        key: 26,
        part: "「ワイヤー」",
        effect: "「耐久度＋１」",
        modifier: AccessoryModifier::Life,
    },
    Accessory {
        key: 33,
        part: "「メガネなどのレンズ」",
        effect: "「命中－１」",
        modifier: AccessoryModifier::Hit,
    },
    Accessory {
        key: 34,
        part: "「マッチ」",
        effect: "「必殺12」",
        modifier: AccessoryModifier::Ability("「必殺12」"),
    },
    Accessory {
        key: 35,
        part: "「ガムテープや接着剤」",
        effect: "「耐久度＋１」",
        modifier: AccessoryModifier::Life,
    },
    Accessory {
        key: 36,
        part: "「洗濯ばさみ」",
        effect: "「命中－１」",
        modifier: AccessoryModifier::Hit,
    },
    Accessory {
        key: 44,
        part: "「花火」",
        effect: "「弾幕1」",
        modifier: AccessoryModifier::Ability("「弾幕1」"),
    },
    Accessory {
        key: 45,
        part: "「食玩」",
        effect: "「暗器」",
        modifier: AccessoryModifier::Ability("「暗器」"),
    },
    Accessory {
        key: 46,
        part: "「真空管やトランジスタ」",
        effect: "「神秘」",
        modifier: AccessoryModifier::Ability("「神秘」"),
    },
    Accessory {
        key: 55,
        part: "「エアコンプレッサ」",
        effect: "「ダメージ＋１」",
        modifier: AccessoryModifier::Damage,
    },
    Accessory {
        key: 56,
        part: "「豆」",
        effect: "「マヒ」",
        modifier: AccessoryModifier::Ability("「マヒ」"),
    },
    Accessory {
        key: 66,
        part: "「ガスボンベや殺虫剤」",
        effect: "「爆発3」",
        modifier: AccessoryModifier::Ability("「爆発3」"),
    },
];

/// Ruby `NPC_AGE_TABLE`（年齢表）。`(区分, 定数, ダイス個数)` で `定数+nD6` 歳。
static NPC_AGE_TABLE: &[(&str, i64, i64)] = &[
    ("幼年", 6, 2),
    ("少年", 10, 2),
    ("青年", 15, 3),
    ("中年", 25, 4),
    ("壮年", 40, 5),
    ("老年", 60, 6),
];

/// Ruby `NPC_LMOOD_TABLE`（好み／雰囲気表）。
static NPC_LMOOD_TABLE: &[&str] = &[
    "ダークな",
    "お金持ちな",
    "美形な",
    "知的な",
    "ワイルドな",
    "バランスがとれてる",
];

/// Ruby `NPC_LAGE_TABLE`（好み／年齢表）。
static NPC_LAGE_TABLE: &[&str] = &["年下が好き。", "同い年が好き。", "年上が好き。"];

/// Ruby `TABLES["CRIMEIET"]`（情報イベント表／〔犯罪〕）の項目。
static TBL_CRIMEIET_ITEMS: &[&str] = &[
    "謎の情報屋チュンさん登場。ターゲットとなる情報を渡し、いずこかへ去る。情報ゲット！",
    "昔やった仕事の依頼人が登場。てがかりをくれる。好きなタグの上位リンク（SL+2）を１つ得る。",
    "謎のメモを発見……このターゲットについて調べている間、このトピックのタグをチーム全員が所有しているものとして扱う",
    "謎の動物が亜侠を路地裏に誘う。好きなタグの上位リンクを２つ得る",
    "偶然、他の亜侠の仕事現場に出くわす。口止め料の代わりに好きなタグの上位リンクを１つ得る",
    "あまりに適切な諜報活動。コストを消費せず、上位リンクを３つ得る",
    "その道の権威を紹介される。現在と同じタグの上位リンクを２つ得る",
    "捜査は足だね。〔肉体点〕を好きなだけ消費する。その値と同じ数の好きなタグの上位リンクを得る",
    "近所のコンビニで立ち読み。思わぬ情報が手に入る。上位リンクを３つ得る",
    "そのエリアの支配盟約からメッセンジャーが1D6人。自分のチームがその盟約に敵対していなければ、好きなタグの上位リンクを２つ得る。敵対していれば、メッセンジャーは「盟約戦闘員（p.127）」となる。血戦を行え",
    "「三下（p.125）」が1D6人現れる。血戦を行え。倒した数だけ、好きなタグの上位リンクを手に入れる",
];

/// Ruby `TABLES["CRIMEIET"]`（`2D6`）。
static TBL_CRIMEIET: Table = Table::from_dice("情報イベント表／〔犯罪〕", 2, 6, TBL_CRIMEIET_ITEMS);

/// Ruby `TABLES["LIFEIET"]`（情報イベント表／〔生活〕）の項目。
static TBL_LIFEIET_ITEMS: &[&str] = &[
    "謎の情報屋チュンさん登場。ターゲットとなる情報を渡し、いずこかへ去る。情報ゲット！",
    "隣の奥さんと世間話。上位リンクを４つ得る",
    "ミナミで接待。次の１ターン何もできない代わりに、好きなタグの上位リンク（SL+2）を１つ得る",
    "息抜きにテレビを見ていたら、たまたまその情報が。好きなタグの上位リンクを１つ得る",
    "器用に手に入れた情報を転売する。《札巻》を１個手に入れ、上位リンクを３つ得る",
    "情報を得るついでに軽い営業。〔サイフ〕を１回復させ、上位リンクを３つ得る",
    "街の有力者からの突然の電話。そのエリアの盟約の幹部NPCの誰かと【コネ】を結ぶことができる",
    "金をばらまく。〔サイフ〕を好きなだけ消費する。その値と同じ数の任意の上位リンクを得る",
    "〔表の顔〕の同僚が思いがけないアドバイスをくれる。上位リンクを1D6つ得る",
    "謎の情報屋チュンさんが、情報とアイテムのトレードを申し出る。DDの指定するアイテムを１つ手に入れると、どこからともなくチュンさんが現れる。そのアイテムをチュンさんに渡せば、情報ゲット！",
    "ターゲットとは関係ないが、ドデかい情報を掘り当てる。その情報を売って〔サイフ〕が全快する",
];

/// Ruby `TABLES["LIFEIET"]`（`2D6`）。
static TBL_LIFEIET: Table = Table::from_dice("情報イベント表／〔生活〕", 2, 6, TBL_LIFEIET_ITEMS);

/// Ruby `TABLES["LOVEIET"]`（情報イベント表／〔恋愛〕）の項目。
static TBL_LOVEIET_ITEMS: &[&str] = &[
    "謎の情報屋チュンさん登場。ターゲットとなる情報を渡し、いずこかへ去る。情報ゲット！",
    "恋人との別れ。自分に恋人がいれば、１人を選んで、お互いのトリコ欄から名前を消す。その代わり情報ゲット！",
    "とびきり美形の情報提供者と遭遇。〔性業値〕判定で律になると、好きなタグの上位リンクを１つ得る",
    "敵対する亜侠と第一種接近遭遇。キスのあとの濡れた唇から、上位リンクを３つ得る",
    "昔の恋人がそれに詳しかったはず。その日の深夜・早朝に行動しなければ、好きなタグの上位リンク（SL+2）を１つ得る",
    "情報はともかくトリコをゲット。データは「女子高生（p.122）」を使用する",
    "関係者とすてきな時間を過ごす。好きなタグの上位リンクを１つ得る。ただし、次の１ターンは行動できない",
    "持つべきものは愛の奴隷。自分のトリコの数だけ好きなタグの上位リンクを得る",
    "自分よりも１０歳年上のイヤなやつに身体を売る。現在と同じタグの上位リンクを１つ得る",
    "有力者からの突然のご指名。チームの仲間を１人、ランダムに決定する。差し出すなら、そのキャラクターは次の１ターン行動できない代わり、その後にそのキャラクターの〔恋愛〕と同じ数の上位リンクを得る",
    "愛する人の死。自分に恋人がいれば、１人選んで、そのキャラクターを死亡させる。その代わり情報ゲット！",
];

/// Ruby `TABLES["LOVEIET"]`（`2D6`）。
static TBL_LOVEIET: Table = Table::from_dice("情報イベント表／〔恋愛〕", 2, 6, TBL_LOVEIET_ITEMS);

/// Ruby `TABLES["CULTUREIET"]`（情報イベント表／〔教養〕）の項目。
static TBL_CULTUREIET_ITEMS: &[&str] = &[
    "謎の情報屋チュンさん登場。ターゲットとなる情報を渡し、いずこかへ去る。情報ゲット！",
    "ネットで幻のリンクサイトを発見。すべての種類のタグに上位リンクがはられる",
    "間違いメールから恋が始まる。ハンドルしか知らない「女子高生（p.122）」と恋人（お互いのトリコ）の関係になる",
    "新聞社でバックナンバーを読みふける。上位リンクを６つ得る",
    "巨大な掲示板群から必要な情報をサルベージ。好きなタグの上位リンクを１つ得る",
    "検索エンジンにかけたらすぐヒット。コストを消費せず、上位リンクを４つ得る",
    "警察無線を傍受。興味深い。好きなタグの上位リンクを２つ得る",
    "クールな推理がさえ渡る。〔精神点〕を好きなだけ消費する。その値と同じ数だけ好きなタグの上位リンクを得る",
    "図書館ロールが貫通。好きなタグの上位リンク（SL+3)を１つ得る",
    "図書館で幻の書物を発見。上位リンクを８つ得る。キャラクターシートのメモ欄に<クトゥルフ神話知識>、SANと記入し、それぞれ後ろに＋５、－５の数値を書き加える",
    "アジトに謎の手紙が届く。自分のアジトに戻れば、情報ゲット！",
];

/// Ruby `TABLES["CULTUREIET"]`（`2D6`）。
static TBL_CULTUREIET: Table =
    Table::from_dice("情報イベント表／〔教養〕", 2, 6, TBL_CULTUREIET_ITEMS);

/// Ruby `TABLES["COMBATIET"]`（情報イベント表／〔戦闘〕）の項目。
static TBL_COMBATIET_ITEMS: &[&str] = &[
    "謎の情報屋チュンさん登場。ターゲットとなる情報を渡し、いずこかへ去る。情報ゲット！",
    "昔、お前が『更正』させた大幇のチンピラから情報を得る。〔精神点〕を２点減少し、好きなタグの上位リンク（SL+2）を１つ得る。",
    "大阪市警の刑事から情報リーク。「敵の敵は味方」ということか……？　〔精神点〕を３点減少し、上位リンクを６つ得る。",
    "無軌道な若者達を拳で『更正』させる。彼等は涙を流しながら情報を差し出した。……情けは人のためならず。好きなだけ〔精神点〕を減少する。減少した値と同じ数だけ、上位リンクを得る。",
    "クスリ漬けの流氓を拳で『説得』。流氓はゲロと一緒に情報を吐き出した。２点のダメージ（セーブ不可）を受け、好きなタグの上位リンクを１つ得る。",
    "次から次へと糞どもがやってくる。コストを消費せずに上位リンクを３つ得る。",
    "自称『善良な一市民』からの情報リークを受ける。オマエの持っている異能の数だけ上位リンクを得る。……罠か！？",
    "サウナ風呂でくつろぐヤクザから情報収集。ヤクザは歯の折れた口から、弱々しい呻きと共に情報を吐き出した。好きなだけダメージを受ける（セーブ不可）。好きなタグの受けたダメージと同じ値のSLへリンクを１つ得る。",
    "ゼロ・トレランスオンスロートなラブ＆ウォー。2D6を振り、その値が現在の〔肉体点〕以上であれば、情報をゲット！",
    "お前達を狙う刺客が冥土の土産に教えてくれる。お前自身かチームの仲間、お前の恋人のいずれかの〔肉体点〕を０点にすれば、情報をゲットできる。",
    "お前の宿敵（データはブラックアドレス）が1D6体現れる。血戦によって相手を倒せば、情報ゲット。",
];

/// Ruby `TABLES["COMBATIET"]`（`2D6`）。
static TBL_COMBATIET: Table =
    Table::from_dice("情報イベント表／〔戦闘〕", 2, 6, TBL_COMBATIET_ITEMS);

/// Ruby `TABLES["CRIMEIHT"]`（情報ハプニング表／〔犯罪〕）の項目。
static TBL_CRIMEIHT_ITEMS: &[&str] = &[
    "謎の情報屋チュンさん登場。ターゲットとなる情報を渡し、いずこかへ去る。情報ゲット！",
    "警官からの職務質問。一晩拘留される。臭い飯表（p.70）を１回振ること",
    "だますつもりがだまされる。〔サイフ〕を１点消費",
    "気のゆるみによる駐車違反。持っている乗物が無くなってしまう",
    "超えてはならない一線を越える。トラウマを１点受ける",
    "そのトピックを取りしきる盟約に目をつけられる。このトピックと同じタグのトピックからはリンクをはれなくなる",
    "過去の亡霊がきみを襲う。自分の修得している異能の中から好きな１つを選ぶ。このセッションでは、その異能が使用不可になる",
    "敵対する盟約のいざこざに巻き込まれる。〔肉体点〕に1D6点のセーブ不可なダメージを受ける",
    "スリにあう。〔通常装備〕からランダムにアイテムを１個選び、それを無くす",
    "敵対する盟約からの妨害工作。この情報は情報収集のルールを使って手に入れることはできなくなる",
    "頼れる協力者のもとへ行くと、彼（彼女）の無惨な姿が……自分の持っている現在のセッションに参加していないキャラクター１体を選び、〔肉体点〕を０にする。そして、致命傷表(p.61）を振ること",
];

/// Ruby `TABLES["CRIMEIHT"]`（`2D6`）。
static TBL_CRIMEIHT: Table =
    Table::from_dice("情報ハプニング表／〔犯罪〕", 2, 6, TBL_CRIMEIHT_ITEMS);

/// Ruby `TABLES["LIFEIHT"]`（情報ハプニング表／〔生活〕）の項目。
static TBL_LIFEIHT_ITEMS: &[&str] = &[
    "謎の情報屋チュンさん登場。ターゲットとなる情報を渡し、いずこかへ去る。情報ゲット！",
    "経理の整理に没頭。この日の行動をすべてそれに費やさない限り、このセッションでは買物を行えなくなる",
    "壮大なる無駄使い。〔サイフ〕を１点消費",
    "「当たり屋(p.124）」が【追跡】を開始",
    "留守の間に空き巣が！　〔アジト装備〕からランダムにアイテムが１個無くなる",
    "「押し売り(p.124）」が【追跡】を開始",
    "新たな風を感じる。自分の好きな〔趣味〕１つをランダムに変更すること",
    "貧乏ひまなし。［1D6－自分の〔生活〕］ターンの間、行動できなくなる",
    "留守の間にアジトが火事に！　〔アジト装備〕がすべて無くなる。明日からどうしよう？",
    "頼りにしていた有力者が失脚する。しわ寄せがこっちにもきて、〔生活〕が１点減少する",
    "覚えのない借金の返済を迫られる。〔サイフ〕を1D6点減らす。〔サイフ〕が足りない場合、そのセッション終了時までに不足分を支払わないと【借金大王】(p.119）の代償を得る",
];

/// Ruby `TABLES["LIFEIHT"]`（`2D6`）。
static TBL_LIFEIHT: Table = Table::from_dice("情報ハプニング表／〔生活〕", 2, 6, TBL_LIFEIHT_ITEMS);

/// Ruby `TABLES["LOVEIHT"]`（情報ハプニング表／〔恋愛〕）の項目。
static TBL_LOVEIHT_ITEMS: &[&str] = &[
    "謎の情報屋チュンさん登場。ターゲットとなる情報を渡し、いずこかへ去る。情報ゲット！",
    "一晩を楽しむが相手はちょっと特殊な趣味だった。アブノーマルの趣味を持っていない限り、トラウマを１点受ける。この日はもう行動できない",
    "一晩を楽しむが相手はちょっと特殊な趣味だった。【両刀使い】の異能を持っていない限り、トラウマを１点受ける。この日はもう行動できない",
    "一晩を楽しむが相手は年齢を10偽っていた。ロマンス判定のファンブル表を振ること",
    "すてきな人を見かけ、一目惚れ。DDが選んだNPC１体のトリコになる",
    "「痴漢・痴女(p.124）」が【追跡】を開始",
    "手を出した相手が有力者の女（ヒモ）だった。手下どもに袋叩きに会い、1D6点のダメージを受ける（セーブ不可）",
    "突然の別れ。トリコ欄からランダムに１体を選び、その名前を消す",
    "乱れた性生活に疲れる。〔肉体点〕と〔精神点〕がともに２点減少する",
    "性病が伝染る。１日以内に病院に行き、治療（価格４）を行わないと、鼻がもげる。鼻がもげると〔恋愛〕が１点減少する",
    "生命の誕生。子供ができる",
];

/// Ruby `TABLES["LOVEIHT"]`（`2D6`）。
static TBL_LOVEIHT: Table = Table::from_dice("情報ハプニング表／〔恋愛〕", 2, 6, TBL_LOVEIHT_ITEMS);

/// Ruby `TABLES["CULTUREIHT"]`（情報ハプニング表／〔教養〕）の項目。
static TBL_CULTUREIHT_ITEMS: &[&str] = &[
    "謎の情報屋チュンさん登場。ターゲットとなる情報を渡し、いずこかへ去る。情報ゲット！",
    "アヤシイ書物を読み、一時的発狂。この日はもう行動できない。トラウマを１点受ける",
    "天才ゆえの憂鬱。自分の〔教養〕と同じ値だけ、〔精神点〕を減少させる",
    "唐突に睡魔が。次から２ターンの間、睡眠しなくてはならない",
    "間違いメールから恋が始まる。ハンドルしか知らない「女子高生（p.122）」に偽装した「殺人鬼（p.137）」と恋人（お互いのトリコ）の関係になる",
    "「勧誘員(p.124）」が【追跡】を開始",
    "OSの不調。徹夜で再インストール。この日はもう行動できない上、「無理」をしてしまう",
    "場を荒らしてしまう。このトピックと同じタグのトピックからはリンクをはれなくなる",
    "ボケる。〔教養〕が１点減少する",
    "クラッキングに遭う。いままで調べていたトピックとリンクをすべて失う",
    "ネットサーフィンにハマってしまい、ついつい時間が過ぎる。毎ターンのはじめに〔性業値〕判定を行い、律にならないとそのターンは行動できない。この効果は１日続く",
];

/// Ruby `TABLES["CULTUREIHT"]`（`2D6`）。
static TBL_CULTUREIHT: Table =
    Table::from_dice("情報ハプニング表／〔教養〕", 2, 6, TBL_CULTUREIHT_ITEMS);

/// Ruby `TABLES["COMBATIHT"]`（情報ハプニング表／〔戦闘〕）の項目。
static TBL_COMBATIHT_ITEMS: &[&str] = &[
    "謎の情報屋チュンさん登場。ターゲットとなる情報を渡し、いずこかへ去る。情報ゲット！",
    "悪を憎む心に支配され、一匹の修羅と化す。キジルシの代償から１種類を選び、このセッションの間、習得すること。修得できるキジルシの代償がなければ、あなたはNPCとなる。",
    "自宅に帰ると、無惨に破壊された君のおたからが転がっていた。「この件から手を引け」という書き置きと共に……。この情報フェイズでは、リンク判定を行ったトピックのタグの〔趣味〕を修得していた場合、それを未修得にする。また、おたからを持っていたなら、このセッション中、そのおたからは利用できなくなる。",
    "「俺にはもっと別の人生があったんじゃないだろうか……！？」突如、空しさがこみ上げて来る……その日は各ターンの始めに〔性業値〕判定を行う。失敗すると、酒に溺れ、そのターンは行動済みになる。",
    "クライムファイター仲間からスパイの容疑を受ける……１点のトラウマを追う。",
    "自宅の扉にメモが……！！　「今ならまだ間に合う」奴等はどこまで知っているんだ！？　このトピックからは、これ以上リンクを伸ばせなくなる。",
    "大幇とコンビナートの抗争に何故か巻き込まれる。……なんとか生還するが、次のターンの最後まで行動できず、1D6点のダメージを受ける（セーブ不可）",
    "地獄組の鉄砲玉が君に襲い掛かってきた！！　〔戦闘〕で難易度９の判定に失敗すると、〔肉体点〕が０になる。",
    "「お前はやり過ぎた」の書きおきと共に、友人の死体が発見される〔戦闘〕で難易度９の判定を行う。失敗すると、ランダムに選んだチームの仲間１人が死亡する。",
    "宿敵によって深い疵を受ける。自分の修得している異能の中から、１つ選ぶこと。このセッションのあいだ、その異能を使用することができなくなる。",
    "流氓の男の卑劣な罠にかかり、肥え喰らいの巣に落ちる！！　「掃き溜めの悪魔」1D6体と血戦を行う。戦いに勝たない限り、生きて帰ることはできないだろう……。もちろん血戦に勝ったところで情報は得られない。",
];

/// Ruby `TABLES["COMBATIHT"]`（`2D6`）。
static TBL_COMBATIHT: Table =
    Table::from_dice("情報ハプニング表／〔戦闘〕", 2, 6, TBL_COMBATIHT_ITEMS);

/// Ruby `TABLES["GENERALACCIDENTT"]`（汎用アクシデント表）の項目。
static TBL_GENERALACCIDENTT_ITEMS: &[&str] = &[
    "痛恨のミス。激しく状況が悪化する。以降のケチャップに関する行為判定の難易度に＋１の修正がつき、あなたが追う側なら逃げる側のコマを２マス進める（逃げる側なら自分を２マス戻す）",
    "最悪の大事故。ケチャップどころではない！　〔犯罪〕で難易度９の判定を行う。失敗したら、ムーブ判定を行ったキャラクターは3D6点のダメージを受け、ケチャップから脱落する。判定に成功すればギリギリ難を逃れる。特に何もなし。",
    "もうダメだ……。絶望感が襲いかかってくる。後３ラウンド以内にケリをつけないと、あなたが追う側なら自動的に逃げる側が勝利する（逃げる側なら追う側が勝利する）",
    "まずい、突発事故だ！　ムーブ判定を行ったキャラクターは、1D6点のダメージを受ける。",
    "一瞬ひやりと緊張が走る。　ムーブ判定を行ったキャラクターは、〔精神点〕を２点減少する。",
    "スランプ！　思わず足踏みしてしまう。ムーブ判定を行った者は、ムーブ判定に使用した能力値を使って難易度７の判定を行うこと。失敗したら、ムーブ判定を行ったキャラクターは、ケチャップから脱落。成功しても、あなたが追う側なら逃げる側のコマを１マス進める（逃げる側なら自分を１マス戻す）",
    "イマイチ集中できない。〔性業値〕判定を行うこと。「激」になると、思わず見とれてしまう。あなたが追う側なら逃げる側のコマを１マス進める（逃げる側なら自分を１マス戻す）",
    "古傷が痛み出す。以降のケチャップに関する行為判定に修正が＋１つく",
    "うっかり持ち物を見失う。〔通常装備〕欄からアイテムを１個選んで消す",
    "苦しい状態に追い込まれた。ムーブ判定を行ったキャラクターは、今後のムーブ判定で成功度が－１される。",
    "頭の中が真っ白になる。〔精神点〕を1D6減少する。",
];

/// Ruby `TABLES["GENERALACCIDENTT"]`（`2D6`）。
static TBL_GENERALACCIDENTT: Table =
    Table::from_dice("汎用アクシデント表", 2, 6, TBL_GENERALACCIDENTT_ITEMS);

/// Ruby `TABLES["ROMANCEFUMBLET"]`（ロマンスファンブル表）の項目。
static TBL_ROMANCEFUMBLET_ITEMS: &[&str] = &[
    "みんなあいそをつかす。自分のトリコ欄のキャラクターの名前をすべて消すこと",
    "痴漢として通報される。〔犯罪〕の難易度９の判定に成功しない限り、1D6ターン後に検挙されてしまう",
    "へんにつきまとわれる。対象は、トリコになるが、ファンブル表の結果やトリコと分かれる判定に成功しない限り、常備化しなくてもトリコ欄から消えることはない",
    "修羅場！　対象とは別にトリコを所有していれば、そのキャラクターが現れ、あなたと対象に血戦をしかけてくる",
    "恋に疲れる。自分の〔精神点〕が1D6点減少する",
    "甘い罠。あなたが対象のトリコになってしまう",
    "平手うち！　自分の〔肉体点〕が1D6点減少する",
    "浮気がばれる。恋人関係にあるトリコがいれば、そのキャラクターの名前をあなたのトリコ欄から消す",
    "無礼な失言をしてしまう。対象はあなたに対し「憎悪（p.120参照）」の反応を抱き、あなたはその対象の名前を書き込んだ【仇敵】の代償を得る",
    "ショックな一言。トラウマを１点受ける",
    "トリコからの監視！　このセッションの間、ロマンス判定のファンブル率が自分のトリコの所持数と同じだけ上昇する",
];

/// Ruby `TABLES["ROMANCEFUMBLET"]`（`2D6`）。
static TBL_ROMANCEFUMBLET: Table =
    Table::from_dice("ロマンスファンブル表", 2, 6, TBL_ROMANCEFUMBLET_ITEMS);

/// Ruby `TABLES["FUMBLET"]`（命中判定ファンブル表）の項目。
static TBL_FUMBLET_ITEMS: &[&str] = &[
    "自分の持ち物がすっぽぬけ、偶然敵を直撃！　持っているアイテムを１つ消し、ジオラマ上にいるキャラクター１人をランダムに選ぶ。そのキャラクターの〔肉体点〕を1D6ラウンドの間０点にし、行動不能にさせる（致命傷表は使用しない）。1D6ラウンドが経過し、行動不能から回復すると、そのキャラクターの〔肉体点〕は、行動不能になる直前の値にまで回復する",
    "敵の増援！　「三下(p.125）」が1D6体現れて、自分たちに襲いかかってくる（DDは、この処理が面倒だと思ったら、ファンブルしたキャラクターの〔肉体点〕を1D6点減少させてもよい）",
    "お前のいるマスに「障害物」が出現！　そのマスに障害物オブジェクトを置き、そのマスにいたキャラクターは全員２ダメージを受ける（セーブ不可）",
    "射撃武器を使っていれば、弾切れを起こす。準備行動を行わないとその武器はもう使えない",
    "転んでしまう。準備行動を行わないと移動フェイズに行動できず、格闘、射撃、突撃攻撃が行えない",
    "急に命が惜しくなる。性業値判定をすること。「激」なら戦闘を続行。「律」なら次のラウンドから全力移動を行い、ジオラマから逃走を試みる。「迷」なら次のラウンドは移動・攻撃フェイズに行動できない",
    "誤って別の目標を攻撃。目標以外であなたに一番近いキャラクターに４ダメージ（セーブ不可）！",
    "誤って自分を攻撃。３ダメージ（セーブ不可）！",
    "今使っている武器が壊れる。アイテム欄から使用中の武器を消すこと。銃器を使っていた場合、暴発して自分に６ダメージ！　武器なしの場合、体を傷つけ３ダメージ（共にセーブ不可）！",
    "「制服警官(p.129）」が１人現れる。その場にいるキャラクターをランダムに攻撃する",
    "最悪の事態。〔肉体点〕を０にして、そのキャラクターは行動不能に（致命傷表は使用しない）",
];

/// Ruby `TABLES["FUMBLET"]`（`2D6`）。
static TBL_FUMBLET: Table = Table::from_dice("命中判定ファンブル表", 2, 6, TBL_FUMBLET_ITEMS);

/// Ruby `TABLES["FATALT"]`（致命傷表）の項目。
static TBL_FATALT_ITEMS: &[&str] = &[
    "死亡。",
    "死亡。",
    "昏睡して行動不能。1D6ラウンド以内に治療し、〔肉体点〕を１以上にしないと死亡。",
    "昏睡して行動不能。1D6ターン以内に治療し、〔肉体点〕を１以上にしないと死亡。",
    "大怪我で行動不能。体の部位のどこかを欠損してしまう。任意の〔能力値〕１つが１点減少。",
    "大怪我で行動不能。1D6ターン以内に治療し、〔肉体点〕を１以上にしないと体の部位のどこかを欠損してしまう。任意の〔能力値〕１つが１点減少。",
    "気絶して行動不能。〔肉体点〕の回復には治療が必要。",
    "気絶して行動不能。１ターン後、〔肉体点〕が１になる。",
    "気絶して行動不能。1D6ラウンド後、〔肉体点〕が１になる。",
    "気絶して行動不能。1D6ラウンド後、〔肉体点〕が1D6回復する。",
    "奇跡的に無傷。さきほどのダメージを無効に。",
];

/// Ruby `TABLES["FATALT"]`（`2D6`）。
static TBL_FATALT: Table = Table::from_dice("致命傷表", 2, 6, TBL_FATALT_ITEMS);

/// Ruby `TABLES["ACCIDENTT"]`（アクシデント表）の項目。
static TBL_ACCIDENTT_ITEMS: &[&str] = &[
    "ゴミか何かが降ってきて、視界を塞ぐ。以降のケチャップに関する判定に修正が＋１つく。あなたが追う側なら逃げる側のコマを２マス進める（逃げる側なら自分を２マス戻す）",
    "対向車線の車（もしくは他の船、飛行機）に激突しそうになる。運転手は難易度９の〔精神〕の判定を行うこと。失敗したら、乗物と乗組員全員は3D6のダメージを受けた上に、ケチャップから脱落",
    "ヤバイ、ガソリンがもうない！　後３ラウンド以内にケリをつけないと逃げられ（追いつかれ）ちまう",
    "露店や消火栓につっこむ。その乗物に1D6ダメージ",
    "一瞬ひやりと緊張が走る。〔精神点〕を２点減らす",
    "何かの障害物に衝突する。運転手は難易度７の〔精神〕の判定を行うこと。失敗したら、乗物と乗組員全員は2D6ダメージを受けた上に、ケチャップから脱落。成功しても、あなたが追う側なら逃げる側のコマを１マス進める（逃げる側なら自分を１マス戻す）",
    "走ってる途中に〔趣味〕に関する何かが目に映る。性業値判定を行うこと。「激」になると思わず見とれてしまう。あなたが追う側なら逃げる側のコマを１マス進める（逃げる側なら自分を１マス戻す）",
    "軽い故障が起きちまった。以降のケチャップに関する行為判定に修正が＋１つく",
    "うっかり落し物。〔通常装備〕欄からアイテムを１個選んで消す",
    "あやうく人にぶつかりそうになる。運転手は難易度９の〔精神〕の判定を行う。失敗したら、その一般人を殺してしまう。あなたが追う側なら逃げる側のコマを１マス進める（逃げる側なら自分を１マス戻す）",
    "信号を無視しちまったら後ろで事故が起きた。警察のサイレンが鳴り響いてくる。DDはケチャップの最後尾に警察の乗物を加えろ。データは「制服警官（p.129）」のものを使用",
];

/// Ruby `TABLES["ACCIDENTT"]`（`2D6`）。
static TBL_ACCIDENTT: Table = Table::from_dice("アクシデント表", 2, 6, TBL_ACCIDENTT_ITEMS);

/// Ruby `TABLES["AFTERT"]`（その後表）の項目。
static TBL_AFTERT_ITEMS: &[&str] = &[
    "ここらが潮時かもしれない。2D6を振り、その目が自分の修得している代償未満であれば、そのキャラクターは引退し、二度と使用できない",
    "苦労の数だけ喜びもある。2D6を振り、自分の代償の数以下の目を出した場合、経験点が追加で１点もらえる",
    "妙な恨みを買ってしまった。【仇敵】（p.95）を修得する。誰が【仇敵】になるかは、DDが今回登場したNPCの中から１人を選ぶ",
    "大物の覚えがめでたい。今回のセッションに登場した盟約へ入るための条件を満たしていれば、その盟約に経験点の消費なしで入ることができる",
    "思わず意気投合。今回登場したNPC１人を選び、そのキャラクターとの【コネ】（p.95）を修得する",
    "今回の事件で様々な教訓を得る。自分の修得しているアドバンスドカルマの中から、汎用以外のものを好きなだけ選ぶ。そのカルマの異能と代償を、別な異能と代償に変更することができる",
    "深まるチームの絆。今回のセッションでミッションが成功していた場合、【絆】（p.95）を修得する",
    "色々な運命を感じる。今回のセッションでトリコができていた場合、経験点の消費なしにそのトリコを常備化することができる。また、自分が誰かのトリコになっていた場合、その人物への【トリコ】(p.95）の代償を得る",
    "やっぱり亜侠も楽じゃないかも。今回のセッションで何かツラい目にあっていた場合、【日常】（p.95）を取得する",
    "くそっ！　ここから出せ！！　今回のセッションで逮捕されていたら、【前科】(p.95）の代償を得る",
    "〔性業値〕が１以下、もしくは１３以上だった場合、そのキャラクターは大阪の闇に消える。そのキャラクターは引退し、二度と使用できない",
];

/// Ruby `TABLES["AFTERT"]`（`2D6`）。
static TBL_AFTERT: Table = Table::from_dice("その後表", 2, 6, TBL_AFTERT_ITEMS);

/// Ruby `TABLES["KUSAIMT"]`（臭い飯表）の項目。
static TBL_KUSAIMT_ITEMS: &[&str] = &[
    "やあ署長、ご苦労さん。いつでも好きなときに留置所を出ることができる。",
    "軽い取り調べを受ける。次の１ターンが終了するまで、未行動にならない。",
    "荒っぽい取り調べを受ける。次の１ターンが終了するまで、未行動にならない。１ターン休み。1D6ダメージを受ける（セーブ不可）。",
    "一晩泊まっていきなさい。次の日の朝まで未行動にならない。",
    "粘り強い取り調べが続く。1D6日後の朝まで未行動にならない。",
    "留置所のトイレで陵辱を受ける。1D6日後の朝まで未行動にならない。トラウマを１点受ける。",
    "劣悪な環境のせいで伝染病にかかる。1D6日後の朝まで未行動にならない。【病弱】の代償を得る。",
    "精神異常を訴え、無罪に。しかし、アーカム・アサイレムに移送され、1D6回別のキャラクターでセッションを行うまで、そのキャラクターを使用できない。キジルシの代償の中から、ランダムに１つの代償を得る。",
    "起訴されて有罪に。海上刑務所行き。1D6回別のキャラクターでセッションを行うまで、そのキャラクターを使用できない。【前科】の代償を得る。",
    "起訴されて有罪に。海上刑務所行き。2D6回別のキャラクターでセッションを行うまで、そのキャラクターを使用できない。【前科】の代償を得る。",
    "起訴されて有罪に。海上刑務所行き。終身刑。そのキャラクターは引退する。",
];

/// Ruby `TABLES["KUSAIMT"]`（`2D6`）。
static TBL_KUSAIMT: Table = Table::from_dice("臭い飯表", 2, 6, TBL_KUSAIMT_ITEMS);

/// Ruby `TABLES["ENTERT"]`（登場表）の項目。
static TBL_ENTERT_ITEMS: &[&str] = &[
    "「こっから先にはいかせないぜ」　【仇敵】がいれば現われ、血戦が始まる。現在の血戦、もしくはケチャップが終了したら、処理を行うこと。",
    "「待たせたな、みんな！」　ジオラマの好きな場所に自分のキャラクターを配置する。",
    "おっと、鉢合わせ。ランダムにジオラマ上の敵を１体選ぶ。選んだ敵と同じマスに、そのキャラクターを配置する。",
    "全力ダッシュで駆けつける！　〔肉体点〕を1D6点消費すれば、ジオラマの好きな場所に自分のキャラクターを配置する。そうでなければ、登場できない。",
    "裏道を歩いていたら、偶然その場所にでくわした。DDはジオラマの好きな場所にそのキャラクターを配置する。",
    "「キキィー！」　もしもそのキャラクターが乗物を装備していれば、DDはジオラマの好きな場所にそのキャラクターを配置する。そうでなければ、登場できない。",
    "……間に合ったみたいだな。仲間を１人選び、そのキャラクターと同じマスに自分のキャラクターを配置する。",
    "ラッキー、「ジャリ銭」を拾った。……と、そんな場合じゃないよな。",
    "をっと、お前の好物だ。〔性業値〕判定を行え。「律」ならもう一回、登場表を振ることができる。それ以外なら、キャラクターを配置できない。",
    "んー。ここは一度通ったような。疲労から〔精神点〕を2点減少。",
    "くあー。完全に道に迷っちまった。この実行フェイズには登場できない。",
];

/// Ruby `TABLES["ENTERT"]`（`2D6`）。
static TBL_ENTERT: Table = Table::from_dice("登場表", 2, 6, TBL_ENTERT_ITEMS);

/// Ruby `TABLES["BUDTT"]`（バッドトリップ表）の項目。
static TBL_BUDTT_ITEMS: &[&str] = &[
    "自分の身の周りにいる人たちが異様な何か(悪魔、宇宙人、ゾンビ、お前と同じ顔をした誰か…)に変貌し襲い掛かってくる。お前はNPCとなって、同じ場所にいる誰かに血戦をしかける。血戦が終了すれば（そして生きていれば）、視界は元に戻っている。",
    "世界は一つ。オープンソース。愛で結びつくべきなんだ。お前は自分の知っていることをペラペラと話だし、1D6ターンの間、聞かれれば知っていることを何でも話してしまう。",
    "自分と他人の区別がつかなくなり、現実感が薄れる。〔精神点〕を1D6点減少する。",
    "誰かが自分を殺そうと企んでいるような錯覚を覚える。1D6ターンの間、ペテン師の代償【疑心暗鬼】を修得する。",
    "風景が極彩色に彩られる！もっと……もっと極彩色に！もし他にも「麻薬」カテゴリのアイテムを持っていれば、その中の１個を使用する（行動は使わない）。",
    "目の前にいる人物が非常にいとおしく思えてくる。同じ場所にいるキャラクターの中からランダムに１人選ぶ。1D6ターンの間、そのキャラクターのトリコになる。",
    "魅力的な裸の異性が、あなたの目の前で誘惑する幻覚を見る。〔性業値〕判定を行う。「激」になると服を脱ぎだす。もしも外にいればそのエリアの〔治安〕の難易度の〔犯罪〕判定を行う。失敗すると「臭い飯」表を振る。",
    "お前は痛みを感じなくなる。1D6ターンの間、〔肉体点〕の重症のペナルティが無効化される。",
    "自分の持っているものから触手が生え、あなたにからみつく。自分の〔通常装備〕欄のアイテムの中からランダムに１種を選ぶ。それを捨てる。",
    "皮膚の中を無数の蟲が蠢いているのを感じる。〔肉体点〕を３点減少する。",
    "神々しい声が聞こえてくる。1D6ターンの間、自分の好きな能力値を１点上昇することができる。",
];

/// Ruby `TABLES["BUDTT"]`（`2D6`）。
static TBL_BUDTT: Table = Table::from_dice("バッドトリップ表", 2, 6, TBL_BUDTT_ITEMS);

/// Ruby `TABLES["GETGT"]`（報酬・ガラクタ表）の項目。
static TBL_GETGT_ITEMS: &[&str] = &[
    "持ち主の〔生活〕と等しい個数の《食事》（基本80p、小道具・日用品）",
    "持ち主の〔生活〕と等しい個数の《トルエン》（基本79p、小道具・麻薬）",
    "持ち主の〔生活〕と等しい個数の《ジャリ銭》（基本78p、小道具・お金）",
    "壊れた実用品。実用品表で決定。（壊れたアイテムは、１ターン使用し〔教養〕で難易度９の判定に使用すると直せる）",
    "《テレカ》（基本78p、小道具・通信手段）",
    "何もなかった（涙）。残念でした。",
    "《ロープ》（基本78p、小道具・保安器具）",
    "《トヨトミピストル》（基本74p、武器）",
    "《自転車》（基本76p、乗物）",
    "《ふとん》（基本79p、小道具・日用品）",
    "持ち主の〔趣味〕からランダムに１種類選ぶ。その趣味おたからを１個ランダムに選ぶ。",
];

/// Ruby `TABLES["GETGT"]`（`2D6`）。
static TBL_GETGT: Table = Table::from_dice("報酬・ガラクタ表", 2, 6, TBL_GETGT_ITEMS);

/// Ruby `TABLES["GETZT"]`（報酬・実用品表）の項目。
static TBL_GETZT_ITEMS: &[&str] = &[
    "持ち主と同じタイプの汎用おたから（基本82p、汎用おたから）",
    "価格５の《ホテル》の使用券（基本80p、小道具・サービス）",
    "《苦力》（基本80p、小道具・手下）",
    "《カメラ》（基本80p、小道具・手下）",
    "持ち主が使っていた装備（ただし、一般アイテムに存在しない装備をＰＣは使用できない）",
    "持ち主の〔生活〕と等しい個数の《札巻》（基本78p、小道具・お金）",
    "持ち主の〔生活〕と等しい個数の《大麻》（基本79p、小道具・麻薬）",
    "《ノートパソコン》と《携帯電話》（基本78p、79p、小道具・日用品、通信手段）",
    "《ヴェスパ》（基本76p、乗物）",
    "《救急箱》（基本79p、小道具・保安器具）",
    "《札束》（基本78p、小道具・お金）",
];

/// Ruby `TABLES["GETZT"]`（`2D6`）。
static TBL_GETZT: Table = Table::from_dice("報酬・実用品表", 2, 6, TBL_GETZT_ITEMS);

/// Ruby `TABLES["GETNT"]`（報酬・値打ち物表）の項目。
static TBL_GETNT_ITEMS: &[&str] = &[
    "社会的身分。【日常】の異能を手に入れる。",
    "《人柱》（基本184p、盟約おたから・沙京流氓）",
    "貴重な貴金属。１ターン使って〔生活〕で難易度９の判定に成功すれば《トランク》と交換できる。",
    "持ち主と同じタイプの汎用おたから（基本82p、汎用おたから）",
    "持ち主の〔生活〕と等しい個数の《ヘロイン》（基本79p、小道具・麻薬）",
    "持ち主の〔生活〕と等しい個数の《札束》（基本78p、小道具・お金）",
    "持ち主の〔生活〕と等しい個数の価格５以下の武器（基本79p、小道具・麻薬）",
    "《ロールスロイス》（基本76p、乗物）",
    "持ち主の〔趣味〕からランダムに１種類選ぶ。その趣味おたからを１個ランダムに選ぶ。",
    "《トランク》（基本78p、小道具・お金）",
    "《宝箱》（基本78p、小道具・お金）",
];

/// Ruby `TABLES["GETNT"]`（`2D6`）。
static TBL_GETNT: Table = Table::from_dice("報酬・値打ち物表", 2, 6, TBL_GETNT_ITEMS);

/// Ruby `TABLES["GETKT"]`（報酬・奇天烈表）の項目。
static TBL_GETKT_ITEMS: &[&str] = &[
    "好きな盟約おたから１個（プレイヤー全員で相談して決定）",
    "《気球》（基本76p、乗物）",
    "《チェインソー》（基本74p、武器）",
    "誰かから感謝される。それだけ？",
    "持ち主の〔趣味〕からランダムに１種類選ぶ。その趣味おたからを１個ランダムに選ぶ。",
    "何もなかった（涙）。残念でした。",
    "持ち主と同じタイプの汎用おたから（基本82p、汎用おたから）",
    "《フォークリフト》（基本76p、乗物）",
    "《RPG-7》（基本74p、武器）",
    "倒されたキャラクターは、致命傷表を振り、まだ生きていれば、そのキャラクターを倒した者のトリコになる。",
    "「先にイッてるぜ」そのキャラクター１体を倒した者に経験点が１点与えられる。",
];

/// Ruby `TABLES["GETKT"]`（`2D6`）。
static TBL_GETKT: Table = Table::from_dice("報酬・奇天烈表", 2, 6, TBL_GETKT_ITEMS);

/// Ruby `TABLES["PAYT"]`（落とし前表）の項目。
static TBL_PAYT_ITEMS: &[&str] = &[
    "闇のゲーム。ロシアンルーレットや地下闘技場への出場といった、致死率の高い理不尽な労働に従事させられる。この落とし前を1回受けるたびに、1D6を振る。1の出目が出ると、そのキャラクターは死亡する。",
    "拷問。心身ともに痛めつけられる。この落とし前を1回受けるたびに、【悪夢】、【疑心暗鬼】、【出不精】、【依存体質】、【弱虫】、【虚弱】の中から代償を一つ選んで修得する。どの代償もすでに修得していた場合、そのキャラクターは死亡する。新しい恋人ができるたび、この落とし前の効果を1回分、無効化することができる。",
    "苦役。売春や、組織犯罪の資金源になるよう強制労働に従事させられる。この落とし前を1回受けるたびに、以降、セッションの間に「苦役」という特に何の効果ももたらさない計画的行動を一度行わなければいけなくなる。セッション中に規定の「苦役」の回数をこなすことができなかったキャラクターは、「苦役」の必要回数に満たない数だけ、「落とし前表」を使用しなければならない。《トランク》を1個消費すると、この落とし前表の効果を1回分、無効化することができる。",
    "係累への被害。自分の身内や恋人が殺される。この落とし前を1回受けるたびに、トラウマを1点受ける。",
    "部位破壊。指や手首を切り落とされたり、臓器を摘出されたりする。この落とし前を1回受けるたびに、「致命傷表」の6番の効果を受ける。",
    "罰金。法外な違約金を払わされたり、借金を負わされたりする。この落とし前を1回受けるたびに、〔サイフ〕の最大値が1点減少する。〔サイフ〕の最大値が0点になると、そのキャラクターは死亡する。《札束》を5個消費すると、この落とし前の効果を一回分、無効化することができる。",
    "さらし者。謝罪会見を行わされたり、恥ずかしい動画や写真を公開されたりする。この落とし前を1回受けるたびに、【世界の敵】、【悪名】、【有名人】、【狼少年】、【手配書】、【カモ】の中から代償を一つ選んで修得する。どの代償もすでに修得していた場合、そのキャラクターは死亡する。経験点を2点消費すると、この落とし前の効果を1回分、無効化することができる。",
    "刻印。坊主頭にされたり、恥ずかしい入れ墨や刻印を刻み付けられたりする。この落とし前を1回受けるたびに、そのキャラクターが行うロマンスや交渉の判定の難易度が1点上昇する。",
    "差し押さえ。この落とし前を1回受けるたびに、そのキャラクターは、自分が装備しているもっとも価格の高いアイテム1つを失う。おたからは価格8として扱い、もっとも高い価格のアイテムを複数持っている場合は、その中からランダムに選ぶ。",
    "監禁。マグロ漁船や地下工場に閉じ込められ、長期的な労働に従事させられる。この落とし前を1回受けるたびに、一回別のキャラクターでセッションを行うまで、そのキャラクターを使用できない。《札束》を5個消費すると、この落とし前の効果を1回分、無効化することができる。",
    "去勢。性的な機能を破壊される。この落とし前を受けると、「無言で押し倒す」ことができなくなる。この落とし前を二回以上受けると、そのキャラクターは死亡する。",
];

/// Ruby `TABLES["PAYT"]`（`2D6`）。
static TBL_PAYT: Table = Table::from_dice("落とし前表", 2, 6, TBL_PAYT_ITEMS);

/// Ruby `TABLES["MINAMIRET"]`（ミナミ遭遇表）の項目。
static TBL_MINAMIRET_ITEMS: &[&str] = &[
    "（場所）大変だ、阪神が勝った。4000人のトラキチが、一緒に道頓堀に飛び込もうと迫る。飛び込むなら水中には「下水ワニ」（基本132p）が待っている。拒否するならトラキチたちは「ベンガル虎」（基本133p）を亜侠にけしかける。",
    "（場所）突然の夕立、そして稲妻！　武器を一番多く持っている亜侠に雷が落ちる。複数いた場合、1D6で一番高い目を出した亜侠に落ちる。黒焦げになり、パーマがかかり、ランダムに一つ武器を失う（熔ける）。",
    "（一人）酔っ払いの吐瀉物を浴びて〔精神点〕に1ダメージ。風呂に入って着替えるまで、あらゆる交渉は自動的に成功度が-1される。",
    "（一人）好みの恋愛対象に出会ったと思ったら「美人局」（基本124p）だった！",
    "（一人）うっかり入った店が暴力喫茶だった！　「押し売り」（基本124p）相当。",
    "（一人）しつこいキャッチに絡まれる。「勧誘員」（基本124p）相当。",
    "（一人）裏路地で襲われる。「痴漢・痴女」（基本124p）相当。",
    "（一人）道を渡ろうとしたら路面電車に撥ねられる。〔肉体点〕に1D6ダメージ。",
    "（一人）契約刑事に恐喝される。逮捕されたくなければ、〔サイフ〕を1減らせ。",
    "（一人）うっかりマリア・ヴィスコンティを怒らせた！　この場では何も起こらないが、マリアは忘れない。次に亜侠が致命傷表送りになったとき、マリアが出現して亜侠の利き腕を吹き飛ばして去る（致命傷表の判定が必ず「7～9」になる）。",
    "（一人）「円盤」（基本138p）に襲われ、さらわれる。1ターン経って戻ってくると、頭からアンテナが生えている。キャラクターイラストにアンテナを書き加えろ。それが嫌なら戦うこと。",
];

/// Ruby `TABLES["MINAMIRET"]`（`2D6`）。
static TBL_MINAMIRET: Table = Table::from_dice("ミナミ遭遇表", 2, 6, TBL_MINAMIRET_ITEMS);

/// Ruby `TABLES["CHINATOWNRET"]`（中華街遭遇表）の項目。
static TBL_CHINATOWNRET_ITEMS: &[&str] = &[
    "（一人）好みの恋愛対象に襲われて目くるめく一時を過ごす。だが1ターン経って目が覚めると、房中術で性転換させられている。",
    "（場所）人を食う「パンダ」（基本133p）に襲われる。",
    "（一人）道端の占い師をうっかり撥ねて、人肉饅頭の呪いを受ける。このセッション中に行動不能になったら、狂気の料理人に饅頭にされて食われ、後には何も残らない。",
    "（場所）漢方薬局を冷やかしていたら、世界自然保護プロレス基金WWWWF（World_Wide_Wildelife_Wrestling_Found）のレスラー1D6人（「街頭覇王」（基本134p））に襲われる。亜侠が手に取った犀の角が気に入らなかったのだ。誰かが〔サイフ〕を1支払ってTシャツを買うと許してくれる（キャラクターイラストにパンダのマークをつけること）が、そうでなければ戦うしかない。",
    "（場所）化石を売りつけようとする「押し売り」（基本124p）に遭遇。品物は1D6を振って決める。1：ゴモラの全身骨格、2：マチカネワニの涙、3：明石原人の糞石、4：生きた北京原人、5：豊臣秀吉15歳のしゃれこうべ、6：三葉虫ボトルキャップ",
    "（場所）映画の撮影現場に紛れ込んでしまった！　このセッションの間、戦闘時になると敵味方全員に《透明ワイヤー》の効果がつく。既に持っていた場合、「跳ぶ」を選んでも使用回数を消費しない。",
    "（一人）美味しい中華料理を食いすぎて、一時的にすごく太る。このセッションの間は、〔肉体〕が+1されるが、その亜侠に対する命中判定にも+1の修正がつく。",
    "（一人）道端で碁の勝負を見ていたら、いつのまにか1ターン経っていた。代わりに〔精神点〕が1回復する。",
    "（一人）お前をスターと間違えたおっかけの大群が迫ってくる。このセッションの間、【有名人】の代償がつく。",
    "（場所）空飛ぶギロチンを持った老人と片腕の格闘家が戦っている。二人はどちらもチームに加勢を求める。老人は「罪狩」（基本135p）、格闘家は「殺人鬼」（基本137p）だ。味方するなら、どちらに手を貸すか決めて戦闘を行え（味方された本人は戦わない）。どちらかを倒したらもう一人は礼を言って即座に去る。倒したのが格闘家なら《不肖の弟子》が、老人なら《空飛ぶギロチン》（本格的武器）が手に入る。",
    "（場所）「狂人」とその一党が現れた！　キャラクターシートの「好きな映画」が空欄だった亜侠は、頭に火をつけられる。キャラクターのイラストから髪を取り除け。シートに顔を描いていなかった場合、キャラクターシートを燃やすか、自分の頭を燃やすか、〔肉体点〕を1減らすこと。「狂人」を倒すことはできない。彼は永遠だ。",
];

/// Ruby `TABLES["CHINATOWNRET"]`（`2D6`）。
static TBL_CHINATOWNRET: Table = Table::from_dice("中華街遭遇表", 2, 6, TBL_CHINATOWNRET_ITEMS);

/// Ruby `TABLES["WARSHIPLANDRET"]`（軍艦島遭遇表）の項目。
static TBL_WARSHIPLANDRET_ITEMS: &[&str] = &[
    "（場所）蟹の押し売りに遭う。しかもただの押し売りではない、食い詰めた「超人兵士」（基本134p）だ！　蟹（価格3の「食事」（基本79p））を買うか、そうでなければ戦うこと。",
    "（場所）救世軍の配給に長い行列ができている。〔性業値〕判定で「激」を出した亜侠はつい並んでしまい、1ターン消費して「食事」（基本79p）をゲット。",
    "（場所）荒廃した通りを横断していたら撃たれる。スナイパーだ！　全員1D6を振り、一番低い目を出した亜侠は〔肉体点〕に1D6のダメージ。",
    "（場所）突然の路面陥没！　乗り物を所持している亜侠は〔精神〕9の判定に成功しないと、逃げ遅れて乗り物に2D6のダメージを受ける。",
    "（場所）季節はずれの雪が降っている。青く光ってなんだかとても美しい。とりあえず〔肉体点〕と〔精神点〕に1ダメージ。",
    "（場所）飢えた野犬がぞろぞろついてくる。このセッション中に軍艦島で行動不能になった亜侠は、すぐに食われて後には何も残らない。",
    "（一人）気がつくと食玩塗りの搾取工場の中……。コンビナートの手配師に捕まったのだ。脱出するのに1ターン無駄にするが、見張りの「ブラックアドレス」（基本127p）一体と戦って勝てば時間を無駄にせずにすむ。",
    "（一人）自宅が膨張する203高地に取り込まれた。脱出するのに1ターンかかる上、アジトの場所が「軍艦島」になってしまう。",
    "（場所）ガス爆発！1D6して出た目のエリアに飛ばされる上、飛ばされたエリアの遭遇表を振らねばならない。1：ミナミ、2：十三、3：沙京、4：中華街、5：官庁街、6：軍艦島",
    "（場所）ひょろひょろ跳んできた「ミサイル」（基本130p）と目が合った。目標を見失っていたミサイルは、亜侠を新たな目標に決めて親しげに近づいてくる。",
    "（一人）「タイラー・ダーデン」に遭遇し、啓蒙される。これがお前の人生だ。お前はいつか必ず死ぬ。それを認識しない限り、お前は糞のままだ。〔肉体点〕と〔精神点〕に1ダメージ。〔性業値〕が2下がる。更にこのセッション中は「迷」が出ても「激」として扱う。",
];

/// Ruby `TABLES["WARSHIPLANDRET"]`（`2D6`）。
static TBL_WARSHIPLANDRET: Table = Table::from_dice("軍艦島遭遇表", 2, 6, TBL_WARSHIPLANDRET_ITEMS);

/// Ruby `TABLES["CIVICCENTERRET"]`（官庁街遭遇表）の項目。
static TBL_CIVICCENTERRET_ITEMS: &[&str] = &[
    "（場所）大規模なデモにぶつかって身動きが取れなくなった。まずいと思う間もなく、列強の鎮圧部隊が容赦なく群集に向かって発砲する。阿鼻叫喚の中で全員〔肉体点〕に1D6のダメージ。",
    "（場所）火事だ！　ビルがぼうぼう燃えている。〔犯罪〕8の判定に失敗すると、野次馬の中でスリにやられ、アイテムをランダムに一つ失う。",
    "（場所）株価暴落で取り付け騒ぎが起こっている。〔生活〕4以上の亜侠は〔サイフ〕が1減る。",
    "（一人）身投げか事故か突き落とされたのか、ビルから人が振ってきた。〔戦闘〕で難易度9の判定に成功しないと、直撃されて〔肉体点〕に2D6のダメージ。",
    "（場所）観光客に写真を撮られる。このセッション中、亜侠に対する逮捕判定の難易度は-1される。",
    "（一人）汚職警官に職務質問される。「押し売り」（基本124p）相当。",
    "（場所）軍事パレードが開催中だ。「デモ行進」（基本124p）相当。",
    "（場所）今日は即売会だ。「ヲタク」か「マニア」の〔趣味〕を持つ亜侠は、1ターン消費して買い物しないと、〔精神点〕に1D6ダメージ。",
    "（一人）爆弾テロだ！　1D6して出た目のエリアに飛ばされる上、飛ばされたエリアの遭遇表を振らねばならない。1：ミナミ、2：十三、3：沙京、4：中華街、5：官庁街、6：軍艦島",
    "（場所）ビルから降ってきたお札をみんなが奪い合っている。争奪戦に加わるなら、〔肉体点〕に1D6ダメージを受けて〔サイフ〕を1回復してよい。",
    "（一人）閉鎖されているはずの地下鉄の入り口が開いている……。性業値判定で「激」が出るとふらふらと入ってしまい、1ターン経った後戻る。奇天烈の宝物表（基本140p）を1回振れる。トラウマを1点受け、中で起こったことは何も憶えていない。入り口は固く閉ざされ、もう開かない。……今のところは。",
];

/// Ruby `TABLES["CIVICCENTERRET"]`（`2D6`）。
static TBL_CIVICCENTERRET: Table = Table::from_dice("官庁街遭遇表", 2, 6, TBL_CIVICCENTERRET_ITEMS);

/// Ruby `TABLES["DOWNTOWNRET"]`（十三遭遇表）の項目。
static TBL_DOWNTOWNRET_ITEMS: &[&str] = &[
    "（場所）地震だ！　亜侠自身には被害はないが、家が大変なことに。帰宅すると、アジト装備がランダムに1個壊れている。アジト装備がなかった場合、家が壊れている。",
    "（場所）山から下りてきた猪が突っ込んでくる！　データは「トラック野郎」（基本123p）を使う。",
    "（一人）草野球の代打を頼まれる。1D6せよ。「スポーツ」の趣味があれば+1。1,2：三振！　冷たい視線を浴びて〔精神点〕-1、3,4：ヒット！　喝采を浴びて〔精神点〕1回復。5,6：ホームラン！　そしてガラスの割れる音！窓を割られた家から怒り狂った「おかん」（基本122p）が飛び出して、大根片手に亜侠を襲う。",
    "（場所）「獅子舞」（基本128p）が亜侠の周りをぐるぐる周って離れようとしない。この状態で戦闘が起こると、獅子舞は敵に加わって亜侠を襲う。〔サイフ〕を1渡せば、獅子舞は歯をがちがち言わせて去る。",
    "（一人）地獄湯の「勝負師」（基本126p）に賭けを挑まれ、ざわざわする。",
    "（一人）お魚くわえようとするドラ猫に襲われる。〔犯罪〕で難易度9の判定に失敗したらランダムにアイテム一つを失う。亜侠が「食事」（基本79p）を持っていれば、優先的にそれを狙う。",
    "（場所）神風師団の自警団に囲まれた！　名前にカタカナがある亜侠がいたら集中的に襲われる（「忘八」（基本128p）1D6人相当）。日本人名の亜侠しかいなければ、被害を受けることはない。",
    "（一人）生臭坊主/生臭尼僧にお布施を要求される。データは「勧誘員」（基本124p）。",
    "（一人）大仏から身投げをした人が降ってきた。〔戦闘〕で難易度9の判定に成功しないと、直撃されて〔肉体点〕に2D6のダメージ",
    "（場所）祭囃子が聞こえてきたかと思ったら、目の前をすごい早さで神輿が通り過ぎる。性業値判定で「激」が出た亜侠は、思わず祭に参加してしまい、高速神輿に連れ去られる。1ターン戻ってこない上、疲れきって〔肉体点〕-1。",
    "（一人）好みの恋愛対象に誘われて夢のような時間を過ごし、気が付くと肥溜めに肩まで漬かっていることに気付く。おばけに化かされた！1ターン消費する。風呂に入って着替えるまで、あらゆる交渉は自動的に成功度が-1される。この効果は一緒に行動する仲間の判定にも影響する。",
];

/// Ruby `TABLES["DOWNTOWNRET"]`（`2D6`）。
static TBL_DOWNTOWNRET: Table = Table::from_dice("十三遭遇表", 2, 6, TBL_DOWNTOWNRET_ITEMS);

/// Ruby `TABLES["SHAOKINRET"]`（沙京遭遇表）の項目。
static TBL_SHAOKINRET_ITEMS: &[&str] = &[
    "（一人）いつのまにか、少女/少年が1人ついてきている。奴隷のようだが、亜侠を主人だと思っているらしく離れようとしない。何に相当するか1D6を振れ。1：【守るべき者】（基本103p）、2：愛人（基本80p）、3：使用人（基本80p）、4：居候（基本80p）、5：落とし穴（基本78p）、6：食事（基本79p）。性別はプレイヤーが決めてよい。",
    "（場所）どこからか煙が漂ってくる……麻薬工場が火事だ！　煙を吸って目を回し、各自1D6を振れ、出た目のドラックの効果を受ける。直接摂取ではないので、ドラッグの強度からは-3。1：コカイン、2：大麻、3：ハルシオン、4：トルエン、5：エクスタシー、6自白剤（ドラッグのデータ→基本79p）",
    "（場所）やけに人懐っこい豚がいると思ったら、人の味を覚えた豚だった！　戦闘になる。データは「ベンガル虎」（基本133p）を使う。",
    "（場所）あばれ象が車を踏み潰して暴走している！　乗り物を所持している亜侠は〔精神〕9の判定に成功しないと、逃げ遅れて乗り物に2D6のダメージを受ける。",
    "（場所）インド人が死んでいる……。死体を漁るなら1D6。1：カレー味の《視肉》（基本85p）、2：サファイア（札束（基本78p）相当）、3：《あわてるなタオル》（基本93p）、4：RPG-7（基本74p）、5：死体じゃなくて「ゾンビ」（137p）だった、6：「はきだめの悪魔」だった",
    "（一人）バクシーシ！　バクシーシ！　じゃりンこ10人が〔サイフ〕1点払うまでぞろぞろついてくる。何かあると「邪魔」（基本37p）10人分を行う。",
    "（一人）いきなり足に何かが噛み付く。「下水ワニ」（基本132p）だ！　〔肉体〕9の判定に成功すると振りほどけるが、失敗すると〔肉体点〕に1ダメージ。成功するまで判定すること。〔肉体点〕が0になると、亜侠は水路に引きずり込まれて食われる。",
    "（一人）気がつくと奴隷船の船倉の中……。奴隷商人の人狩りに捕まったのだ。脱出するのに1ターン無駄にするが、見張りの「ククバット」（基本127p）1体と戦って勝てば時間を無駄にせずにすむ。",
    "（一人）アラブの露天商に水煙草を勧められる。一服してまったりする亜侠だが、その懐に小猿が手を伸ばす……。アイテムをランダムに1つ失う。",
    "（一人）魚を満載したトラックから鮫が落ちてきて、亜侠に噛み付いて死ぬ。怪我はないが離れようとしない。死んだ鮫をぶら下げて歩くことになるので、セッション終了時まで、その亜侠と一緒にいると治安が+2される。",
    "（場所）祝祭だ！　巨大なジャガーノート（山車）が通りを突き進んでくる。これに轢かれると幸せな来世が保証されるのだ。性業値判定を3回振れ。全部「激」を出した亜侠は、思わず車輪の下に飛び込んで〔肉体点〕に10のダメージを受ける。この亜侠が死んだら、次に作るキャラクターに異能と代償を一つずつ受け継がせること。これがカルマだ。",
];

/// Ruby `TABLES["SHAOKINRET"]`（`2D6`）。
static TBL_SHAOKINRET: Table = Table::from_dice("沙京遭遇表", 2, 6, TBL_SHAOKINRET_ITEMS);

/// Ruby `TABLES["LOVELOVERET"]`（らぶらぶ遭遇表）の項目。
static TBL_LOVELOVERET_ITEMS: &[&str] = &[
    "（場所）お互いに運命を感じる。この後のロマンス判定で成功して、トリコを獲得した場合、セッション終了時にそのトリコを経験点消費なしに常備化できる。ただし、このトリコと別れたり、このトリコが死亡したりすると、経験点が1点減少する。",
    "（場所）「……こんなとこで何やってんの？」「げ」もし、自分のトリコの中に、この場所と同じ〔趣味〕の持ち主がいた場合、その人物が現れる。血戦を行うこと。",
    "（一人）デート中に相手の姿を見失ってしまう。〔犯罪〕で難易度9の判定を行う。成功すると、その場所の〔趣味〕に対応した趣味おたからをランダムに1つ獲得する。失敗すると、ロマンス判定は行えなくなる。",
    "（一人）楽しくショッピング！　〔生活〕で難易度9の判定を行う。成功すると、その成功度と同じ値だけ、〔精神点〕を回復できる。失敗すると、セッション中、この後のロマンス判定のファンブル率が2点上昇する。",
    "（一人）「…………」互いに遠慮して気まずい感じ。〔恋愛〕で難易度9の判定を行う。成功すると、この後のロマンス判定の難易度が1点減少する。失敗すると、この後のロマンス判定の難易度が2点上昇する。",
    "（場所）亜侠稼業を忘れてしまいそうなほど、充実した時間を過ごす。デートを行っているキャラクター全員は、〔精神点〕が2点回復する。",
    "（一人）「うーん、ここつまんないね。場所変えよっか」〔恋愛〕で難易度9の判定を行う。成功すると、この後のロマンス判定の難易度が1点減少する。失敗すると、デート参加者全員は、このエリアの遭遇表をさらに1回ずつ振らなければならない。",
    "（一人）趣味の会話で盛り上がる！　〔教養〕で難易度9の判定を行う。成功すると、それ以降一度だけ、そのセッション中に行う判定の難易度を、その成功度と同じ値だけ減少することができる。失敗すると、セッション中、この場所の〔趣味〕が未修得の状態になる。",
    "（一人）「あぶない、暴れ馬だッ！」〔戦闘〕で難易度9の判定を行う。成功すると、デートの相手の好みを、自分のタイプと同じものに変更することができる。失敗すると、デートの相手は1D6点のダメージを受ける（セーブ不可）。",
    "（場所）「ようよう、綺麗なねぇちゃん、連れとるやんけ」「三下」が1D6人現れる。血戦を行うこと。",
    "（場所）「んー付き合っちゃおうか」デートに参加したキャラクターは、この後のロマンス判定が自動的に成功する。",
];

/// Ruby `TABLES["LOVELOVERET"]`（`2D6`）。
static TBL_LOVELOVERET: Table = Table::from_dice("らぶらぶ遭遇表", 2, 6, TBL_LOVELOVERET_ITEMS);

/// Ruby `TABLES["AJITORET"]`（アジト遭遇表）の項目。
static TBL_AJITORET_ITEMS: &[&str] = &[
    "（一人）「強盗殺人の容疑で逮捕する！　お前には黙秘権があり、供述は、法廷で不利な証拠として……」どやどやと踏み込んでくる警官たち。「制服警官」がお前を対象にして【逮捕】の異能を使ってくる。「制服警官」の判定が失敗すると、それ以降、このセッション中では、「臭い飯」表を振るとき、その2D6の目からマイナス3することができる。",
    "（一人）ピンポーン♪　チャイムの音。イヤな予感がするなぁ。1D6を振る。奇数なら「勧誘員」が【ムダ話】を、偶数なら「押し売り」が【売り口上】を使ってくる。",
    "（一人）イメージチェンジ！　たまにはスタイルを変えてみようかな？　外見表を使って、ランダムに外見を変える。そのセッション中、各エリアで〔犯罪〕の行為判定を行うとき、そのエリアの衣装欄に書かれた外見であれば、振ることのできる2D6の回数が1回上昇する。",
    "（場所）「やっぱり、ここにいやがったな」このアジトにいるキャラクターが【コネ】か【仇敵】か【トリコ】の汎用異能、もしくは汎用代償を修得していれば、それに対応するキャラクター（コネ、仇敵、自分の主人）が現れる。コネなら、アジトにいる全員は、価格がそのキャラクターの〔生活〕-1以下のアイテムを1つ獲得できる。仇敵なら、アジトにいる全員は1ダメージを受ける（セーブ不可）。自分の主人なら【トリコ】の持ち主は、別れをつげられ【トリコ】を失うが〔精神点〕が2D6点減少する。",
    "（一人）「よう、元気にしてるか？」家族や友人からの突然の連絡。懐かしい気持ちに高揚しつつも、優しい気持ちになる。〔性業値〕を1点上昇、もしくは1点減少することができる。",
    "（場所）何となくテレビでもつけ、面白いチャンネルがないか、探してみる。うーん。ケーブルテレビに入るべきか……。アジトの持ち主の〔生活〕の値と同じ回数だけ「趣味決定表」を振り、各自、その結果と自分の〔趣味〕を比べてみる。自分が持っている〔趣味〕と同じ〔趣味〕が出ていれば、その回数だけ、自分の〔精神点〕を回復する。",
    "（一人）あ、こんなところに買い置きが。《食事》を1D6個獲得する。",
    "（場所）「みんなで鍋でもするか」現在、このアジトにいるキャラクターと手下カテゴリーのアイテムの合計数だけ、〔精神点〕が回復する。",
    "（一人）ふぅ。やっぱり、自宅が一番落ち着くなぁ。もしも修得していなければ【日常】の異能を修得する。",
    "（一人）一休み……のつもりが、ついつい居眠りしてしまう。アジトに自分しかいなければ、性業値判定を行うこと。「律」になれば、〔精神点〕をアジトの〔快適度〕の半分だけ回復することができる。「激」なら、「睡眠」をしてしまう。「迷」なら、次のターンもアジトから移動できず、行動もできない。アジトに誰かいたら、もう一度アジト遭遇表を振ること。",
    "（一人）謎の贈り物が届く……。趣味おたからの中からランダムに1つを選び、それを獲得する。その後、1D6を振る。その目が、このイベントで趣味おたからを獲得した回数以下だった場合、贈り主の呪いによって2D6点のダメージを受ける（セーブ不可）。",
];

/// Ruby `TABLES["AJITORET"]`（`2D6`）。
static TBL_AJITORET: Table = Table::from_dice("アジト遭遇表", 2, 6, TBL_AJITORET_ITEMS);

/// Ruby `TABLES["JIGOKUSPARET"]`（地獄湯遭遇表）の項目。
static TBL_JIGOKUSPARET_ITEMS: &[&str] = &[
    "（一人）なぜかあなたはローマ時代にタイムスリップする！　今後、このシナリオのあらゆる判定の難易度が1減少する（累積不可）。",
    "（場所）お湯の中に鮫が！　〔戦闘〕で難易度9の判定を行う。失敗したキャラクターは〔肉体点〕を1D6点減少。",
    "（一人）地獄湯の出張販売！　望むなら価格のある地獄組の盟約アイテムを購入することができる。",
    "（場所）ふぅ。湯上がりは親でも惚れるね。〔恋愛〕で難易度9の判定に成功すると、その場にいる好きなキャラクター1人をトリコにすることができる。",
    "（一人）あああ、なんか面白そうだなぁ。性業値判定を行う。「激」ならついついギャンブルゾーンに行ってしまう。「迷」なら行動済みになってしまう。「ギャンブル」の〔趣味〕の持ち主は、サイコロの目に2を加えること。",
    "（場所）地獄組による監視。もしチームが彼らと敵対していれば、うまく身を隠す必要がある。〔犯罪〕で難易度9の判定を行う。失敗すると、〔肉体点〕を1点減少する。そうでなければ、何もなし。",
    "（一人）うーん。のぼせちゃったかな。〔精神点〕を1点減少する。",
    "（場所）道に迷いそうになる。〔教養〕で難易度9の判定を行い、失敗したキャラクターは、地獄湯内の6つのゾーンの中からランダムに1つを選び、そこに移動する。",
    "（一人）ナンパにあう。〔恋愛〕で難易度9の判定に成功すると、色々おごってもらえる。〔サイフ〕を1点回復することができる。",
    "（場所）まずい。お湯の温度が恐ろしいことになっている！　〔精神点〕を2点減少する。",
    "（一人）価格3の買い物を行うと、マッサージをしてもらえる。〔肉体点〕を1点、〔精神点〕を1D6点回復できる。",
];

/// Ruby `TABLES["JIGOKUSPARET"]`（`2D6`）。
static TBL_JIGOKUSPARET: Table = Table::from_dice("地獄湯遭遇表", 2, 6, TBL_JIGOKUSPARET_ITEMS);

/// Ruby `TABLES["JAILHOUSERET"]`（JAILHOUSE遭遇表）の項目。
static TBL_JAILHOUSERET_ITEMS: &[&str] = &[
    "「あちらのお客様からです」と渡されたグラス。その中には爆発寸前の《手榴弾》が入っていた。手榴弾はそのPCに命中したものとして扱う。",
    "「……パパぁ」小さな「じゃりンこ」があなたの裾をつかむ。そのセッションの間、「じゃりンこ」がついてきて、そのPCのロマンス判定を【邪魔】する。",
    "「あ、サイフがない！」〔サイフ〕を1点減らす。",
    "乗物が盗まれる！　装備の中に乗物があった場合、そのアイテムを失う。",
    "誰かと荷物を間違えてしまう！　自分の装備からランダムに1個のアイテムを失う。その後、自分の〔生活〕と等しい報酬表を振ってアイテムを1つ手に入れる。",
    "「な、なんだテメェ！」他の客たちの喧嘩に巻き込まれる。〔肉体点〕を2点減少（セーブ不可）。",
    "「突然だけど……別れましょう」あなたに恋人がいれば、そのキャラクターが現れ、2人は別れる。",
    "何かの間違いだろうか。きみのあおったグラスの中に、《エクスタシー》が混じっていた！　か、体があつィっ！",
    "「ようチンピラ、まだ生きてたのかい？」契約刑事のマリアが絡んでくる。彼女を楽しませるために「飲み会」を行わないとチーム全員が「臭い飯」表を一回ずつ振らなければいけない。",
    "突然の銃声！　「侠客」1人がきみに向かって《トカレフ》を「仁義なく」ぶっ放す！　血戦を開始せよ。",
    "「エルヴィス」があらわれ、店で奇跡的なまでに楽しいパーティーが行われる。チーム全員が気付くと1日が経過していた。",
];

/// Ruby `TABLES["JAILHOUSERET"]`（`2D6`）。
static TBL_JAILHOUSERET: Table = Table::from_dice("JAILHOUSE遭遇表", 2, 6, TBL_JAILHOUSERET_ITEMS);

/// Ruby `TABLES["TREATMENTIT"]`（治療イベント表）の項目。
static TBL_TREATMENTIT_ITEMS: &[&str] = &[
    "不治の病だったことが分かる。1D6セッション後に死亡するが、今回以降のセッションで得られる経験値はすべて2倍になる。",
    "治療中の動物が脱走！　サイコロを1個振り、1～4なら「番犬」が、5～6なら「ベンガル虎」が現れる。誰かが〔戦闘〕で難易度11の判定に成功すると、血戦を回避できる。",
    "「ここかなぁ～」治療の結果、変なツボをつかれたらしく、このセッションの間、〔破壊力〕が9に、〔反応力〕が1になる。",
    "「芸術的な内臓をしている」希望すれば、あなたの腎臓を《トランク》1つで買ってくれる。",
    "急患が大量に運ばれてくる。これ以降、このセッション終了時まで、乃木クリニックの治療の価格が、すべて1上昇する。",
    "乃木センセイの本気が炸裂！　美形タイプの男性キャラクターが1人いるたびに、治療の成功度が自動的に+2される。",
    "「だって、字ぃ読めないしぃ」リョータが点滴を間違える。サイコロを1個振り、下記のドラッグを摂取してしまう。1：《トルエン》　2：《ヘロイン》　3：《ハルシオン》　4《エクスタシー》　5：《コカイン》　6：《シャブ》",
    "「ウッソ、マッジ！？」待合室で読みたかった雑誌のバックナンバーを発見。〔精神点〕が全快し、トラウマを受けていればそれも1点回復する。",
    "医療ミス！　サイコロを1個振り、1～4ならメスが、5～6ならランダムに趣味おたから1個が手術のミスで身体の中に残ってしまう。メスが身体に残っているキャラクターは、ファンブルを起こすたびに1ダメージを受けてしまう。重症の治療の判定に成功すると、中のアイテムを取り出すことができる。",
    "治療のついでに身体の異常が発見される。男性なら性病で鼻がもげ〔恋愛〕が1点減少、女性なら子供ができていることがわかる。",
    "奇跡的な治療のワザ！　治療判定の結果に関わらず〔肉体点〕が全快する。",
];

/// Ruby `TABLES["TREATMENTIT"]`（`2D6`）。
static TBL_TREATMENTIT: Table = Table::from_dice("治療イベント表", 2, 6, TBL_TREATMENTIT_ITEMS);

/// Ruby `TABLES["COLLEGEIT"]`（大学イベント表）の項目。
static TBL_COLLEGEIT_ITEMS: &[&str] = &[
    "運動家に勧誘される。〔精神点〕を2点減少する。",
    "痴情のもつれ！　自分のトリコの数を数える。1D6を振り、その数以下の目を出してしまった場合、刺されてしまう。〔肉体点〕に3ダメージ（セーブ不可）。",
    "バイトの張り紙が……　セッション中に、何でもいいのでおたからを手に入れていれば、セッション終了時にそれを《トランク》で買い取ってくれる。",
    "コンパに誘われる。次の日の夜にコンパに行くことができる。コンパに行ったキャラクターは〔恋愛〕で難易度9の判定に成功すると、「女子高生」と恋人になる。",
    "授業にもぐりこむ。すやすやと心地よい時間が過ぎ、〔精神点〕を6点回復する。",
    "サークルボックスでダベる。同じターンに、他の仲間がリンク判定を行っていれば、好きな情報イベント表を振ることができる。",
    "キャンパスでいちゃいちゃカップルに遭遇する。らぶらぶオーラにあてられる。",
    "代返を頼まれる。〔精神〕で難易度9の判定に成功しないと、次の日の朝は行動を行えない。その代わり、学食で一回おごってもらえる。",
    "教授の実験に付き合わされる。〔肉体〕で難易度9の判定に失敗すると、《LSD》を飲まされる。",
    "麻雀に誘われる。次の日の夜に麻雀に行くことができる。麻雀に行ったキャラクターは、〔犯罪〕で難易度9の判定を行い、その成功度分だけ〔サイフ〕を回復することができる。ただし、〔性業値〕の判定を行い「律」以外だと徹マンになり、その日は無理してしまう。",
    "恋愛フラグが起動。チームの異性のキャラクターをランダムに1人選び、その人のトリコになる。また、選ばれた異性のキャラクターもランダムに異性キャラクターを1人選び、その人のトリコになる。",
];

/// Ruby `TABLES["COLLEGEIT"]`（`2D6`）。
static TBL_COLLEGEIT: Table = Table::from_dice("大学イベント表", 2, 6, TBL_COLLEGEIT_ITEMS);

/// Ruby `TABLES["FATALVT"]`（乗物致命傷表）の項目。
static TBL_FATALVT_ITEMS: &[&str] = &[
    "乗物は破壊。乗物に乗車していたキャラクターは、性業値判定を行う。「激」か「迷」だった者は、大破に巻き込まれ、2D6点のダメージを受ける（セーブ不可）。",
    "「ひどい運転しやがって！」死んだかと思った乗物が幽霊自動車になって、襲いかかってくる。",
    "「今まで一緒に乗ってくれてありがとう」乗物が、最後にきみに語りかけてくる。炎上する乗物を眺めながら、思わず涙が流れる。乗物は破壊。乗物の持ち主は、〔精神点〕を2点回復。",
    "ハンドルがきかず、人をひいてしまう。乗物は破壊。〔犯罪〕で難易度9の判定に失敗すると「臭い飯表」を1回振ること。",
    "コロコロコロコロ……車輪が転がる。ダメだ。もう一歩も動かない。乗物が破壊される。",
    "壮絶なクラッシュ！！　乗物とその乗物に乗せていたアイテムがすべて破壊される。",
    "乗物に衝撃が走る！　〔精神〕で難易度9の判定を行う。失敗すると、その乗物に乗せていたアイテムがすべて破壊される。乗物は破壊される。",
    "バッテリーがあがってしまった。乗物が一時的に使用不能に。1ターン後、乗物の〔肉体点〕が1になる。",
    "「おい！　走ってくれ！　走ってくれよ！」乗物が一時的に使用不能に。1D6ラウンド後、〔肉体点〕が1になる。",
    "エンスト！　乗物が一時的に使用不能に。1D6ターン後、〔肉体点〕が1D6点回復する。",
    "「まだ走れるよ！」奇跡のような走り！　さきほどのダメージを無効に。",
];

/// Ruby `TABLES["FATALVT"]`（`2D6`）。
static TBL_FATALVT: Table = Table::from_dice("乗物致命傷表", 2, 6, TBL_FATALVT_ITEMS);

/// Ruby `TABLES["TIMEUT"]`（時間切れ表）の項目。
static TBL_TIMEUT_ITEMS: &[&str] = &[
    "は！　夢か。今までのことは夢だった。すべて世はこともなし。",
    "UFOが現れ、トラクタービームに牽引される。全員〔精神〕で難易度9の判定を行う。失敗したキャラクターは、1D6セッション別のキャラクターでセッションを行うまで、再利用できなくなる。",
    "まわりでバタバタと人が倒れ始める。新型インフルエンザウイルスが、知性を持ち始め、突如人類に反旗を翻す。全員、〔肉体〕で難易度9の判定を行う。失敗したキャラクターは、〔肉体〕が1点減少する。",
    "急に、家のガスコンロを消したかどうか気になり始める。全員、〔生活〕で難易度9の判定を行う。失敗したキャラクターは、本当に火を消し忘れていた。家が火事になり、〔アジト装備〕がすべて破壊される。",
    "突如、みんなが歌い踊り出す。全員、〔教養〕で難易度9の判定を行う。失敗したキャラクターは、リズムを外して、トラウマを1点受ける。",
    "内戦勃発！　派手な市街戦が開始される。全員、〔戦闘〕で難易度9の判定を行う。失敗したキャラクターは、2D6点のダメージを受ける。内戦は3日後に終結する。",
    "いやーん、まいっちんぐ。200人の裸の美女が目の前を走りさっていく。一体何が起きたんだろう？　全員、〔恋愛〕で難易度9の判定を行う。失敗したキャラクターは、いつの間にか、その集団に呑み込まれ……トラウマを1点受ける。",
    "ビルの上から、大量のお札が降ってくる。皆、我を忘れて、それに群がり始める。全員、〔犯罪〕で難易度9の判定を行う。失敗したキャラクターは、〔通常装備〕欄からランダムにアイテム1つを失う。",
    "聖者が街にやってくる。「悔い改めよ！」全員、〔精神〕で難易度9の判定を行う。失敗したキャラクターは、好きなカルマ1種類の異能と代償が1つずつ未修得の状態になる。",
    "地獄の釜が開く。街に死者たちがあふれ出す。全員〔肉体〕で難易度9の判定を行う。失敗したキャラクターは、屍人になる。",
    "たらら、たらら、たらららら、たらららら♪　大阪湾に怪獣王が現れる。大阪市は大混乱に！　全員、爆発4の効果を適用される。",
];

/// Ruby `TABLES["TIMEUT"]`（`2D6`）。
static TBL_TIMEUT: Table = Table::from_dice("時間切れ表", 2, 6, TBL_TIMEUT_ITEMS);

/// Ruby `TABLES`（キーは `transform_keys(&:upcase)` 済み）。
static TABLES: &[(&str, &Table)] = &[
    ("CRIMEIET", &TBL_CRIMEIET),
    ("LIFEIET", &TBL_LIFEIET),
    ("LOVEIET", &TBL_LOVEIET),
    ("CULTUREIET", &TBL_CULTUREIET),
    ("COMBATIET", &TBL_COMBATIET),
    ("CRIMEIHT", &TBL_CRIMEIHT),
    ("LIFEIHT", &TBL_LIFEIHT),
    ("LOVEIHT", &TBL_LOVEIHT),
    ("CULTUREIHT", &TBL_CULTUREIHT),
    ("COMBATIHT", &TBL_COMBATIHT),
    ("GENERALACCIDENTT", &TBL_GENERALACCIDENTT),
    ("ROMANCEFUMBLET", &TBL_ROMANCEFUMBLET),
    ("FUMBLET", &TBL_FUMBLET),
    ("FATALT", &TBL_FATALT),
    ("ACCIDENTT", &TBL_ACCIDENTT),
    ("AFTERT", &TBL_AFTERT),
    ("KUSAIMT", &TBL_KUSAIMT),
    ("ENTERT", &TBL_ENTERT),
    ("BUDTT", &TBL_BUDTT),
    ("GETGT", &TBL_GETGT),
    ("GETZT", &TBL_GETZT),
    ("GETNT", &TBL_GETNT),
    ("GETKT", &TBL_GETKT),
    ("PAYT", &TBL_PAYT),
    ("MINAMIRET", &TBL_MINAMIRET),
    ("CHINATOWNRET", &TBL_CHINATOWNRET),
    ("WARSHIPLANDRET", &TBL_WARSHIPLANDRET),
    ("CIVICCENTERRET", &TBL_CIVICCENTERRET),
    ("DOWNTOWNRET", &TBL_DOWNTOWNRET),
    ("SHAOKINRET", &TBL_SHAOKINRET),
    ("LOVELOVERET", &TBL_LOVELOVERET),
    ("AJITORET", &TBL_AJITORET),
    ("JIGOKUSPARET", &TBL_JIGOKUSPARET),
    ("JAILHOUSERET", &TBL_JAILHOUSERET),
    ("TREATMENTIT", &TBL_TREATMENTIT),
    ("COLLEGEIT", &TBL_COLLEGEIT),
    ("FATALVT", &TBL_FATALVT),
    ("TIMEUT", &TBL_TIMEUT),
];

/// Ruby `ALIASES`（キー・値とも `upcase` 済み）。
static ALIASES: &[(&str, &str)] = &[
    ("RFT", "ROMANCEFUMBLET"),
    ("GAT", "GENERALACCIDENTT"),
    ("ROMANCEFT", "ROMANCEFUMBLET"),
    ("GENERALAT", "GENERALACCIDENTT"),
    ("RFUMBLET", "ROMANCEFUMBLET"),
    ("GACCIDENTT", "GENERALACCIDENTT"),
];

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;

    fn toml_path() -> Option<PathBuf> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .join("test/data/Satasupe.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `TABLES` の全表が 2D6 の全出目（2〜12）で項目を返すこと。
    ///
    /// TOMLが引くのは各表のごく一部なので、項目の取りこぼしはここで検出する。
    #[test]
    fn tables_cover_every_roll() {
        assert_eq!(TABLES.len(), 38, "TABLES の件数");

        for (key, table) in TABLES {
            assert_eq!((table.times(), table.sides()), (2, 6), "{key} のダイス種別");
            // Ruby `Table#choice` は `items[value - times]` なので項目数は times*sides-times+1
            for value in table.times()..=(table.times() * table.sides()) {
                assert!(
                    !table.choice(value).last_body().is_empty(),
                    "{key} の出目 {value} に対応する項目が無い"
                );
            }
            // 余分な項目が付いていないこと（末尾の1つ先が空であること）
            assert!(
                table
                    .choice(table.times() * table.sides() + 1)
                    .last_body()
                    .is_empty(),
                "{key} に余分な項目がある"
            );
        }
    }

    /// Ruby `ALIASES` の参照先が `TABLES` に存在すること。
    #[test]
    fn aliases_resolve_to_tables() {
        for (alias, target) in ALIASES {
            assert!(
                TABLES.iter().any(|(key, _)| key == target),
                "{alias} → {target} が TABLES に無い"
            );
        }
    }

    /// Ruby `register_prefix(..., TABLES.keys, ALIASES.keys)` と接頭辞一覧が一致すること。
    #[test]
    fn prefixes_cover_all_table_commands() {
        let prefixes = Satasupe.prefixes();
        for (key, _) in TABLES {
            assert!(prefixes.contains(key), "接頭辞に {key} が無い");
        }
        for (alias, _) in ALIASES {
            assert!(prefixes.contains(alias), "接頭辞に {alias} が無い");
        }
    }

    /// Ruby `CREATE_ARMS_ACCESSORY_TABLE` が昇順D66の全21通りを覆うこと。
    #[test]
    fn accessory_table_covers_ascending_d66() {
        assert_eq!(CREATE_ARMS_ACCESSORY_TABLE.len(), 21);
        for d1 in 1..=6 {
            for d2 in d1..=6 {
                let key = d1 * 10 + d2;
                assert!(
                    CREATE_ARMS_ACCESSORY_TABLE.iter().any(|a| a.key == key),
                    "D66 {key} が無い"
                );
            }
        }
    }

    /// 先頭の `S`（シークレットダイス）が剥がされて固有コマンドへ渡ること。
    ///
    /// TOMLに `secret = true` のケースが1件も無いので、ここで押さえておく。
    /// 接頭辞 `SR` と、`S` で始まる表コマンド `SHAOKINRET` の両方を見る
    /// （`^(S)?(…)` の `(S)?` が貪欲でも、表名側が一致しなければバックトラックして
    /// 空マッチに落ちる。Ruby の後戻り探索と同じ挙動になっていることの確認）。
    #[test]
    fn secret_prefix_is_stripped_and_propagated() {
        for (plain, secret, rands) in [
            ("SR6", "SSR6", vec![(3_i64, 6_i64), (2, 6)]),
            ("ShaokinRET", "SShaokinRET", vec![(1_i64, 6_i64), (1, 6)]),
        ] {
            let mut src = SeededRandomizer::new(rands.clone());
            let plain_result = eval_command(&GameSystemId::new("Satasupe"), plain, &mut src)
                .expect("eval")
                .expect("some result");
            assert!(!plain_result.secret, "{plain} は secret ではない");

            let mut src = SeededRandomizer::new(rands);
            let secret_result = eval_command(&GameSystemId::new("Satasupe"), secret, &mut src)
                .expect("eval")
                .expect("some result");
            assert!(secret_result.secret, "{secret} は secret になる");
            assert_eq!(
                secret_result.text, plain_result.text,
                "{secret} の出力は {plain} と同じ"
            );
        }
    }

    /// `test/data/Satasupe.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Satasupe.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Satasupe.toml must parse");
        assert_eq!(
            data.tests.len(),
            346,
            "case count in test/data/Satasupe.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Satasupe",
                "unexpected game system in Satasupe.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Satasupe"), &tc.input, &mut src) {
                Err(e) => reasons.push(format!("eval error: {e}")),
                Ok(None) => {
                    if !tc.expects_nil() {
                        reasons.push(format!(
                            "eval returned nil, but output was expected: {:?}",
                            tc.output
                        ));
                    }
                }
                Ok(Some(result)) => {
                    if tc.expects_nil() {
                        reasons.push(format!("expected nil output, got {:?}", result.text));
                    } else if result.text != tc.output {
                        reasons.push(format!(
                            "output mismatch\n    expected: {:?}\n    actual:   {:?}",
                            tc.output, result.text
                        ));
                    }
                    check_flag(&mut reasons, "secret", tc.secret, result.secret);
                    check_flag(&mut reasons, "success", tc.success, result.success);
                    check_flag(&mut reasons, "failure", tc.failure, result.failure);
                    check_flag(&mut reasons, "critical", tc.critical, result.critical);
                    check_flag(&mut reasons, "fumble", tc.fumble, result.fumble);
                }
            }

            if !src.is_empty() {
                reasons.push(format!("unconsumed rands remain ({})", src.remaining()));
            }

            if !reasons.is_empty() {
                failures.push(format!(
                    "FAIL Satasupe:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Satasupe cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
