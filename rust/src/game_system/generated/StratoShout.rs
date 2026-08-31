//! P4で手書き移植した `lib/bcdice/game_system/StratoShout.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `StratoShout#result_2d6`（2以下でファンブル、12以上でスペシャル。それ以外は `nil`）
//! - `#eval_game_system_specific_command` → `roll_tables` と `RTT.roll_command`
//! - `TABLES`（トラブル表・感情表・シーン表・シーン展開表）と `RTT`（ランダム特技表）
//!
//! # 表データ
//!
//! Ruby側は `I18n.t("StratoShout.…", locale:)` で `i18n/StratoShout/ja_jp.yml` から表を作る。
//! Rust側は同じ値を `static` として直接持つ。データ部分（`JA_` 接頭辞の `static` 群）は
//! 同YAMLから機械的に書き出したもので、値は1文字も変えていない。
//!
//! ロケール差のあるデータは [`SystemTables`] に束ね、
//! `StratoShout_Korean`（`ko_kr`）が同じ関数群を使い回す。

use crate::dice_table::sai_fic_skill_table::{DEFAULT_RCT_FORMAT, DEFAULT_SKILL_FORMAT};
use crate::dice_table::{
    D66Table, RollableTable, SaiFicCategory, SaiFicFormats, SaiFicSkillTable, Table, TableItem,
};
use crate::enums::D66SortType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::{CheckOutcome, EvalResult};
use crate::Int as I;

// ---------------------------------------------------------------------------
// ロケールごとの表と定型文
// ---------------------------------------------------------------------------

/// 1ロケール分の表と定型文。`StratoShout` と `StratoShout_Korean` はこれだけが違う。
pub(crate) struct SystemTables {
    /// Ruby `TABLES`
    pub(crate) tables: &'static [(&'static str, &'static dyn RollableTable)],
    /// Ruby `RTT`
    pub(crate) rtt: &'static SaiFicSkillTable,
    /// i18n `StratoShout.critical`
    pub(crate) critical: &'static str,
    /// i18n `StratoShout.fumble`
    pub(crate) fumble: &'static str,
    /// i18n `success`（`Base#result_ndx` が使う）
    pub(crate) success: &'static str,
    /// i18n `failure`（同上）
    pub(crate) failure: &'static str,
}

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `StratoShout#result_2d6`。
///
/// ファンブル・スペシャルだけを返し、それ以外は `nil` を返して
/// `Base#result_ndx` の汎用比較（成功/失敗）に任せる。
pub(crate) fn check_result_2d6(
    sys: &SystemTables,
    dice_total: crate::Int,
    cmp_op: CmpOp,
) -> Option<CheckOutcome> {
    // Ruby: return nil unless cmp_op == :>=
    if cmp_op != CmpOp::Ge {
        return None;
    }

    if dice_total <= I::from(2) {
        Some(CheckOutcome::Result(Box::new(EvalResult::fumble(
            sys.fumble,
        ))))
    } else if dice_total >= I::from(12) {
        Some(CheckOutcome::Result(Box::new(EvalResult::critical(
            sys.critical,
        ))))
    } else {
        // Ruby: if/elsif がどちらも偽なら nil
        None
    }
}

/// Ruby `StratoShout#eval_game_system_specific_command`。
///
/// `roll_tables(command, TABLES) || RTT.roll_command(@randomizer, command)`。
pub(crate) fn eval_specific_command(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(text) = roll_tables(sys.tables, command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    Ok(sys
        .rtt
        .roll_command(rng, command)?
        .map(SpecificCommandOutput::text))
}

/// Ruby `Base#roll_tables(command, tables)`。
fn roll_tables(
    tables: &'static [(&'static str, &'static dyn RollableTable)],
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    match tables.iter().find(|(key, _)| *key == command) {
        None => Ok(None),
        Some((_, table)) => Ok(Some(table.roll(rng)?.to_string())),
    }
}

/// Ruby `Base#result_ndx`（ロケールの定型文で）。
///
/// トレイトの既定実装は `ja_jp` 固定なので、`ko_kr` 側から共有できるよう
/// ここに文言を受け取る形で置いておく。
pub(crate) fn result_ndx(
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

// ---------------------------------------------------------------------------
// ja_jp ロケールの表と定型文
// ---------------------------------------------------------------------------

/// i18n `StratoShout.table.VOT`（ボーカルトラブル表(P167)）。
static JA_TABLE_VOT: Table = Table::from_dice(
    "ボーカルトラブル表(P167)",
    1,
    6,
    &[
        "歌詞を忘れてしまった！ 何も言葉が出てこない……",
        "マイクのコードに足を引っ掛けてしまった！ 危ない！",
        "マイクスタンドが倒れてしまった！",
        "音程がズレているけど、なかなか直せない！",
        "リズムがズレてきている気がする……修正できない！",
        "喉が枯れそうだ。まずい、セーブしないと……！",
    ],
);

/// i18n `StratoShout.table.GUT`（ギタートラブル表(P169)）。
static JA_TABLE_GUT: Table = Table::from_dice(
    "ギタートラブル表(P169)",
    1,
    6,
    &[
        "やべっ、コードを間違えた！ どうにかごまかそう……",
        "ゲッ、シールド(信号を伝えるコード)が抜けちゃった！ 音が出ない！",
        "ギターの音にノイズが乗ってるような……直ってくれ……！",
        "あれ？ 今曲のどの辺りだっけ……？",
        "弦が切れてしまった！ なんて不吉な……。",
        "ピックが飛んでった！ 指で弾くしかない……！",
    ],
);

/// i18n `StratoShout.table.BAT`（ベーストラブル表(P171)）。
static JA_TABLE_BAT: Table = Table::from_dice(
    "ベーストラブル表(P171)",
    1,
    6,
    &[
        "やべっ、コードを間違えた！ どうにかごまかそう……",
        "ゲッ、シールド(信号を伝えるコード)が抜けちゃった！ 音が出ない！",
        "ベースの音にノイズが乗ってるような……直ってくれ……！",
        "あれ？ 今曲のどの辺りだっけ……？",
        "指先の感覚が麻痺してきた。動かない……！",
        "テンポが速くなってきているけど、止まらない！",
    ],
);

/// i18n `StratoShout.table.KEYT`（キーボードトラブル表(P173)）。
static JA_TABLE_KEYT: Table = Table::from_dice(
    "キーボードトラブル表(P173)",
    1,
    6,
    &[
        "指先の感覚が麻痺してきた。動かない……！",
        "音量のスライドに触れてしまった！ 爆音が出てしまう！",
        "あれ？ 今曲のどの辺りだっけ……？",
        "音の出ない鍵がある……故障！？",
        "音色を間違えた！ 元の音色は何番だっけ……！？",
        "手を置く位置が一つズレてる……！ 不協和音だ！",
    ],
);

/// i18n `StratoShout.table.DRT`（ドラムトラブル表(P175)）。
static JA_TABLE_DRT: Table = Table::from_dice(
    "ドラムトラブル表(P175)",
    1,
    6,
    &[
        "手がこんがらがってきた！ 軌道修正しないと……！",
        "あれ？ 今曲のどの辺りだっけ……？",
        "ハイハットが開かない！ ネジが緩んでるのか……！？",
        "アドリブ入れたけど、次のフレーズが思いつかない……！",
        "テンポが速くなってきているけど、止まらない！",
        "スティックが飛んでった！ 代わりはどこだっけ……。",
    ],
);

/// i18n `StratoShout.table.EMO`（感情表(P183)）。
static JA_TABLE_EMO: Table = Table::from_dice(
    "感情表(P183)",
    1,
    6,
    &[
        "共感/不信",
        "友情/嫉妬",
        "好敵手/苛立ち",
        "不可欠/敬遠",
        "尊敬/劣等感",
        "愛情/負い目",
    ],
);

/// i18n `StratoShout.table.SCENE`（シーン表(P199)）。
static JA_TABLE_SCENE: Table = Table::from_dice(
    "シーン表(P199)",
    2,
    6,
    &[
        "一人の時間。ふと過去の記憶を辿る。そういえば以前、あんなことがあったような……。",
        "どこからか、言い争っているような声が聞こえてきた。喧嘩だろうか？",
        "夜の帳が下り、辺りは静寂に包まれている。あいつは今、何をしているだろう。",
        "仲間と一緒にご飯を食べていると、会話は自然とあの話に……。",
        "笑い声に満ちた空間。ずっとこんな時間が続けばいいのに。",
        "日の当たる場所。毎日の忙しさを離れ、穏やかな時間が過ぎていく。",
        "スマートフォンに着信の通知がやって来た。電話？ メッセージ？ 誰からだろう。",
        "突然、あなたのもとに来訪者が現れた。何か伝えたいことがあるようだ。",
        "誰かの忘れ物を見つけた。届けてあげたほうがいいだろうか。",
        "誰かが噂話をしている。聞くつもりはなくとも、それは勝手に耳に入ってきた。",
        "なんだか悪寒がする。なにか良くないことが起きているような……。",
    ],
);

/// i18n `StratoShout.table.MACHI`（街角シーン表(P199)）。
static JA_TABLE_MACHI: Table = Table::from_dice(
    "街角シーン表(P199)",
    2,
    6,
    &[
        "入ったことのない場所に、初めて足を踏み入れた。少し緊張してしまうな。",
        "アルバイト先。バイト仲間から、意外なことを教えられた。",
        "会話もままならないような、大音量の音楽。その場にいるだけで気分が高揚する。",
        "横断歩道で信号を待っていると、見知った人物の姿を見つけた。",
        "突然の雨に、慌てて足を早める人々。自分も早く帰らなければ。",
        "何気なく立ち寄った店の中で、知人とばったり。こんなところで何を？",
        "練習を終えて立ち寄った飲食店で、意外な人物を発見。少し様子を見てみよう。",
        "あちこちから子どもたちのはしゃぎ声が聞こえてくる。自分にもあんな頃があったんだろうか。",
        "音のない、静寂の世界。たまには音から離れるのもいいものだ。",
        "電車の中。つり革に掴まりながら揺られていると、見覚えのある乗客を見つけた。",
        "カラオケの廊下を歩いていると、どこからか聞き覚えのある声が……？",
    ],
);

/// i18n `StratoShout.table.GAKKO`（学校シーン表(P199)）。
static JA_TABLE_GAKKO: Table =
    Table::from_dice("学校シーン表(P199)", 2, 6, &[
        "校舎裏、何かを話す二人組を見かけた。一体何を話しているのだろう……？",
        "とある部室。部員たちは集中して部活に励んでいるようだが……。",
        "先生から、ターゲットについて尋ねられた。なにか気になることがあるようだ。",
        "木々の隙間から朝日差し込む通学路。ある者は忙しそうに、またある者は楽しそうに校舎へ向かっている。",
        "休み時間。教室のあちこちで飛び交う、他愛のない噂話。その中から、気になる会話が聞こえてきた。",
        "何もかもが茜色に染まる夕暮れ時。生徒たちは学業から解放され、自由に残り少ない一日を過ごしている。",
        "移動教室だ。渡り廊下から下を見ると、見覚えのある人物がいた。",
        "昼休み。生徒は思い思いの場所で昼食を取っている。さて、自分はどこで食べようか。",
        "先生から頼まれごとを引き受けてしまった。さっさと済ませてしまおう。",
        "そろそろ学校が閉まる時間だ。明かりの付いている教室はもうほとんどない。",
        "スピーカーから校内放送が聞こえてきた。誰かを呼んでいるようだが……？",
    ]);

/// i18n `StratoShout.table.BAND`（バンドシーン表(P199)）。
static JA_TABLE_BAND: Table = Table::from_dice(
    "バンドシーン表(P199)",
    2,
    6,
    &[
        "音楽専門のネットニュースサイトをチェック。大小様々な記事が投稿されている。",
        "意外なところで練習している人物を発見。少し声をかけてみようか。",
        "ちょっとした壁に衝突。誰かに相談したほうがいいかも……。",
        "ライブを見るためライブハウスへとやってきた。どんなステージになるのだろう。",
        "打ち合わせに行ったライブハウス。来ているのは自分たちだけじゃないようだ。",
        "練習が終わった帰り道。あいつも練習が終わった頃だろうか。",
        "どこからか楽器の音が聞こえてくる。誰か演奏しているのだろうか。",
        "熱気のこもる部屋を出て、スタジオの待合室でクールダウン。ソファに座っているのは……。",
        "訪れた楽器店で、見知った人物を発見。何をしに来ているのだろう。",
        "最新のヒット曲が流れるCDショップの店内。次はどんな曲をやろうか……。",
        "何気なく鳴らした音から、突発セッションに発展。軽く実力を見せつけてやろう。",
    ],
);

/// i18n `StratoShout.table.TENKAI`（シーン展開表(P201)）。
static JA_TABLE_TENKAI: D66Table =
    D66Table::new("シーン展開表(P201)", D66SortType::Asc, &[
        (11, TableItem::Text("絶望: ステップを更に大きくする、あるいはシーンプレイヤーを破滅に追い込むような状況に陥ります。【ディスコード】+2点。")),
        (12, TableItem::Text("崩壊: ステップによってシーンプレイヤーの大切なものが崩壊する、あるいは崩壊目前に追い込まれます。【ディスコード】+2点。")),
        (13, TableItem::Text("断絶: シーンプレイヤーはステップによって何かと絶縁状態になります。【ディスコード】+2点。")),
        (14, TableItem::Text("恐怖: ステップに恐怖するような出来事に遭遇します。【ディスコード】+2点。")),
        (15, TableItem::Text("誤解: シーンプレイヤーがステップに関するなんらかの誤解を受けます。【ディスコード】+2点。")),
        (16, TableItem::Text("試練: シーンプレイヤーはステップに関連した試練に直面します。【ディスコード】+2点。")),
        (22, TableItem::Text("悪心: シーンプレイヤーの心に魔が差し、ステップを不合理に解決しようとします。【ディスコード】+1点。")),
        (23, TableItem::Text("束縛: ステップに関わるなんらかに束縛され、自由な行動ができなくなります。【ディスコード】+1点。")),
        (24, TableItem::Text("凶兆: ステップについて、悪いことが起きそうな前触れが訪れます。【ディスコード】+1点。")),
        (25, TableItem::Text("加速: シーンプレイヤーはステップの解決に追われます。【ディスコード】+1点。")),
        (26, TableItem::Text("日常: シーンプレイヤーはのんびりとした日常を送ります。【コンディション】+1点。")),
        (33, TableItem::Text("休息: ステップを忘れられるような、穏やかなひとときを過ごします。【コンディション】+1点。")),
        (34, TableItem::Text("吉兆: ステップについて、いいことが起きそうな前触れが訪れます。【コンディション】+1点。")),
        (35, TableItem::Text("発見: シーンプレイヤーはステップについて何かを発見します。【コンディション】+1点。")),
        (36, TableItem::Text("希望: シーンプレイヤーの中に、ステップに対して前向きに取り組む意思が生まれます。【コンディション】+1点。")),
        (44, TableItem::Text("成長: ステップを通して、シーンプレイヤーが成長します。【コンディション】+2点。")),
        (45, TableItem::Text("愛情: ステップを通して、シーンプレイヤーが愛情に触れます。【コンディション】+2点。")),
        (46, TableItem::Text("朗報: ステップに関する良い知らせが舞い込みます。【コンディション】+2点。")),
        (55, TableItem::Text("好転: ステップが良い方向に向かうような事件が起きます。【コンディション】+3点。")),
        (56, TableItem::Text("直感: ステップを解決させる決定的な閃きを得ます。【コンディション】+3点。")),
        (66, TableItem::Text("奇跡: ステップに関して、奇跡的な幸運が舞い込みます。【コンディション】+3点。")),
    ]);

/// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表）。
static JA_TABLES: &[(&str, &dyn RollableTable)] = &[
    ("VOT", &JA_TABLE_VOT),
    ("GUT", &JA_TABLE_GUT),
    ("BAT", &JA_TABLE_BAT),
    ("KEYT", &JA_TABLE_KEYT),
    ("DRT", &JA_TABLE_DRT),
    ("EMO", &JA_TABLE_EMO),
    ("SCENE", &JA_TABLE_SCENE),
    ("MACHI", &JA_TABLE_MACHI),
    ("GAKKO", &JA_TABLE_GAKKO),
    ("BAND", &JA_TABLE_BAND),
    ("TENKAI", &JA_TABLE_TENKAI),
];

/// i18n `StratoShout.RTT.items[0]`（主義）。
static JA_RTT_SKILLS1: &[&str] = &[
    "過去", "恋人", "仲間", "家族", "自分", "今", "理由", "夢", "世界", "幸せ", "未来",
];
/// i18n `StratoShout.RTT.items[1]`（身体）。
static JA_RTT_SKILLS2: &[&str] = &[
    "頭", "目", "耳", "口", "胸", "心臓", "血", "背中", "手", "XXX", "足",
];
/// i18n `StratoShout.RTT.items[2]`（モチーフ）。
static JA_RTT_SKILLS3: &[&str] = &[
    "闇", "武器", "魔法", "獣", "町", "歌", "窓", "花", "空", "季節", "光",
];
/// i18n `StratoShout.RTT.items[3]`（情緒）。
static JA_RTT_SKILLS4: &[&str] = &[
    "悲しい",
    "怒り",
    "不安",
    "恐怖",
    "驚き",
    "高鳴り",
    "情熱",
    "確信",
    "期待",
    "楽しい",
    "喜び",
];
/// i18n `StratoShout.RTT.items[4]`（行動）。
static JA_RTT_SKILLS5: &[&str] = &[
    "泣く",
    "忘れる",
    "消す",
    "壊す",
    "叫ぶ",
    "歌う",
    "踊る",
    "走る",
    "出会う",
    "呼ぶ",
    "笑う",
];
/// i18n `StratoShout.RTT.items[5]`（逆境）。
static JA_RTT_SKILLS6: &[&str] = &[
    "死", "喪失", "暴力", "孤独", "後悔", "実力", "退屈", "本性", "富", "恋愛", "生",
];

/// Ruby `RTT` の特技リスト（分野は1D6の出目順）。
static JA_RTT_CATEGORIES: &[SaiFicCategory] = &[
    SaiFicCategory::new("主義", JA_RTT_SKILLS1),
    SaiFicCategory::new("身体", JA_RTT_SKILLS2),
    SaiFicCategory::new("モチーフ", JA_RTT_SKILLS3),
    SaiFicCategory::new("情緒", JA_RTT_SKILLS4),
    SaiFicCategory::new("行動", JA_RTT_SKILLS5),
    SaiFicCategory::new("逆境", JA_RTT_SKILLS6),
];

/// Ruby `RTT`（`SaiFicSkillTable.from_i18n("StratoShout.RTT", :ja_jp, rtt: 'AT', rttn: [...])`）。
///
/// `from_i18n` は `I18n.t("RTT", locale:)`（グローバル）に
/// `I18n.t("StratoShout.RTT", locale:)` を `merge` するが、`i18n/ja_jp.yml` に
/// グローバルの `RTT` は無いので `rct_format` と `s_format` は既定のまま。
static JA_RTT: SaiFicSkillTable = SaiFicSkillTable::new(JA_RTT_CATEGORIES)
    .with_commands(
        Some("AT"),
        None,
        &["AT1", "AT2", "AT3", "AT4", "AT5", "AT6"],
    )
    .with_formats(SaiFicFormats {
        rtt: "特技リスト ＞ [%<category_dice>d,%<row_dice>d] ＞ %<text>s",
        rct: DEFAULT_RCT_FORMAT,
        rttn: "特技リスト(%<category_name>s分野) ＞ [%<row_dice>d] ＞ %<text>s",
        skill: DEFAULT_SKILL_FORMAT,
    });

/// `ja_jp` ロケールの表と定型文一式。
pub(crate) static JA_SYSTEM: SystemTables = SystemTables {
    tables: JA_TABLES,
    rtt: &JA_RTT,
    critical: "スペシャル！ (【コンディション】+2)",
    fumble:
        "ファンブル！ (ドラマフェイズ: 【ディスコード】+2 / ライブフェイズ: 【コンディション】-2)",
    success: "成功",
    failure: "失敗",
};

/// Ruby `BCDice::GameSystem::StratoShout`（ID: `StratoShout`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StratoShout;

impl GameSystem for StratoShout {
    fn id(&self) -> &'static str {
        "StratoShout"
    }

    fn name(&self) -> &'static str {
        "ストラトシャウト"
    }

    fn sort_key(&self) -> &'static str {
        "すとらとしやうと"
    }

    fn help_message(&self) -> &'static str {
        r"
VOT, GUT, BAT, KEYT, DRT: (ボーカル、ギター、ベース、キーボード、ドラム)トラブル表
EMO: 感情表
ATn, RTTn: 特技表(n＝分野。空:ランダム 1:主義 2:身体 3:モチーフ 4:情緒 5:行動 6:逆境)
RCT: 分野ランダム表
SCENE, MACHI, GAKKO, BAND: (汎用、街角、学校、バンド)シーン表 接近シーンで使用
TENKAI: シーン展開表 奔走シーン 練習シーンで使用

D66入れ替えあり
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "VOT",
            "GUT",
            "BAT",
            "KEYT",
            "DRT",
            "EMO",
            "SCENE",
            "MACHI",
            "GAKKO",
            "BAND",
            "TENKAI",
            "RTT[1-6]?",
            "RCT",
            "AT",
            "AT1",
            "AT2",
            "AT3",
            "AT4",
            "AT5",
            "AT6",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `StratoShout#initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `StratoShout#initialize` の `@d66_sort_type = D66SortType::ASC`。
    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }

    /// Ruby `StratoShout#result_2d6`。
    fn result_2d6(
        &self,
        _total: crate::Int,
        dice_total: i64,
        _value_list: &[i64],
        cmp_op: CmpOp,
        _target: Target,
    ) -> Option<CheckOutcome> {
        check_result_2d6(&JA_SYSTEM, crate::Int::from(dice_total), cmp_op)
    }

    /// Ruby `StratoShout#eval_game_system_specific_command`。
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
    use std::path::{Path, PathBuf};

    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;

    fn toml_path() -> Option<PathBuf> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .join("test/data/StratoShout.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/StratoShout.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/StratoShout.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("StratoShout.toml must parse");
        assert_eq!(
            data.tests.len(),
            23,
            "case count in test/data/StratoShout.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "StratoShout",
                "unexpected game system in StratoShout.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("StratoShout"), &tc.input, &mut src) {
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
                    "FAIL StratoShout:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} StratoShout cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
