//! P4で手書き移植した `lib/bcdice/game_system/CyberpunkRed.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `CyberpunkRed#cp_roll_result`（判定 `CPx+y>z`。10で決定的成功、1で決定的失敗）
//! - `CyberpunkRed#eval_game_system_specific_command`（判定 → `roll_tables`）
//! - `lib/bcdice/game_system/cyberpunk_red/tables.rb`
//!   （`ScreamSheetRandomizerTable` / `ShopPeopleTable` / `VMCR` の `ChainTable`）
//!
//! 表データは `i18n/CyberpunkRed/ja_jp.yml` から機械的に書き出したもので、値は1文字も変えていない。
//! ロケール差のあるデータは [`SystemTables`] に束ね、
//! `CyberpunkRed_Korean`（`ko_kr`）が同じ関数群を使い回す。

use std::sync::OnceLock;

use crate::command_parser::Parser;
use crate::dice_table::{ChainTable, RangeInc, RangeTable, RollableTable, Table, TableItem};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int as I;

static JA_FFD_ITEMS: &[&str] = &[
    "片腕喪失 ＞ 1本の腕が完全に失われる。おまえはその腕の手に持っていたアイテムを即座に落とす。死亡セーヴ修正値の基本値が1増える。",
    "片手喪失 ＞ 1本の手が完全に失われる。おまえはその手に持っていたアイテムを即座に落とす。死亡セーヴ修正値の基本値が1増える。",
    "肺損傷 ＞ 【移動】に-2(最低1)。死亡セーヴ修正値の基本値が1増える。",
    "肋骨損傷 ＞ おまえは1ターンに徒歩で4m(2マス)より多く移動するたび、そのターン終了時に、この致命的損傷によるボーナス・ダメージ5点を直接HPに受ける。",
    "片腕損傷 ＞ 損傷した腕は使い物にならない。おまえはその腕の手に持っていたアイテムを即座に落とす。",
    "外傷異物 ＞ 1ターンに徒歩で4m(2マス)より多く移動するたび、そのターン終了時に、この致命的損傷によるボーナス・ダメージ5点を直接HPに受ける。",
    "片脚損傷 ＞ 【移動】に-4(最低1)。",
    "筋肉断裂 ＞ 近接攻撃に-2。",
    "脊椎損傷 ＞ おまえは次のターンにアクションを行えないが、移動アクションは行える。死亡セーヴ修正値の基本値が1増える。",
    "指損傷 ＞ その手に関係したあらゆるアクションに-4。",
    "片脚喪失 ＞ 1本の脚が完全に失われる。【移動】に-6(最低1)。死亡セーヴ修正値の基本値が1増える。",
];
static JA_FFD: Table = Table::from_dice("身体への致命的損傷", 2, 6, JA_FFD_ITEMS);

static JA_HFD_ITEMS: &[&str] = &[
    "片目喪失 ＞ 1個の目が完全に失われる。遠隔攻撃と視覚に関する〈知覚〉判定に-2。死亡セーヴ修正値の基本値が1増える。",
    "脳損傷 ＞ あらゆるアクションに-2。死亡セーヴ修正値の基本値が1増える。",
    "片目損傷 ＞ 遠隔攻撃と視覚に関する〈知覚〉判定に-2。",
    "脳震盪 ＞ あらゆるアクションに-2。",
    "顎部損傷 ＞ 声を発することに関連したあらゆるアクションに-2。",
    "外傷異物 ＞ 1ターンに徒歩で4m(2マス)より多く移動するたび、そのターン終了時に、この致命的損傷によるボーナス・ダメージ5点を直接HPに受ける。",
    "頚椎損傷 ＞ 死亡セーヴ修正値の基本値が1増える。",
    "頭蓋損傷 ＞ おまえの頭部に対する狙い撃ちは、SPを貫通した後のダメージが2倍ではなく3倍になる。死亡セーヴ修正値の基本値が1増える。",
    "片耳損傷 ＞ 1ターンに徒歩で4m(2マス)より多く移動すると、その次のターンは移動アクションを行うことができなくなる。聴覚に関する〈知覚〉判定に-2。",
    "気管損傷 ＞ 声を発することができない。死亡セーヴ修正値の基本値が1増える。",
    "片耳喪失 ＞ 1個の耳が完全に失われる。1ターンに徒歩で4m(2マス)より多く移動すると、その次のターンは移動アクションを行うことができなくなる。聴覚に関する〈知覚〉判定に-4。死亡セーヴ修正値の基本値が1増える。",
];
static JA_HFD: Table = Table::from_dice("頭部への致命的損傷", 2, 6, JA_HFD_ITEMS);

static JA_NCDT_ITEMS: &[(RangeInc, &str)] = &[
    (RangeInc::new(1, 5), "地元の警官 ＞ PCグループの半分の人数の巡回警官。P.417参照。"),
    (RangeInc::new(6, 11), "企業の警備員 ＞ この地区をパトロールしている、企業の下級警備員。PCグループと同じ人数。P.417参照。"),
    (RangeInc::new(12, 13), "テク ＞ PCグループの半分の人数のテク(技術者)。P.417参照。"),
    (RangeInc::new(14, 17), "私立探偵 ＞ 1人の私立探偵。P.417参照。"),
    (RangeInc::new(18, 20), "企業の社員 ＞ 現地企業の社員がタクシーを探している。人数はPCと同数。P.417参照。"),
    (RangeInc::new(21, 27), "地元民 ＞ この辺りに住む2人の若者。P.418参照。"),
    (RangeInc::new(28, 32), "リクレイマー ＞ PCは十分に装備の整ったリクレイマーの一団と出くわす。人数は団員が［PCグループの人数-2］人と、リーダーが1人だ。P.418参照。"),
    (RangeInc::new(33, 37), "メディア ＞ カメラとインタビュアーの2人組のメディア。特ダネを求めて1軒の建物に張り込みしている。P.418参照。"),
    (RangeInc::new(38, 41), "私立探偵 ＞ 1人の私立探偵。P.418参照。"),
    (RangeInc::new(42, 46), "トラウマ・チーム ＞ AV-4が銃撃戦の真っ只中に強行着陸し、降りて来たメディックたちが、負傷した6人ほどのギャンガーたちに応急処置を施しはじめる。P.418参照。"),
    (RangeInc::new(47, 57), "スカヴァーズ ＞ PCと同人数の貧相で汚らしい浮浪者たちが、焼け落ちた街区近辺にある廃墟またはゴミ箱を漁っている。P.419参照。"),
    (RangeInc::new(58, 63), "ノーマッド ＞ PCと同人数のノーマッドの一団。P.419参照。"),
    (RangeInc::new(64, 70), "ブースターギャング ＞ ピラーニャズというブースターギャングに所属する、低レベルなストリート・パンクたち。人数はPCと同数。P.419参照。"),
    (RangeInc::new(71, 76), "ストリート・パンク ＞ 合成麻薬飲料中毒者どもが、飲み代を稼ごうとカモを探している(人数はPCと同数)。P.419参照。"),
    (RangeInc::new(77, 82), "狂信者 ＞ 終末カルト”レコナーズ”が大挙して押し寄せてくる。P.419参照。"),
    (RangeInc::new(83, 88), "ノーマッドのトラック ＞ 1台の壊れたトラックの周りで、スチール・ヴァロケスのノーマッドたち(人数はPCの半分；最低2人)が、何かしている。P.419参照。"),
    (RangeInc::new(89, 94), "ブースターギャング ＞ アイアン・サイツというギャングのメンバーたち。人数はPCと同じ。P.419参照。"),
    (RangeInc::new(95, 100), "メジャー級の犯罪者 ＞ PCたちは冷酷非情なヴィルシェンコ・シンジケート(ネオソビエトの犯罪組織)の大規模なシノギの現場に足を踏み入れてしまう。P.419参照。"),
];
static JA_NCDT: RangeTable =
    RangeTable::from_dice("ナイトシティにおける日中の遭遇", 1, 100, JA_NCDT_ITEMS);

static JA_NCMT_ITEMS: &[(RangeInc, &str)] = &[
    (RangeInc::new(1, 10), "市警察 ＞ PCグループの半分の人数の警察官。P.422参照。"),
    (RangeInc::new(11, 22), "企業の警備員 ＞ この地区をパトロールしている、企業の警備員。PCグループと同じ人数。P.422参照。"),
    (RangeInc::new(23, 24), "私立探偵 ＞ 超大型拳銃とマチェーテで武装し、軽装アーマージャックを着用した私立探偵1人。P.422参照。"),
    (RangeInc::single(25), "メディア ＞ カメラとインタビュアーの2人組のメディア。特ダネを求めて1軒の建物に張り込みしている。P.422参照。"),
    (RangeInc::new(26, 29), "クローマー ＞ 地元のクロマティック・ロック・バンドの筋金入りのファンの一団。P.423参照。"),
    (RangeInc::new(30, 39), "エッジランナー・チーム ＞ 少人数のエッジランナーのチーム。ネットランナー1人、ソロ1人、ノーマッド1人。P.423参照。"),
    (RangeInc::new(40, 42), "トラウマ・チーム ＞ AV-4が銃撃戦の真っ只中に強行着陸し、降りて来たメディックたちが、負傷した6人ほどのギャンガーたちに応急処置を施しはじめる。P.423参照。"),
    (RangeInc::new(43, 45), "レンジャー ＞ 1人のローマン(保安官)と、このローマンに任命された保安官助手が、市内に隠れている浮浪者たち(地元のギャング団員)を探している。P.423参照。"),
    (RangeInc::new(46, 58), "ノーマッド ＞ ワイルドマン・パックに属するノーマッドの一団。人数はPCより2人多い。P.423参照。"),
    (RangeInc::new(59, 63), "狂信者 ＞ ジャーン！　まさかの時のインクィジェターズ異端審問！　カルトのインクィジェターズが総出で押し寄せてくる。P.423参照。"),
    (RangeInc::new(64, 73), "ストリート・パンク ＞ ブラック・レース中毒者どもが、クスリを買うカネを欲しがっている。人数はPCより2人多い。P.423参照。"),
    (RangeInc::single(74), "メジャー級の犯罪者 ＞ PCは悪名高いスカガッタリア・ファミリーの大きな仕事の真っただ中に迷い込んだ。P.423参照。"),
    (RangeInc::new(75, 79), "ギャングの抗争 ＞ すげえ。PCはこの地域で最大級のギャング団同士の、縄張りを賭けた全面衝突の場に居合わせちまった。P.423参照。"),
    (RangeInc::new(80, 87), "放火魔 ＞ この地域の誰かに恨みがある、急進的なアナーキストの集団。フレイムスロワーと斧と大型拳銃で武装しサイバードアップしたギャンガーが1名が、PCより3人少ない(最低2人)ブースターを率いている。P.424参照。"),
    (RangeInc::new(88, 92), "ギャングの抗争 ＞ すげえ。PCはこの地域で最大級のギャング団同士の、縄張りを賭けた全面衝突の場に居合わせちまった。P.424参照。"),
    (RangeInc::new(93, 99), "メジャー級の犯罪者 ＞ PCは悪名高いスカガッタリア・ファミリーの大きな仕事の真っただ中に迷い込んだ。P.424参照。"),
    (RangeInc::single(100), "暴れまわるサイバーサイコ ＞ ギラギラと輝くメタル・ボディのサイバーサイコが1体。通行人がうかつに挑発し過ぎて最後のエッジを超えさせてしまったらしく、そいつに湧き上がる怒りを叩きつけている。P.424参照。"),
];
static JA_NCMT: RangeTable =
    RangeTable::from_dice("ナイトシティにおける深夜の遭遇", 1, 100, JA_NCMT_ITEMS);

static JA_NMCT_ITEMS: &[&str] = &[
    "食品とドラッグ",
    "個人用電子機器",
    "武器と防具",
    "サイバーウェア",
    "衣料品とファッションウェア",
    "サバイバル用品",
];
static JA_NMCT: Table = Table::from_dice("ナイトマーケット（夜の市）のタイプ", 1, 6, JA_NMCT_ITEMS);

static JA_NMCFO_ITEMS: &[(RangeInc, &str)] = &[
    (RangeInc::new(1, 5), "缶詰　100ed(安物)"),
    (RangeInc::new(6, 10), "パック入り食品　10ed(安物)"),
    (RangeInc::new(11, 15), "冷凍食品　10ed(安物)"),
    (RangeInc::new(16, 20), "穀物の袋　20ed(日用)"),
    (RangeInc::new(21, 25), "キブルのパック　10ed(安物)"),
    (RangeInc::new(26, 30), "プレパックの袋　20ed(日用)"),
    (RangeInc::new(31, 35), "ストリート・ドラッグ　20ed以下"),
    (RangeInc::new(36, 40), "低品質の酒　10ed(安物)"),
    (RangeInc::new(41, 45), "酒　20ed(日用)"),
    (RangeInc::new(46, 50), "高品質の酒　100ed(上等)"),
    (RangeInc::new(51, 55), "MRE　10ed(安物)"),
    (RangeInc::new(56, 60), "生きた鶏　50ed(程々)"),
    (RangeInc::new(61, 65), "生きた魚　50ed(程々)"),
    (RangeInc::new(66, 70), "生の果物　50ed(程々)"),
    (RangeInc::new(71, 75), "生野菜　50ed(程々)"),
    (RangeInc::new(76, 80), "根菜類　20ed(日用)"),
    (RangeInc::new(81, 85), "生きた豚　100ed(上等)"),
    (RangeInc::new(86, 90), "珍しい果物　100ed(上等)"),
    (RangeInc::new(91, 95), "珍しい野菜　100ed(上等)"),
    (RangeInc::new(96, 99), "ストリート・ドラッグ　50edちょうど"),
    (
        RangeInc::single(100),
        "缶詰　100ed(安物) or ストリート・ドラッグ　50edちょうど",
    ),
];
static JA_NMCFO: RangeTable = RangeTable::from_dice("食品とドラッグ", 1, 100, JA_NMCFO_ITEMS);

static JA_NMCME_ITEMS: &[(RangeInc, &str)] = &[
    (RangeInc::new(1, 5), "エージェント　100ed(上等)"),
    (
        RangeInc::new(6, 10),
        "プログラムおよびハードウェア　100ed以下",
    ),
    (RangeInc::new(11, 15), "オーディオ・レコーダー　100ed(上等)"),
    (RangeInc::new(16, 20), "盗聴器検出器　500ed(高級)"),
    (RangeInc::new(21, 25), "化学物質分析器　1,000ed(超高級)"),
    (RangeInc::new(26, 30), "コンピュータ　50ed(程々)"),
    (RangeInc::new(31, 35), "サイバーデッキ　500ed(高級)"),
    (RangeInc::new(36, 40), "使い捨て携帯電話　50ed(程々)"),
    (
        RangeInc::new(41, 45),
        "エレキギターその他の楽器　500ed(高級)",
    ),
    (
        RangeInc::new(46, 50),
        "プログラムおよびハードウェア　500edちょうど",
    ),
    (RangeInc::new(51, 55), "メドスキャナー　1,000ed(超高級)"),
    (RangeInc::new(56, 60), "追跡装置　500ed(高級)"),
    (RangeInc::new(61, 65), "無線通信機　100ed(上等)"),
    (RangeInc::new(66, 70), "テクスキャナー　1,000ed(超高級)"),
    (RangeInc::new(71, 75), "スマート・グラス　500ed(高級)"),
    (RangeInc::new(76, 80), "レーダー探知機　500ed(高級)"),
    (
        RangeInc::new(81, 85),
        "スクランブラー／ディスクランブラー　500ed(高級)",
    ),
    (
        RangeInc::new(86, 90),
        "通信スキャナー／音楽プレイヤー　50ed(程々)",
    ),
    (
        RangeInc::new(91, 95),
        "ブレインダンス・ビューア　1,000ed(超高級)",
    ),
    (
        RangeInc::new(96, 99),
        "ヴァーチャリティ・ゴーグル　100ed(上等)",
    ),
    (
        RangeInc::single(100),
        "エージェント　100ed(上等) or ヴァーチャリティ・ゴーグル　100ed(上等)",
    ),
];
static JA_NMCME: RangeTable = RangeTable::from_dice("個人用電子機器", 1, 100, JA_NMCME_ITEMS);

static JA_NMCWE_ITEMS: &[(RangeInc, &str)] = &[
    (RangeInc::new(1, 5), "中型拳銃　50ed(程々)"),
    (
        RangeInc::new(6, 10),
        "大型拳銃および超大型拳銃　100ed(上等)",
    ),
    (RangeInc::new(11, 15), "SMG　100ed(上等)"),
    (RangeInc::new(16, 20), "大型SMG　100ed(上等)"),
    (RangeInc::new(21, 25), "ショットガン　500ed(高級)"),
    (RangeInc::new(26, 30), "アサルト・ライフル　500ed(高級)"),
    (RangeInc::new(31, 35), "スナイパー・ライフル　500ed(高級)"),
    (RangeInc::new(36, 40), "弓およびクロスボウ　100ed(上等)"),
    (
        RangeInc::new(41, 45),
        "グレネード・ランチャーおよびロケット・ランチャー　500ed(高級)",
    ),
    (RangeInc::new(46, 50), "弾薬　500ed以下"),
    (RangeInc::new(51, 55), "GMが選んだ特殊武器1個"),
    (RangeInc::new(56, 60), "小型近接武器　50ed(程々)"),
    (RangeInc::new(61, 65), "中型近接武器　50ed(程々)"),
    (RangeInc::new(66, 70), "大型近接武器　100ed(上等)"),
    (RangeInc::new(71, 75), "超大型近接武器　100ed(上等)"),
    (RangeInc::new(76, 80), "防具　100ed以下"),
    (RangeInc::new(81, 85), "防具　500edちょうど"),
    (RangeInc::new(86, 90), "防具　1,000edちょうど"),
    (RangeInc::new(91, 95), "武器用アタッチメント　100ed以下"),
    (RangeInc::new(96, 99), "武器用アタッチメント　500ed以上"),
    (
        RangeInc::single(100),
        "中型拳銃　50ed(程々) or 武器用アタッチメント　500ed以上",
    ),
];
static JA_NMCWE: RangeTable = RangeTable::from_dice("武器と防具", 1, 100, JA_NMCWE_ITEMS);

static JA_NMCCY_ITEMS: &[(RangeInc, &str)] = &[
    (RangeInc::new(1, 5), "サイバーアイ　100ed(上等)"),
    (
        RangeInc::new(6, 10),
        "サイバー聴覚基本システム　500ed(高級)",
    ),
    (RangeInc::new(11, 15), "ニューラル・リンク　500ed(高級)"),
    (RangeInc::new(16, 20), "サイバーアーム　500ed(高級)"),
    (RangeInc::new(21, 25), "サイバーレッグ　100ed(上等)"),
    (
        RangeInc::new(26, 30),
        "体表用サイバーウェア　1,000edちょうど",
    ),
    (RangeInc::new(31, 35), "体表用サイバーウェア　500ed以下"),
    (
        RangeInc::new(36, 40),
        "体内用サイバーウェア　1,000edちょうど",
    ),
    (RangeInc::new(41, 45), "体内用サイバーウェア　500ed以下"),
    (
        RangeInc::new(46, 50),
        "サイバーアイのオプション　1,000edちょうど",
    ),
    (RangeInc::new(51, 55), "サイバーアイのオプション　500ed以下"),
    (
        RangeInc::new(56, 60),
        "サイバー聴覚のオプション　1,000edちょうど",
    ),
    (RangeInc::new(61, 65), "サイバー聴覚のオプション　500ed以下"),
    (
        RangeInc::new(66, 70),
        "ニューラルウェアのオプション　1,000edちょうど",
    ),
    (
        RangeInc::new(71, 75),
        "ニューラルウェアのオプション　500ed以下",
    ),
    (
        RangeInc::new(76, 80),
        "サイバー四肢のオプション　1,000edちょうど",
    ),
    (RangeInc::new(81, 85), "サイバー四肢のオプション　500ed以下"),
    (RangeInc::new(86, 90), "GMが選んだファッションウェア"),
    (RangeInc::new(91, 95), "GMが選んだボーグウェア"),
    (RangeInc::new(96, 99), "GMが選んだ任意のサイバーウェア"),
    (
        RangeInc::single(100),
        "サイバーアイ　100ed(上等) or GMが選んだ任意のサイバーウェア",
    ),
];
static JA_NMCCY: RangeTable = RangeTable::from_dice("サイバーウェア", 1, 100, JA_NMCCY_ITEMS);

static JA_NMCFA_ITEMS: &[(RangeInc, &str)] = &[
    (RangeInc::new(1, 5), "バッグ・レディ・シック"),
    (RangeInc::new(6, 10), "ギャング・カラー"),
    (RangeInc::new(11, 15), "ジェネリック・シック"),
    (RangeInc::new(16, 20), "ボヘミアン"),
    (RangeInc::new(21, 25), "レジャーウェア"),
    (RangeInc::new(26, 30), "ノーマッド・レザー"),
    (RangeInc::new(31, 35), "アジア・ポップ"),
    (RangeInc::new(36, 40), "アーバン・フラッシュ"),
    (RangeInc::new(41, 45), "ビジネスウェア"),
    (RangeInc::new(46, 50), "ハイ・ファッション"),
    (RangeInc::new(51, 55), "バイオモニタ　100ed(上等)"),
    (RangeInc::new(56, 60), "ケムスキン　100ed(上等)"),
    (RangeInc::new(61, 65), "EMPスレッディング　10ed(安物)"),
    (RangeInc::new(66, 70), "ライト・タトゥー　100ed(上等)"),
    (RangeInc::new(71, 75), "シフトタクト　100ed(上等)"),
    (RangeInc::new(76, 80), "スキンウォッチ　100ed(上等)"),
    (RangeInc::new(81, 85), "テックヘア　100ed(上等)"),
    (RangeInc::new(86, 90), "ジェネリック・シック"),
    (RangeInc::new(91, 95), "レジャーウェア"),
    (RangeInc::new(96, 99), "ギャング・カラー"),
    (
        RangeInc::single(100),
        "バッグ・レディ・シック or ギャング・カラー",
    ),
];
static JA_NMCFA: RangeTable =
    RangeTable::from_dice("衣料品とファッションウェア", 1, 100, JA_NMCFA_ITEMS);

static JA_NMCSU_ITEMS: &[(RangeInc, &str)] = &[
    (RangeInc::new(1, 5), "対スモッグ呼吸マスク　20ed(日用)"),
    (
        RangeInc::new(6, 10),
        "自動制音イヤー・プロテクタ　1,000ed(超高級)",
    ),
    (RangeInc::new(11, 15), "双眼鏡　50ed(程々)"),
    (RangeInc::new(16, 20), "旅行バッグ　20ed(日用)"),
    (RangeInc::new(21, 25), "懐中電灯　20ed(日用)"),
    (RangeInc::new(26, 30), "ダクト・テープ　20ed(日用)"),
    (RangeInc::new(31, 35), "携帯用ベッド＆寝袋　20ed(日用)"),
    (RangeInc::new(36, 40), "鍵開けセット　20ed(日用)"),
    (RangeInc::new(41, 45), "手錠　50ed(程々)"),
    (RangeInc::new(46, 50), "メドテク・バッグ　100ed(上等)"),
    (RangeInc::new(51, 55), "テント＆キャンプ用品　50ed(程々)"),
    (RangeInc::new(56, 60), "ロープ(60m)　20ed(日用)"),
    (RangeInc::new(61, 65), "テクツール　100ed(上等)"),
    (RangeInc::new(66, 70), "パーソナル・ケアパック　20ed(日用)"),
    (RangeInc::new(71, 75), "放射線防護服　1,000ed(超高級)"),
    (RangeInc::new(76, 80), "発煙筒　10ed(安物)"),
    (RangeInc::new(81, 85), "グラップル・ガン　100ed(上等)"),
    (RangeInc::new(86, 90), "テク・バッグ　500ed(高級)"),
    (RangeInc::new(91, 95), "シャベルおよび斧　50ed(程々)"),
    (RangeInc::new(96, 99), "エアハイポ　50ed(程々)"),
    (
        RangeInc::single(100),
        "対スモッグ呼吸マスク　20ed(日用) or エアハイポ　50ed(程々)",
    ),
];
static JA_NMCSU: RangeTable = RangeTable::from_dice("サバイバル用品", 1, 100, JA_NMCSU_ITEMS);

static JA_SCST_ITEMS: &[&str] = &["国際", "全国", "州", "地方", "経済", "ゴシップ"];
static JA_SCST: Table = Table::from_dice("スクリームシート分類", 1, 6, JA_SCST_ITEMS);

static JA_SCSA_ITEMS: &[&str] = &[
    "(企業を1つ選ぶ)",
    "上院議員／議員",
    "大統領／会長／社長",
    "企業／企業群",
    "市議会",
    "サイバーサイコ",
    "殺し屋／殺人鬼",
    "退治人／暴漢",
    "悲劇的な／不運な",
    "捜査員／研究員",
];
static JA_SCSA: Table = Table::from_dice("ヘッドラインA", 1, 10, JA_SCSA_ITEMS);

static JA_SCSB_ITEMS: &[&str] = &[
    "協力を",
    "シティを",
    "妥協を／譲歩を",
    "警告を",
    "計画を",
    "スキャンダルを",
    "女性を",
    "男性を",
    "事故を",
    "希望を",
];
static JA_SCSB: Table = Table::from_dice("ヘッドラインB", 1, 10, JA_SCSB_ITEMS);

static JA_SCSC_ITEMS: &[&str] = &[
    "提案／提供",
    "脅かす／危機",
    "妥協／譲歩",
    "殺害／破綻／破棄",
    "殺害される／中止／中断",
    "死亡／終了／途絶える",
    "賞賛／表彰／好評",
    "表明／発表",
    "判明／暴露／発覚",
    "継続",
];
static JA_SCSC: Table = Table::from_dice("ヘッドラインC", 1, 10, JA_SCSC_ITEMS);

static JA_SCSOF_ITEMS: &[&str] = &["を", "が", "に対し", "とともに", "より", "へ向けて"];
static JA_SCSOF: Table = Table::from_dice("ヘッドライン助詞", 1, 6, JA_SCSOF_ITEMS);

static JA_VMCT_ITEMS: &[(RangeInc, &str)] = &[
    (RangeInc::new(1, 3), "食品"),
    (RangeInc::new(4, 5), "ファッション"),
    (RangeInc::single(6), "変なもの"),
];
static JA_VMCT: RangeTable = RangeTable::from_dice("最寄りの自販機のタイプ", 1, 6, JA_VMCT_ITEMS);

static JA_VMCE_ITEMS: &[&str] = &[
    "ラーメン",
    "ピザ",
    "ハンバーガー",
    "スマッシュ",
    "スシ",
    "暖かい肉料理",
    "キブルの1kgパック",
    "缶コーヒー",
    "缶ジュース",
    "缶入りの清涼飲料水",
];
static JA_VMCE: Table = Table::from_dice("食品", 1, 10, JA_VMCE_ITEMS);

static JA_VMCF_ITEMS: &[&str] = &[
    "缶に入ったTシャツ",
    "性的玩具",
    "傘",
    "ネクタイ",
    "外科手術用マスク",
    "下着",
    "花束",
    "銃と弾",
    "ブレインダンス・チップ",
    "ビデオゲーム",
];
static JA_VMCF: Table = Table::from_dice("ファッション", 1, 10, JA_VMCF_ITEMS);

static JA_VMCS_ITEMS: &[&str] = &[
    "ペット用のカツラ",
    "バグ・スナック(昆虫料理)",
    "レタス1玉",
    "トビウオがまるごと入ったスープ",
    "人工培養シーフード",
    "匂いの缶詰",
    "生きたカブトムシ",
    "紙製のコスプレ衣装",
    "奇妙なカプセル・トイ",
    "使用済みのパンティ",
];
static JA_VMCS: Table = Table::from_dice("変なもの", 1, 10, JA_VMCS_ITEMS);

static JA_STOREA_ITEMS: &[&str] = &[
    "母性的な老婆。客を自分の子供のように扱い、世話を焼いたり小言を言う。",
    "怒りっぽい年寄り。皆が嫌いで、特におまえのことが大嫌いだ。",
    "退屈しきったヨーギャンガー。両親の手でレジに繋がれている。",
    "異様になれなれしく、おまえに付きまとい続ける鬱陶しいやつ。",
    "露骨な薬物常用者。おまえにひらひら手を振って「もうどうにでもしてくれ……」と言う。",
    "理解不能な中年。何かつけておまえに文句を言おうとする。",
];
static JA_STOREA: Table = Table::from_dice("店主またはレジ係", 1, 6, JA_STOREA_ITEMS);

static JA_STOREB_ITEMS: &[&str] = &[
    "ぶつぶつ独り言を言い続ける客。急に立ち止まっておまえをじろじろ見る。",
    "騒ぎ立てる迷惑な酔っぱらい。",
    "薬物に酔った客。彼にしか見えない何かを殴ろうと拳を振り回している。",
    "金欠のジョイガール／ボーイ（娼婦／男娼）。「お願いを聞いてあげるから、スマッシュをおごってくれない？」と持ちかけてくる。",
    "非番のローマン。すぐに食べられるものを探しており、おしゃべりをしたい雰囲気ではない。",
    "ストリートで襲われる不安に駆られた客。誰かに近寄られると身構える(銃を抜くかもしれない)。",
];
static JA_STOREB: Table = Table::from_dice("変わった客その1", 1, 6, JA_STOREB_ITEMS);

static JA_STOREC_ITEMS: &[&str] = &[
    "この店に強盗に入る下見中のヨーギャンガー。強盗になる覚悟はまだ固まっていないようだ。",
    "1d6ターン後にこの店を襲う予定のギャンガー。",
    "店主を痛めつけて”みかじめ料”を取り立てようとする三流のギャンガー。",
    "かわいらしい四歳の迷子。”ママ”を探して偶然この店に入り込んだ。",
    "口論しながら入店したカップル。声はどんどんやかましくなる。",
    "場違いに高そうな服を着た金持ちのカップル。上流社会のパーティーからの帰り道に、酒を買おうとしてたまたまこの店に立ち寄った。",
];
static JA_STOREC: Table = Table::from_dice("変わった客その2", 1, 6, JA_STOREC_ITEMS);
/// Ruby `"VMCR" => DiceTable::ChainTable.new(VendingMachineTable.name, "1D6", [...])`。
static JA_VMCR_ITEMS: &[TableItem] = &[
    TableItem::Table(&JA_VMCE),
    TableItem::Table(&JA_VMCE),
    TableItem::Table(&JA_VMCE),
    TableItem::Table(&JA_VMCF),
    TableItem::Table(&JA_VMCF),
    TableItem::Table(&JA_VMCS),
];
static JA_VMCR: ChainTable = ChainTable::from_dice("最寄りの自動販売機表", 1, 6, JA_VMCR_ITEMS);

/// Ruby `TABLES` の値。`Table` / `RangeTable` / `ChainTable` と
/// 合成表（`ScreamSheetRandomizerTable` / `ShopPeopleTable`）が混在する。
pub(crate) enum TableRef {
    /// Ruby `DiceTable::Table`
    Plain(&'static Table),
    /// Ruby `DiceTable::RangeTable`
    Range(&'static RangeTable),
    /// Ruby `DiceTable::ChainTable`
    Chain(&'static ChainTable),
    /// Ruby `ScreamSheetRandomizerTable`（`SCSR`）
    ScreamSheet,
    /// Ruby `ShopPeopleTable`（`STORE`）
    ShopPeople,
}

/// Ruby `ScreamSheetRandomizerTable` が参照する表。
pub(crate) struct ScreamSheetTables {
    pub(crate) type_table: &'static Table,
    pub(crate) a_table: &'static Table,
    pub(crate) of_table: &'static Table,
    pub(crate) b_table: &'static Table,
    pub(crate) c_table: &'static Table,
}

/// Ruby `ShopPeopleTable` が参照する表と定型文（`CyberpunkRed.ShopPeopleTableText`）。
pub(crate) struct ShopPeopleTables {
    pub(crate) staff_table: &'static Table,
    pub(crate) people_a_table: &'static Table,
    pub(crate) people_b_table: &'static Table,
    pub(crate) intro: &'static str,
    pub(crate) shop_staff: &'static str,
    pub(crate) people_a: &'static str,
    pub(crate) people_b: &'static str,
    pub(crate) outro: &'static str,
}

/// 1ロケール分の表と定型文。
pub(crate) struct SystemTables {
    pub(crate) tables: &'static [(&'static str, TableRef)],
    /// `CyberpunkRed.critical`
    pub(crate) critical: &'static str,
    /// `CyberpunkRed.fumble`
    pub(crate) fumble: &'static str,
    /// `success`
    pub(crate) success: &'static str,
    /// `failure`
    pub(crate) failure: &'static str,
    /// `CyberpunkRed.news`
    pub(crate) news: &'static str,
    pub(crate) scream_sheet: ScreamSheetTables,
    pub(crate) shop_people: ShopPeopleTables,
}

/// Ruby `TABLES`（`translate_tables(:ja_jp)`）。
pub(crate) static JA_TABLES: &[(&str, TableRef)] = &[
    ("FFD", TableRef::Plain(&JA_FFD)),
    ("HFD", TableRef::Plain(&JA_HFD)),
    ("NCDT", TableRef::Range(&JA_NCDT)),
    ("NCMT", TableRef::Range(&JA_NCMT)),
    ("NMCT", TableRef::Plain(&JA_NMCT)),
    ("NMCFO", TableRef::Range(&JA_NMCFO)),
    ("NMCME", TableRef::Range(&JA_NMCME)),
    ("NMCWE", TableRef::Range(&JA_NMCWE)),
    ("NMCCY", TableRef::Range(&JA_NMCCY)),
    ("NMCFA", TableRef::Range(&JA_NMCFA)),
    ("NMCSU", TableRef::Range(&JA_NMCSU)),
    ("SCST", TableRef::Plain(&JA_SCST)),
    ("SCSA", TableRef::Plain(&JA_SCSA)),
    ("SCSB", TableRef::Plain(&JA_SCSB)),
    ("SCSC", TableRef::Plain(&JA_SCSC)),
    ("SCSR", TableRef::ScreamSheet),
    ("VMCT", TableRef::Range(&JA_VMCT)),
    ("VMCE", TableRef::Plain(&JA_VMCE)),
    ("VMCF", TableRef::Plain(&JA_VMCF)),
    ("VMCS", TableRef::Plain(&JA_VMCS)),
    ("VMCR", TableRef::Chain(&JA_VMCR)),
    ("STOREA", TableRef::Plain(&JA_STOREA)),
    ("STOREB", TableRef::Plain(&JA_STOREB)),
    ("STOREC", TableRef::Plain(&JA_STOREC)),
    ("STORE", TableRef::ShopPeople),
];

pub(crate) static JA_SYSTEM: SystemTables = SystemTables {
    tables: JA_TABLES,
    critical: "決定的成功！",
    fumble: "決定的失敗！",
    success: "成功",
    failure: "失敗",
    news: "ニュース",
    scream_sheet: ScreamSheetTables {
        type_table: &JA_SCST,
        a_table: &JA_SCSA,
        of_table: &JA_SCSOF,
        b_table: &JA_SCSB,
        c_table: &JA_SCSC,
    },
    shop_people: ShopPeopleTables {
        staff_table: &JA_STOREA,
        people_a_table: &JA_STOREB,
        people_b_table: &JA_STOREC,
        intro: "おまえが立ち寄ったボデガには――",
        shop_staff: "――といった店員と、",
        people_a: "――という印象の客と、",
        people_b: "――という感じの客がいるようだ。",
        outro: "どうにも、嫌な予感がする。",
    },
};

/// Ruby `ScreamSheetRandomizerTable#roll`。
fn roll_scream_sheet(sys: &SystemTables, rng: &mut Randomizer) -> Result<String, EvalError> {
    let t = &sys.scream_sheet;
    let mut result = String::new();

    let dice = rng.roll_once(6)?;
    let scs_type = t.type_table.choice(dice);
    result.push_str(&format!("{scs_type}{}　『", sys.news));

    let dice = rng.roll_once(10)?;
    result.push_str(t.a_table.choice(dice).last_body());

    let dice = rng.roll_once(6)?;
    result.push_str(t.of_table.choice(dice).last_body());

    let dice = rng.roll_once(10)?;
    result.push_str(t.a_table.choice(dice).last_body());

    let dice = rng.roll_once(6)?;
    result.push_str(t.of_table.choice(dice).last_body());

    let dice = rng.roll_once(10)?;
    result.push_str(t.b_table.choice(dice).last_body());

    let dice = rng.roll_once(10)?;
    result.push_str(t.c_table.choice(dice).last_body());

    result.push('』');
    Ok(result)
}

/// Ruby `String#[0..-2]`（末尾1文字を落とす）。
fn drop_last_char(s: &str) -> &str {
    match s.char_indices().last() {
        Some((i, _)) => &s[..i],
        None => "",
    }
}

/// Ruby `ShopPeopleTable#roll`。
fn roll_shop_people(sys: &SystemTables, rng: &mut Randomizer) -> Result<String, EvalError> {
    let t = &sys.shop_people;
    let mut result = String::from(t.intro);

    let dice = rng.roll_once(6)?;
    let staff = t.staff_table.choice(dice);
    result.push_str(drop_last_char(staff.last_body()));
    result.push_str(t.shop_staff);

    let dice = rng.roll_once(6)?;
    let people = t.people_a_table.choice(dice);
    result.push_str(drop_last_char(people.last_body()));
    result.push_str(t.people_a);

    let dice = rng.roll_once(6)?;
    let people = t.people_b_table.choice(dice);
    result.push_str(drop_last_char(people.last_body()));
    result.push_str(t.people_b);
    result.push_str(t.outro);

    Ok(result)
}

/// Ruby `Base#roll_tables(command, TABLES)`。
fn roll_tables(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    let Some((_, table)) = sys.tables.iter().find(|(key, _)| *key == command) else {
        return Ok(None);
    };
    let text = match table {
        TableRef::Plain(table) => table.roll(rng)?.to_string(),
        TableRef::Range(table) => table.roll(rng)?.to_string(),
        TableRef::Chain(table) => table.roll(rng)?.to_string(),
        TableRef::ScreamSheet => roll_scream_sheet(sys, rng)?,
        TableRef::ShopPeople => roll_shop_people(sys, rng)?,
    };
    Ok(Some(text))
}

/// Ruby `CyberpunkRed#cp_roll_result`。
fn cp_roll_result(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    let parser = PARSER.get_or_init(|| {
        Parser::new(&["CP"], RoundType::Floor)
            .enable_suffix_number()
            .restrict_cmp_op_to(&[None, Some(CmpOp::Gt)])
    });
    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };

    let dice_cnt = 1;
    let dice_face = 10;
    let mut modify_number: I = I::ZERO;
    let mut total: I = I::ZERO;

    let mut result = EvalResult::new();

    let first = rng.roll_once(dice_face)?;
    total += first;
    modify_number += I::from(
        parsed
            .suffix_number
            .as_ref()
            .map(crate::randomizer::sat_i64)
            .unwrap_or(0),
    );
    modify_number += parsed.modify_number;
    total += modify_number.clone();

    // 10 なら決定的成功、1 なら決定的失敗。どちらももう1個振る。
    let mut last = None;
    match first {
        10 => {
            let d = rng.roll_once(dice_face)?;
            total += d;
            result.critical = true;
            last = Some(d);
        }
        1 => {
            let d = rng.roll_once(dice_face)?;
            total -= d;
            result.fumble = true;
            last = Some(d);
        }
        _ => {}
    }

    if let Some(target) = parsed.target_number.clone() {
        result.set_condition(total > target);
    }

    // Ruby: "#{parsed.cmp_op}#{parsed.target_number}"（nil は空文字列）
    let cmp_op = parsed.cmp_op.map(|op| op.symbol_str()).unwrap_or("");
    let target = parsed
        .target_number
        .map(|t| t.to_string())
        .unwrap_or_default();
    let modifier = format::modifier(&modify_number);

    let mut text = format!("({dice_cnt}D{dice_face}{modifier}{cmp_op}{target})");
    text.push_str(" ＞ ");
    text.push_str(&format!("{first}[{first}]{modifier}"));
    text.push_str(" ＞ ");

    if result.critical {
        text.push_str(&format!("{} ＞ ", sys.critical));
        if let Some(d) = last {
            text.push_str(&format!("{d}[{d}] ＞ "));
        }
    }
    if result.fumble {
        text.push_str(&format!("{} ＞ ", sys.fumble));
        if let Some(d) = last {
            text.push_str(&format!("{d}[{d}] ＞ "));
        }
    }

    text.push_str(&total.to_string());

    if result.success {
        text.push_str(&format!(" ＞ {}", sys.success));
    }
    if result.failure {
        text.push_str(&format!(" ＞ {}", sys.failure));
    }

    result.text = text;
    Ok(Some(result))
}

/// Ruby `CyberpunkRed#eval_game_system_specific_command`。
pub(crate) fn eval_specific_command(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(result) = cp_roll_result(sys, command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    Ok(roll_tables(sys, command, rng)?.map(SpecificCommandOutput::text))
}

/// Ruby `BCDice::GameSystem::CyberpunkRed`（ID: `CyberpunkRed`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CyberpunkRed;

impl GameSystem for CyberpunkRed {
    fn id(&self) -> &'static str {
        "CyberpunkRed"
    }

    fn name(&self) -> &'static str {
        "サイバーパンクRED"
    }

    fn sort_key(&self) -> &'static str {
        "さいはあはんくれつと"
    }

    fn help_message(&self) -> &'static str {
        r"・判定　CPx+y>z
　(x＝能力値と技能値の合計、y＝修正値、z＝難易度 or 受動側　x、y、zは省略可)
　例）CP12 CP10+2>12　CP7-1　CP8+4　CP7>12　CP　CP>9

各種表
・致命的損傷表
　FFD　：身体への致命的損傷
　HFD　：頭部への致命的損傷
・遭遇表
　NCDT　：ナイトシティ(日中)
　NCMT　：ナイトシティ(深夜)
・スクリームシート
　SCSR　：スクリームシート(ランダム)
　SCST　：スクリームシート分類
　SCSA　：ヘッドラインA
　SCSB　：ヘッドラインB
　SCSC　：ヘッドラインC
・最寄りの自販機
　VMCR　：最寄りの自販機表
　VMCT　：自販機タイプ決定表
　VMCE　：食品
　VMCF　：ファッション
　VMCS　：変なもの
・ボデガの客
　STORE　：ボデガの客と店員
　STOREA　：店主またはレジ係
　STOREB　：変わった客その1
　STOREC　：変わった客その2
・夜の市
　NMCT　：商品の分野
　NMCFO　：食品とドラッグ
　NMCME　：個人用電子機器
　NMCWE　：武器と防具
　NMCCY　：サイバーウェア
　NMCFA　：衣料品とファッションウェア
　NMCSU　：サバイバル用品
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "CP", "FFD", "HFD", "NCDT", "NCMT", "NMCT", "NMCFO", "NMCME", "NMCWE", "NMCCY",
            "NMCFA", "NMCSU", "SCST", "SCSA", "SCSB", "SCSC", "SCSR", "VMCT", "VMCE", "VMCF",
            "VMCS", "VMCR", "STOREA", "STOREB", "STOREC", "STORE",
        ]
    }

    crate::impl_prefixes_pattern!();

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
            .join("test/data/CyberpunkRed.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/CyberpunkRed.toml` の全ケースが通ること。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            eprintln!("skip: test/data/CyberpunkRed.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("CyberpunkRed.toml must parse");
        assert_eq!(
            data.tests.len(),
            56,
            "case count in test/data/CyberpunkRed.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "CyberpunkRed",
                "unexpected game system in CyberpunkRed.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("CyberpunkRed"), &tc.input, &mut src) {
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
                    "FAIL CyberpunkRed:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} CyberpunkRed cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
