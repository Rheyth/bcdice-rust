//! P4で手書き移植した `lib/bcdice/game_system/Bakenokawa.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Bakenokawa#check_action`（行為判定 `xBKy@z`）
//! - `Bakenokawa#eval_game_system_specific_command` → `check_action || roll_tables`
//! - `TABLES`（今の関係表 / カイブツ時代からの因縁表 / 調査演出表 / コラボテーマ表 / ファンブル表）
//!
//! 表データは Ruby の定数から機械的に書き出したもので、値は1文字も変えていない
//! （`RTK` 10項目目の末尾カンマ、`RTB` が 1D10 で9項目しか無いことも原典どおり）。

use std::sync::OnceLock;

use crate::command_parser::Parser;
use crate::dice_table::{D66Table, RollableTable, Table, TableItem};
use crate::enums::{D66SortType, RoundType};
use crate::eval::EvalError;
use crate::game_system::{dice_text, table_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

// ---------------------------------------------------------------------------
// 表データ
// ---------------------------------------------------------------------------

static NRT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("バケノカワ同士が親戚だった")),
    (12, TableItem::Text("仲のいい職場の同僚")),
    (13, TableItem::Text("仕事のことで助けてもらったことがある")),
    (14, TableItem::Text("人間社会のことを教え合っている")),
    (15, TableItem::Text("施設同士がよくコラボする")),
    (16, TableItem::Text("街のお店に二人で出かける仲")),
    (
        22,
        TableItem::Text("どうにもウマが合わずに喧嘩ばかりしている"),
    ),
    (23, TableItem::Text("そのバケノカワを羨ましいと思っていた")),
    (
        24,
        TableItem::Text("バケノカワに関する苦労を聞いたことがある"),
    ),
    (25, TableItem::Text("バケノカワとしての生活圏が近い")),
    (26, TableItem::Text("バケノカワ同士が知り合いだった")),
    (33, TableItem::Text("あんな人間になりたいと憧れを覚えた")),
    (
        34,
        TableItem::Text("友人として色々な悩みを共に解決してきた"),
    ),
    (
        35,
        TableItem::Text("放っておけないところがあると思っている"),
    ),
    (36, TableItem::Text("いつも迷惑をかけて悪いと思っている")),
    (
        44,
        TableItem::Text("毎朝挨拶をするようなご近所さん／寮の部屋が隣"),
    ),
    (45, TableItem::Text("何か悩んでいそうな顔が気になった")),
    (
        46,
        TableItem::Text("何かと仕事関係で助けてもらい、世話になっている"),
    ),
    (55, TableItem::Text("立ち振る舞いが人間らしくて羨ましい")),
    (
        56,
        TableItem::Text("ワンダーランドでの振る舞いや仕事ぶりに一目置いている"),
    ),
    (
        66,
        TableItem::Text("そのバケノカワを見ると懐かしさを覚える"),
    ),
];
static NRT: D66Table = D66Table::new("今の関係表", D66SortType::Asc, NRT_ITEMS);

static KKT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("一緒の牢獄に捕まっていたことがある")),
    (12, TableItem::Text("地下の国で一緒に遊んでいたことがある")),
    (
        13,
        TableItem::Text("地上に出て人間を楽しませる夢を語り合った"),
    ),
    (14, TableItem::Text("一緒に人間社会を学ぶ訓練を受けた")),
    (15, TableItem::Text("生まれた時からずっと一緒だった")),
    (16, TableItem::Text("時々喧嘩をし合うような仲だった")),
    (22, TableItem::Text("幼少期に育ててもらった恩がある")),
    (23, TableItem::Text("昔助けてもらった時の借りがある")),
    (
        24,
        TableItem::Text("思い出せないぐらい昔に、助けてもらった……気がする"),
    ),
    (
        25,
        TableItem::Text("一緒に地上で人間を楽しませようと約束した"),
    ),
    (26, TableItem::Text("人間に対する憧れを語った")),
    (
        33,
        TableItem::Text("カイブツ時代、一方的に憧れを抱いていた"),
    ),
    (34, TableItem::Text("人間のまね事を一緒にしていた")),
    (
        35,
        TableItem::Text("少しの間、一緒に暮らしていたことがある"),
    ),
    (36, TableItem::Text("一緒にお茶会をした仲")),
    (44, TableItem::Text("同じ親のもとで育った")),
    (
        45,
        TableItem::Text("カイブツ時代に貸しがあって、まだ返してもらってない"),
    ),
    (46, TableItem::Text("何かと競い合う、ライバル同士だった")),
    (55, TableItem::Text("戦いを挑み、ボコボコにされた")),
    (
        56,
        TableItem::Text("一緒に退屈を紛らわすために色々悪だくみしていた"),
    ),
    (
        66,
        TableItem::Text("バケノカワを貰った後にやりたいことを語ってもらった覚えがある"),
    ),
];
static KKT: D66Table = D66Table::new("カイブツ時代からの因縁表", D66SortType::Asc, KKT_ITEMS);

static RTK_ITEMS: &[&str] = &[
    "カイブツとして街に潜み、チェインNPCの周りを調べて回った",
    "カイブツとしての力を使い、チェインNPCを尾行して好みを調べた",
    "人間には潜入できないルートを使って、チェインNPCの周りを調べた",
    "人間として調べてみようとしたが、カイブツとしての特徴が出て困った",
    "カイブツとしての特徴を調査に活かし、調べ上げた",
    "魔法の鏡に問いかけて、答えを貰った",
    "チェインNPCの痕跡から、魔法の力を使って過去を辿った",
    "仲間のカイブツたちと一緒に、街に繰り出してドタバタ劇を繰り広げながら情報を集めた",
    "小さなカイブツたちに、気付かれないようにチェインNPCを調べてもらった",
    "魔女のカイブツに頼んで、調査に便利な薬を作ってもらった,",
];
static RTK: Table = Table::from_dice("調査演出表　カイブツ", 1, 10, RTK_ITEMS);

static RTB_ITEMS: &[&str] = &[
    "人間の友人を頼ってチェインNPCの噂話を聞く",
    "人間の街に行ってチェインNPCのことを聞いて回る",
    "チェインPCに話を聞いて、その子の特徴から好みを推察する",
    "チェインPCと一緒に、チェインNPCの周囲を聞き込みして回った",
    "人間として、情報通の人間が集まる場所に向かい、そこでいろいろな話を聞いて回った",
    "ネットを利用して、チェインNPCの痕跡がないか調べてみた",
    "チェインNPCがよく行く場所を掴み、そこで情報収集をした",
    "実は、チェインNPCは自分のバケノカワとして会ったことがあり、そのときのことを思い出した",
    "実は、チェインNPCはお客様として自分の担当施設に来たことがあり、そのときの印象を思い出した",
];
static RTB: Table = Table::from_dice("調査演出表　バケノカワ", 1, 10, RTB_ITEMS);

static CTK_ITEMS: &[(i64, TableItem)] = &[
    (
        11,
        TableItem::Text("自分とコラボ相手の「カイブツとしての姿」を使ったド派手なコラボ"),
    ),
    (12, TableItem::Text("自分の特徴を使ったコラボ")),
    (13, TableItem::Text("コラボ相手の特徴を使ったコラボ")),
    (14, TableItem::Text("自分とコラボ相手の特徴を活かすコラボ")),
    (
        15,
        TableItem::Text("自分の施設とコラボ相手の特徴を合わせたコラボ"),
    ),
    (
        16,
        TableItem::Text("コラボ相手の施設と自分の特徴を合わせたコラボ"),
    ),
    (
        22,
        TableItem::Text("自分とコラボ相手の「カイブツとしての力」を使ったコラボ"),
    ),
    (
        23,
        TableItem::Text("自分とコラボ相手のパビリオンを活かすコラボ"),
    ),
    (
        24,
        TableItem::Text("自分のカイブツとしての力を使ったコラボ"),
    ),
    (
        25,
        TableItem::Text("相手のカイブツとしての力を使ったコラボ"),
    ),
    (
        26,
        TableItem::Text("自分がカイブツとして優れているところを、コラボ相手に聞いてヒントにする"),
    ),
    (
        33,
        TableItem::Text("カイブツとしての姿をキャラクターグッズとして売り出すコラボ"),
    ),
    (
        34,
        TableItem::Text("魔法の力を演出に使った、キラキラのコラボ"),
    ),
    (
        35,
        TableItem::Text("魔法の道具を使った、お客様を驚かせるようなコラボ"),
    ),
    (
        36,
        TableItem::Text("童話の世界をくっつけて、ちぐはぐな感じを楽しんでもらう"),
    ),
    (
        44,
        TableItem::Text("パビリオンの仲間たちを集めたステージを開催"),
    ),
    (
        45,
        TableItem::Text("カイブツとしての姿をあえて晒し、それを「演出」に組み込む"),
    ),
    (
        46,
        TableItem::Text("自分たちの力を「演出」として使ったコラボ"),
    ),
    (
        55,
        TableItem::Text("魔法の力がかかったお土産を持たせるコラボ"),
    ),
    (56, TableItem::Text("不思議な演出がいっぱいのコラボ")),
    (
        66,
        TableItem::Text("自分たち以外のカイブツも呼んだ、賑やかなコラボ"),
    ),
];
static CTK: D66Table = D66Table::new("コラボテーマ表　カイブツ", D66SortType::Asc, CTK_ITEMS);

static CTB_ITEMS: &[(i64, TableItem)] = &[
    (
        11,
        TableItem::Text("バケノカワとして生活するうちに覚えた、「楽しかったこと」をやってみる"),
    ),
    (12, TableItem::Text("コラボ相手の施設におじゃまするコラボ")),
    (13, TableItem::Text("自分の施設にコラボ相手を呼ぶ")),
    (
        14,
        TableItem::Text("自分とコラボ相手の施設にお土産を用意する"),
    ),
    (15, TableItem::Text("コラボ相手の施設の要素を使う")),
    (
        16,
        TableItem::Text("自分の施設の要素をコラボ相手の施設に贈る"),
    ),
    (
        22,
        TableItem::Text("バケノカワ生活の中で気付いた、「人間は面白い」と思えたことをやる"),
    ),
    (
        23,
        TableItem::Text("自分とコラボ相手の技能で園全体を盛り上げる"),
    ),
    (24, TableItem::Text("自分の技能をコラボ相手の施設に活かす")),
    (25, TableItem::Text("パートナーの技能を自分の施設に活かす")),
    (26, TableItem::Text("自分のカバーパーソナルを活かすコラボ")),
    (
        33,
        TableItem::Text("自分だけではできないことをコラボ相手と相談する"),
    ),
    (
        34,
        TableItem::Text("パートナーのカバーパーソナルを活かすコラボ"),
    ),
    (
        35,
        TableItem::Text("二つの施設を合わせたような施設を限定オープン"),
    ),
    (
        36,
        TableItem::Text("お客様の笑顔を思い出し、そうなるように二人で努力する"),
    ),
    (44, TableItem::Text("二つの施設を繋げる園内バスを用意する")),
    (
        45,
        TableItem::Text("お客様に楽しんでもらえる演出を二人で考える"),
    ),
    (
        46,
        TableItem::Text("お客様が好みそうなお土産を二人で考える"),
    ),
    (
        55,
        TableItem::Text("人間の友達に、「何が楽しいのか」を改めて聞く"),
    ),
    (
        56,
        TableItem::Text("バケノカワの伝手を使って、人間の友達に話を聞く"),
    ),
    (
        66,
        TableItem::Text("ワンダーランドのみんなに手伝ってもらう豪華なコラボ"),
    ),
];
static CTB: D66Table = D66Table::new("コラボテーマ表　バケノカワ", D66SortType::Asc, CTB_ITEMS);

static FT_ITEMS: &[&str] = &[
    "「あれ？”自分”は一体何者だったかな？」カバーパーソナルを1つ選んで失う",
    "ふとした瞬間に、楽しい感情に対して懐疑的になってしまう。ワンダートークンを1個選んで失う",
    "せっかく用意した道具を壊してしまう。セットしていたサプライズカードを1枚選んで破棄する",
    "バケノカワの記憶が思い出される。カバーパーソナルとそれに付随するパーソナル技能を1つずつ獲得する",
    "カイブツとしての自分が前面に出てしまう。次に行う判定の間、すべての技能を修得していないものとして扱う",
    "失敗してしまったが、それがお客様にウケた。ワンダートークンを1個獲得する",
];
static FT: Table = Table::from_dice("ファンブル表", 1, 6, FT_ITEMS);

/// Ruby `TABLES`。
static TABLES: &[(&str, &dyn RollableTable)] = &[
    ("NRT", &NRT),
    ("KKT", &KKT),
    ("RTK", &RTK),
    ("RTB", &RTB),
    ("CTK", &CTK),
    ("CTB", &CTB),
    ("FT", &FT),
];

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `Bakenokawa#check_action`（行為判定 `xBKy@z`）。
fn check_action(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    let parser = PARSER.get_or_init(|| {
        Parser::new(&["BK"], RoundType::Floor)
            .enable_critical()
            .enable_prefix_number()
            .has_suffix_number()
    });
    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };

    let target = 4;
    let dice_cnt = parsed
        .prefix_number
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(2);
    // has_suffix_number なので必ず入る
    let Some(dice_faces) = parsed.suffix_number else {
        return Ok(None);
    };
    let special_target = parsed
        .critical
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(12);

    let mut dice_arr = rng.roll_barabara(dice_cnt, crate::randomizer::sat_i64(&dice_faces))?;
    dice_arr.sort_unstable();
    let dice_str = dice_text::join_dice(&dice_arr);
    let dice_sum: i64 = dice_arr.iter().fold(0i64, |a, b| a.wrapping_add(*b));
    let has_special = dice_sum >= special_target;
    let has_fumble = dice_sum <= 2;
    let result = dice_arr.iter().any(|&x| x >= target);

    let text = format!(
        "({dice_cnt}B{dice_faces}>={target}) ＞ [{dice_str}] ＞ {dice_sum} ＞ {}{}{}",
        if result { "成功" } else { "失敗" },
        if has_special { "(スペシャル)" } else { "" },
        if has_fumble { "(ファンブル)" } else { "" },
    );

    Ok(Some(EvalResult {
        text,
        critical: has_special,
        fumble: has_fumble,
        success: result,
        failure: !result,
        ..EvalResult::default()
    }))
}

/// Ruby `BCDice::GameSystem::Bakenokawa`（ID: `Bakenokawa`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bakenokawa;

impl GameSystem for Bakenokawa {
    fn id(&self) -> &'static str {
        "Bakenokawa"
    }

    fn name(&self) -> &'static str {
        "バケノカワ"
    }

    fn sort_key(&self) -> &'static str {
        "はけのかわ"
    }

    fn help_message(&self) -> &'static str {
        r"・行為判定
  xBKy@z
    x：振るダイスの数(省略可、省略した場合は2)
    y：振るダイスの面数
    z：スペシャル値(@ごと省略可、省略した場合は12)
  （例）BK10
  　　　4BK6
  　　　2BK6@10

・各種表
  今の関係表 NRT
  カイブツ時代からの因縁表 KKT
  調査演出表
    カイブツ RTK
    バケノカワ RTB
  コラボテーマ表
    カイブツ CTK
    バケノカワ CTB
  ファンブル表 FT
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d*BK", "NRT", "KKT", "RTK", "RTB", "CTK", "CTB", "FT"]
    }

    crate::impl_prefixes_pattern!();

    fn sort_barabara_dice(&self) -> bool {
        true
    }

    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }

    /// Ruby `Bakenokawa#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(result) = check_action(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        Ok(table_helpers::roll_table(command, TABLES, rng)?.map(SpecificCommandOutput::text))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "Bakenokawa",
            "Bakenokawa.toml",
            69,
        );
    }
}
