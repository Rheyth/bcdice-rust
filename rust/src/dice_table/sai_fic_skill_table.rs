//! Ruby `BCDice::DiceTable::SaiFicSkillTable`
//! （lib/bcdice/dice_table/sai_fic_skill_table.rb とその配下）の移植。
//!
//! サイコロ・フィクション系のランダム特技表。分野（1D6）×特技（2D6）で特技を選ぶ。

use crate::eval::EvalError;
use crate::randomizer::Randomizer;

/// Ruby `SaiFicSkillTable::RTTN` 定数。
pub const RTTN: [&str; 6] = ["RTT1", "RTT2", "RTT3", "RTT4", "RTT5", "RTT6"];

/// Ruby `DEFAULT_RTT`。
pub const DEFAULT_RTT_FORMAT: &str = "ランダム特技表(%<category_dice>d,%<row_dice>d) ＞ %<text>s";
/// Ruby `DEFAULT_RCT`。
pub const DEFAULT_RCT_FORMAT: &str = "ランダム分野表(%<category_dice>d) ＞ %<category_name>s";
/// Ruby `DEFAULT_RTTN`。
pub const DEFAULT_RTTN_FORMAT: &str =
    "%<category_name>s分野ランダム特技表(%<row_dice>d) ＞ %<text>s";
/// Ruby `DEFAULT_S`。
pub const DEFAULT_SKILL_FORMAT: &str = "《%<skill_name>s／%<category_name>s%<row_dice>d》";

/// 出力書式一式。
///
/// Ruby は `initialize` のキーワード引数として個別に受け取るが、
/// 引数を増やしすぎないようひとまとめにした。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SaiFicFormats {
    /// Ruby `rtt_format`
    pub rtt: &'static str,
    /// Ruby `rct_format`
    pub rct: &'static str,
    /// Ruby `rttn_format`
    pub rttn: &'static str,
    /// Ruby `s_format`（`Skill#to_s` 用）
    pub skill: &'static str,
}

impl SaiFicFormats {
    /// Ruby の既定書式。
    pub const DEFAULT: Self = Self {
        rtt: DEFAULT_RTT_FORMAT,
        rct: DEFAULT_RCT_FORMAT,
        rttn: DEFAULT_RTTN_FORMAT,
        skill: DEFAULT_SKILL_FORMAT,
    };
}

impl Default for SaiFicFormats {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// 特技。Ruby `SaiFicSkillTable::Skill`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SaiFicSkill {
    /// 分野名
    pub category_name: &'static str,
    /// 特技名
    pub name: &'static str,
    /// 分野を決めた1D6の出目（1始まり）
    pub category_dice: i64,
    /// 特技を決めた2D6の出目（2始まり）
    pub row_dice: i64,
}

impl SaiFicSkill {
    /// Ruby `Skill#to_s`。
    pub fn format_with(&self, s_format: &str) -> String {
        format_named(
            s_format,
            &[
                ("category_dice", FormatArg::Int(self.category_dice)),
                ("row_dice", FormatArg::Int(self.row_dice)),
                ("category_name", FormatArg::Str(self.category_name)),
                ("skill_name", FormatArg::Str(self.name)),
            ],
        )
    }
}

/// 分野。Ruby `SaiFicSkillTable::Category`。
///
/// Ruby は分野の位置（1始まり）を `@dice` として保持するが、こちらは
/// 表内での位置から [`SaiFicSkillTable`] が渡す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SaiFicCategory {
    name: &'static str,
    skills: &'static [&'static str],
}

impl SaiFicCategory {
    /// Ruby `Category.new(name, skills, dice, s_format)` のうち、静的な部分。
    pub const fn new(name: &'static str, skills: &'static [&'static str]) -> Self {
        Self { name, skills }
    }

    /// Ruby `Category#name`。
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Ruby `Category#skills`。
    pub fn skills(&self) -> &'static [&'static str] {
        self.skills
    }

    /// Ruby `Category#roll(randomizer)`: `skills[roll_sum(2, 6) - 2]`。
    ///
    /// `dice` はこの分野の1始まりの位置（Ruby の `@dice`）。
    pub fn roll(&self, rng: &mut Randomizer, dice: i64) -> Result<Option<SaiFicSkill>, EvalError> {
        let row_dice = rng.roll_sum(2, 6)?;
        Ok(self.skill_at(dice, row_dice))
    }

    /// 2D6の出目に対応する特技を返す。範囲外は `None`。
    pub fn skill_at(&self, dice: i64, row_dice: i64) -> Option<SaiFicSkill> {
        let name = usize::try_from(row_dice - 2)
            .ok()
            .and_then(|i| self.skills.get(i))
            .copied()?;
        Some(SaiFicSkill {
            category_name: self.name,
            name,
            category_dice: dice,
            row_dice,
        })
    }
}

/// サイコロ・フィクション用ランダム特技表。Ruby `SaiFicSkillTable`。
#[derive(Debug, Clone, Copy)]
pub struct SaiFicSkillTable {
    categories: &'static [SaiFicCategory],
    rtt: Option<&'static str>,
    rct: Option<&'static str>,
    rttn: &'static [&'static str],
    formats: SaiFicFormats,
}

impl SaiFicSkillTable {
    /// Ruby `SaiFicSkillTable.new(items)`（コマンド・書式はすべて既定）。
    pub const fn new(categories: &'static [SaiFicCategory]) -> Self {
        Self {
            categories,
            rtt: None,
            rct: None,
            rttn: &[],
            formats: SaiFicFormats::DEFAULT,
        }
    }

    /// Ruby `rtt:` / `rct:` / `rttn:` キーワード引数（別名コマンド）を指定する。
    pub const fn with_commands(
        mut self,
        rtt: Option<&'static str>,
        rct: Option<&'static str>,
        rttn: &'static [&'static str],
    ) -> Self {
        self.rtt = rtt;
        self.rct = rct;
        self.rttn = rttn;
        self
    }

    /// Ruby `rtt_format:` などの書式指定をまとめて上書きする。
    pub const fn with_formats(mut self, formats: SaiFicFormats) -> Self {
        self.formats = formats;
        self
    }

    /// Ruby `#categories`。
    pub fn categories(&self) -> &'static [SaiFicCategory] {
        self.categories
    }

    /// Ruby `#prefixes`: `(["RTT[1-6]?", "RCT", @rtt, @rct] + @rttn).compact`。
    pub fn prefixes(&self) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = vec!["RTT[1-6]?", "RCT"];
        out.extend(self.rtt);
        out.extend(self.rct);
        out.extend(self.rttn.iter().copied());
        out
    }

    /// Ruby `#roll_category(randomizer)`: 1D6で分野を決める。
    ///
    /// 戻り値は `(1始まりの出目, 分野)`。範囲外の出目では `None`。
    pub fn roll_category(
        &self,
        rng: &mut Randomizer,
    ) -> Result<Option<(i64, &'static SaiFicCategory)>, EvalError> {
        let dice = rng.roll_once(6)?;
        Ok(usize::try_from(dice - 1)
            .ok()
            .and_then(|i| self.categories.get(i))
            .map(|c| (dice, c)))
    }

    /// Ruby `#roll_skill(randomizer)`: 1D6と2D6でランダムに特技を決める。
    pub fn roll_skill(&self, rng: &mut Randomizer) -> Result<Option<SaiFicSkill>, EvalError> {
        let Some((dice, category)) = self.roll_category(rng)? else {
            return Ok(None);
        };
        category.roll(rng, dice)
    }

    /// Ruby `#roll_command(randomizer, command)`。
    ///
    /// `RTT` / `RCT` / `RTT1`〜`RTT6`（および別名）を解釈する。
    /// 解釈できないコマンドでは `None`（ダイスも振らない）。
    pub fn roll_command(
        &self,
        rng: &mut Randomizer,
        command: &str,
    ) -> Result<Option<String>, EvalError> {
        if command == "RTT" || self.rtt == Some(command) {
            let Some(skill) = self.roll_skill(rng)? else {
                return Ok(None);
            };
            return Ok(Some(self.format_skill(self.formats.rtt, &skill)));
        }

        if command == "RCT" || self.rct == Some(command) {
            let Some((dice, category)) = self.roll_category(rng)? else {
                return Ok(None);
            };
            return Ok(Some(format_named(
                self.formats.rct,
                &[
                    ("category_dice", FormatArg::Int(dice)),
                    ("category_name", FormatArg::Str(category.name())),
                ],
            )));
        }

        // Ruby: (index = RTTN.index(c)) || (index = @rttn.index(c))
        let index = RTTN
            .iter()
            .position(|c| *c == command)
            .or_else(|| self.rttn.iter().position(|c| *c == command));
        let Some(index) = index else {
            return Ok(None);
        };
        let Some(category) = self.categories.get(index) else {
            return Ok(None);
        };
        // Ruby の Category#roll は自分の @dice（1始まりの位置）を使う
        let Some(skill) = category.roll(rng, index as i64 + 1)? else {
            return Ok(None);
        };
        Ok(Some(self.format_skill(self.formats.rttn, &skill)))
    }

    /// Ruby private `#format_skill(format_string, skill)`。
    fn format_skill(&self, template: &str, skill: &SaiFicSkill) -> String {
        format_named(
            template,
            &[
                ("category_dice", FormatArg::Int(skill.category_dice)),
                ("row_dice", FormatArg::Int(skill.row_dice)),
                ("category_name", FormatArg::Str(skill.category_name)),
                ("skill_name", FormatArg::Str(skill.name)),
                (
                    "text",
                    FormatArg::Owned(skill.format_with(self.formats.skill)),
                ),
            ],
        )
    }
}

/// [`format_named`] に渡す値。
enum FormatArg {
    Int(i64),
    Str(&'static str),
    Owned(String),
}

impl FormatArg {
    fn render(&self) -> std::borrow::Cow<'_, str> {
        match self {
            FormatArg::Int(v) => std::borrow::Cow::Owned(v.to_string()),
            FormatArg::Str(s) => std::borrow::Cow::Borrowed(s),
            FormatArg::Owned(s) => std::borrow::Cow::Borrowed(s.as_str()),
        }
    }
}

/// Ruby `Kernel#format` の名前付き参照（`%<name>d` / `%{name}`）だけを解釈する簡易版。
///
/// 変換指定は `%<name>` の直後の1文字だけを読み飛ばす。Ruby の `format` はフラグ・幅
/// （`%<n>02d` 等）も許すが、本家の `i18n/` と `lib/` に実在する `%<name>` 全139件を
/// 確認したところ、直後は `d`（72件）か `s`（67件）の1文字のみで幅指定は存在しない
/// （2026-08-30 実測）。将来 `%<n>02d` 形式が入ったら読み飛ばしを
/// `[-+ 0#]*\d*(\.\d+)?[dis]` 相当へ広げること。
///
/// 値の描画は変換指定によらず「整数なら10進表記、文字列ならそのまま」。
/// Ruby も `%<int>s` を `"3"` にするので一致する。
/// 未知の名前は Ruby では `KeyError` になるが、ここでは元の文字列を残す。
fn format_named(template: &str, args: &[(&str, FormatArg)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;

    while let Some(pos) = rest.find('%') {
        out.push_str(&rest[..pos]);
        rest = &rest[pos..];

        let mut chars = rest.char_indices();
        chars.next(); // '%'
        match chars.next() {
            // "%%" -> "%"
            Some((_, '%')) => {
                out.push('%');
                rest = &rest['%'.len_utf8() * 2..];
            }
            // "%<name>X"
            Some((open, '<')) => {
                let after = &rest[open + '<'.len_utf8()..];
                match after.find('>') {
                    // 変換文字1つ（d / i / s）を読み飛ばす
                    Some(close) if close + 1 < after.len() => {
                        let name = &after[..close];
                        out.push_str(&lookup(args, name));
                        rest = &after[close + '>'.len_utf8() + 1..];
                    }
                    _ => {
                        out.push('%');
                        rest = &rest['%'.len_utf8()..];
                    }
                }
            }
            // "%{name}"
            Some((open, '{')) => {
                let after = &rest[open + '{'.len_utf8()..];
                match after.find('}') {
                    Some(close) => {
                        let name = &after[..close];
                        out.push_str(&lookup(args, name));
                        rest = &after[close + '}'.len_utf8()..];
                    }
                    None => {
                        out.push('%');
                        rest = &rest['%'.len_utf8()..];
                    }
                }
            }
            _ => {
                out.push('%');
                rest = &rest['%'.len_utf8()..];
            }
        }
    }

    out.push_str(rest);
    out
}

/// 名前から値を引く。未知の名前は元の記法を残す。
fn lookup(args: &[(&str, FormatArg)], name: &str) -> String {
    match args.iter().find(|(k, _)| *k == name) {
        Some((_, v)) => v.render().into_owned(),
        None => format!("%<{name}>"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::randomizer::SeededRandomizer;

    static SKILLS_1: &[&str] = &[
        "器術2", "器術3", "器術4", "器術5", "器術6", "器術7", "器術8", "器術9", "器術10", "器術11",
        "器術12",
    ];
    static SKILLS_2: &[&str] = &[
        "体術2", "体術3", "体術4", "体術5", "体術6", "体術7", "体術8", "体術9", "体術10", "体術11",
        "体術12",
    ];
    static CATEGORIES: &[SaiFicCategory] = &[
        SaiFicCategory::new("器術", SKILLS_1),
        SaiFicCategory::new("体術", SKILLS_2),
    ];
    static TABLE: SaiFicSkillTable = SaiFicSkillTable::new(CATEGORIES);

    fn run(table: &SaiFicSkillTable, command: &str, rands: &[(i64, i64)]) -> Option<String> {
        let mut src = SeededRandomizer::new(rands.to_vec());
        let mut rng = Randomizer::new(&mut src);
        let out = table.roll_command(&mut rng, command).expect("roll");
        assert!(src.is_empty(), "unconsumed rands");
        out
    }

    #[test]
    fn rtt_rolls_category_then_skill() {
        // 1D6=2（体術）、2D6=3+4=7 → skills[7-2]=体術7
        assert_eq!(
            run(&TABLE, "RTT", &[(2, 6), (3, 6), (4, 6)]).as_deref(),
            Some("ランダム特技表(2,7) ＞ 《体術7／体術7》")
        );
    }

    #[test]
    fn rct_rolls_category_only() {
        assert_eq!(
            run(&TABLE, "RCT", &[(1, 6)]).as_deref(),
            Some("ランダム分野表(1) ＞ 器術")
        );
    }

    #[test]
    fn rttn_selects_category_by_command() {
        // RTT2 は2番目の分野（体術）。分野ダイスは振らない。
        assert_eq!(
            run(&TABLE, "RTT2", &[(6, 6), (6, 6)]).as_deref(),
            Some("体術分野ランダム特技表(12) ＞ 《体術12／体術12》")
        );
    }

    #[test]
    fn unknown_command_rolls_nothing() {
        assert_eq!(run(&TABLE, "XX", &[]), None);
    }

    #[test]
    fn aliases_are_accepted() {
        static ALIASED: SaiFicSkillTable = SaiFicSkillTable::new(CATEGORIES).with_commands(
            Some("RT"),
            Some("RC"),
            &["RT1", "RT2"],
        );
        assert_eq!(
            run(&ALIASED, "RC", &[(2, 6)]).as_deref(),
            Some("ランダム分野表(2) ＞ 体術")
        );
        assert_eq!(
            run(&ALIASED, "RT1", &[(1, 6), (1, 6)]).as_deref(),
            Some("器術分野ランダム特技表(2) ＞ 《器術2／器術2》")
        );
        // 既定のコマンド名も引き続き使える
        assert!(run(&ALIASED, "RTT", &[(1, 6), (1, 6), (1, 6)]).is_some());
    }

    #[test]
    fn prefixes_match_ruby() {
        assert_eq!(TABLE.prefixes(), vec!["RTT[1-6]?", "RCT"]);

        static ALIASED: SaiFicSkillTable = SaiFicSkillTable::new(CATEGORIES).with_commands(
            Some("RT"),
            Some("RC"),
            &["RT1", "RT2"],
        );
        assert_eq!(
            ALIASED.prefixes(),
            vec!["RTT[1-6]?", "RCT", "RT", "RC", "RT1", "RT2"]
        );
    }

    #[test]
    fn custom_formats_are_used() {
        static FORMATS: SaiFicFormats = SaiFicFormats {
            rtt: "特技: %<text>s",
            rct: "分野: %<category_name>s(%<category_dice>d)",
            rttn: DEFAULT_RTTN_FORMAT,
            skill: "[%<skill_name>s]",
        };
        static CUSTOM: SaiFicSkillTable = SaiFicSkillTable::new(CATEGORIES).with_formats(FORMATS);
        assert_eq!(
            run(&CUSTOM, "RTT", &[(1, 6), (1, 6), (1, 6)]).as_deref(),
            Some("特技: [器術2]")
        );
        assert_eq!(
            run(&CUSTOM, "RCT", &[(2, 6)]).as_deref(),
            Some("分野: 体術(2)")
        );
    }

    #[test]
    fn format_named_handles_ruby_forms() {
        let args = [("n", FormatArg::Int(7)), ("s", FormatArg::Str("あ"))];
        assert_eq!(format_named("%<n>d/%<s>s", &args), "7/あ");
        assert_eq!(format_named("%{n}と%{s}", &args), "7とあ");
        assert_eq!(format_named("100%%", &args), "100%");
        assert_eq!(format_named("書式なし", &args), "書式なし");
        // 未知の名前は元の記法を残す（Ruby は KeyError）
        assert_eq!(format_named("%<x>d", &args), "%<x>");
    }
}
