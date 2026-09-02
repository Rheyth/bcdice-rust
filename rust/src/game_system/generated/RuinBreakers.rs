use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int as I;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuinBreakers;

impl GameSystem for RuinBreakers {
    fn id(&self) -> &'static str {
        "RuinBreakers"
    }
    fn name(&self) -> &'static str {
        "ルーインブレイカーズ"
    }
    fn sort_key(&self) -> &'static str {
        "るういんふれいかあす"
    }
    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }
    fn prefixes(&self) -> &'static [&'static str] {
        &["RB", "FP[DR]", "PE", "NE", "DXM", "JC", "RDF", "TC", "DA"]
    }
    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific(command, rng)
    }
}

static HELP_MESSAGE: &str = r"■ 基本判定 (RBx@y#z)
  x：成功率、y：クリティカル値（省略可）、z：ファンブル値（省略可）
  1D100を振って、成功率に応じて成功／失敗／クリティカル／ファンブルの判定を行います。(P.60)
  クリティカル値を省略した場合は成功率の5分の1（切り捨て、最低1）
  ファンブル値を省略した場合は、成功率が99以下の場合は96、100以上の場合は99
  例） RB32, RB(45+20)/2, RB30@10, RB35+20#90, RB40-20+10@10#90

■ FPへのダメージ (FPDx)
  x：破滅ポイント
  ルーインブレイクロール失敗時やラウンド終了時に、残っている
  破滅ポイントに応じて発生するダメージのダイスロールを行います。(P.91,92)
  例） FPD23

■ FPの回復 (FPRx)
  x：破滅ポイント
  ルーインブレイク成功時に発生する、FPの回復量を決定するダイスロールを行います。(P.93)
  例） FPR29

■ 各種表
  ・ポジティブ感情表 (PE)
  ・ネガティブ感情表 (NE)
  ・デウス・エクス・マキナ表 (DXM)
  ・断罪チャート (JC)
  ・破滅のイヤな感じ表 (RDF)
  ・トラブルチャート／トラブル解決チャート (TC)
  ・ドタバタアクション表 (DA)
";

fn rb_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^RB(.+?)(?:@(\d+))?(?:#(\d+))?$").expect("valid regex"))
}

fn eval_specific(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if command.starts_with("RB") {
        return check_roll(command, rng);
    }
    if let Some(value) = command.strip_prefix("FPD").and_then(|s| s.parse().ok()) {
        return Ok(Some(SpecificCommandOutput::text(fp_damage(value, rng)?)));
    }
    if let Some(value) = command.strip_prefix("FPR").and_then(|s| s.parse().ok()) {
        return Ok(Some(SpecificCommandOutput::text(fp_recovery(value, rng)?)));
    }
    let Some((title, sides, table)) = table(command) else {
        return Ok(None);
    };
    let number = rng.roll_once(sides)?;
    let index = if sides == 100 {
        (number - 1) / 5
    } else if command == "DXM" {
        (number - 1) / 2
    } else {
        number - 1
    };
    Ok(Some(SpecificCommandOutput::text(format!(
        "{title}({number}) ＞ {}",
        table[index as usize]
    ))))
}

fn check_roll(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let Some(m) = rb_pattern().captures(command) else {
        return Ok(None);
    };
    let Some(success_rate) = arithmetic::eval(&m[1], RoundType::Floor)? else {
        return Ok(None);
    };
    let critical: I = m
        .get(2)
        .and_then(|v| v.as_str().parse::<i64>().ok().map(I::from))
        .unwrap_or_else(|| (success_rate.clone() / I::from(5)).max(I::ONE));
    let fumble: I = m
        .get(3)
        .and_then(|v| v.as_str().parse::<i64>().ok().map(I::from))
        .unwrap_or(if success_rate < I::from(100) {
            I::from(96)
        } else {
            I::from(99)
        });
    let total = rng.roll_once(100)?;
    let total_i = I::from(total);
    let (text, result) = if total_i >= fumble {
        ("ファンブル", EvalResult::fumble(""))
    } else if total_i == I::ONE || total_i <= critical {
        ("クリティカル", EvalResult::critical(""))
    } else if total_i <= success_rate {
        ("成功", EvalResult::success(""))
    } else {
        ("失敗", EvalResult::failure(""))
    };
    let mut result = result;
    result.text = format!("(1D100<={success_rate}@{critical}#{fumble}) ＞ {total} ＞ {text}");
    Ok(Some(SpecificCommandOutput::result(result)))
}

fn fp_damage(point: i64, rng: &mut Randomizer) -> Result<String, EvalError> {
    let tens = point / 10;
    let ones = point % 10;
    let dice = rng.roll_barabara(1 + tens, 10)?;
    let total: i64 = dice.iter().sum();
    let list = dice
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    Ok(format!(
        "((1+{tens})D10+{ones}) ＞ {total}[{list}]+{ones} ＞ {}ダメージ",
        total + ones
    ))
}

fn fp_recovery(point: i64, rng: &mut Randomizer) -> Result<String, EvalError> {
    let count = (point + 9) / 10;
    let dice = rng.roll_barabara(count, 10)?;
    let total: i64 = dice.iter().sum();
    let list = dice
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    Ok(format!("({count}D10) ＞ {total}[{list}] ＞ {total}回復"))
}

fn table(command: &str) -> Option<(&'static str, i64, &'static [&'static str])> {
    match command {
        "PE" => Some(("ポジティブ感情表", 100, PE)),
        "NE" => Some(("ネガティブ感情表", 100, NE)),
        "DXM" => Some(("デウス・エクス・マキナ表", 10, DXM)),
        "JC" => Some(("断罪チャート", 10, JC)),
        "RDF" => Some(("破滅のイヤな感じ表", 100, RDF)),
        "TC" => Some(("トラブルチャート／トラブル解決チャート", 10, TC)),
        "DA" => Some(("ドタバタアクション表", 10, DA)),
        _ => None,
    }
}

static PE: &[&str] = &[
    "【希望】相手はまるで自分の過去、あるいは未来を見ているように感じる。",
    "【礼儀】相手に礼を尽くすべきだとあなたは考えている。",
    "【家族】相手とは家族のような関係となる。",
    "【恩人】相手から助けを受けたことがある。それは大事な思い出だ。",
    "【友人】相手とはなんとなくウマが合う。一緒にいると楽しい。",
    "【信用】相手は信用できる人物だと思う。",
    "【仲間】相手は同じ目的を持つ仲間だ。",
    "【庇護】相手のことを助けてあげたいと思っている。",
    "【尊敬】相手の行動、思考、思想などを尊敬している。",
    "【憧れ】相手の生き方、外見、能力などになんとなく憧れている。",
    "【好意】相手の主張、外見、生き方などに好意を抱いている。",
    "【忠義】相手に対して真摯に忠実でありたいと思っている。",
    "【目標】相手はあなたにとっての目標であり、理想の存在だ。",
    "【借り】相手から助けを受けた。それはいつか返すべき、借りだ。",
    "【貸し】相手には貸しがある。別に返してもらおうとは思っていない。",
    "【腐れ縁】相手は昔から何かというと縁がある。この縁は今も続いている。",
    "【相性】相手とはなんとなくうまくいく。相性がいいようだ。",
    "【有為】相手はあなたにとって益をもたらす人物だ、そう考えている。",
    "【秘密】相手の秘密を知っている。あるいはお互い秘密を共有している。",
    "【好敵手】相手のことを好敵手、ライバルだと思っている。",
];
static NE: &[&str] = &[
    "【同族嫌悪】1日に自分の忌むべき過去、あるいは自分自身を見ているように感じる。",
    "【侮蔑】相手を蔑む気持ちがある。どうにも、気に入らない。",
    "【反発】相手の主張や行動などに反発を感じる。相手を受け入れることに抵抗がある。",
    "【わだかまり】相手には言葉にしにくいもやもやとした感情を持っている。",
    "【隔たり】相手とはなんとなくウマが合わない。一緒にいても面白くない。",
    "【疑惑】相手は信用できない人物だと思っている。",
    "【裏切り】相手に裏切られたという気持ちがある。",
    "【妨害】相手のことを気に入らず、何かあれば、邪魔したいと思っている。",
    "【侮辱】相手の行動、思考、思想などを嫌悪している。",
    "【うらやみ】相手の生き方、外見、能力などをうらやんでいる。",
    "【害意】相手の主張、外見、生き方などを嫌い、害を与えたいと思っている。",
    "【不快】相手を不快な人間だと思っている。生理的に受け付けない。",
    "【反面】相手を反面教師としている。ああはなるまい、と。",
    "【詐欺】相手に騙されているように思う。何か嘘を吐かれているように思うのだ。",
    "【搾取】相手に自分の何かを奪われているような怒りを感じる。",
    "【悪縁】相手は昔から縁がある。この縁を絶ちきりたいと思っている。",
    "【相性】相手とはなんとなくうまくいかない。残念だが相性が悪い。",
    "【害悪】相手はあなたにとって害をもたらす、そう思っている。",
    "【怨恨】相手に恨みを持っている。この恨みを晴らす日は来るだろうか。",
    "【仇敵】相手のことを倒すべき相手と思っている。",
];
static DXM: &[&str] = &["神降臨。エンディングフェイズに効果を発揮する。あなたの願いはかなう。願いの内容はGMと相談して決定すること。","逃走。状況を無視してあなた以外のキャストはシーンから退場できる。","命の雫。あなた以外のキャストのFPが3D10点だけ回復する。","天変地異。巨大な嵐や地震、雷雨などが発生し、周囲は大混乱に陥る。トループやエキストラはシーン終了まで何も行なえない(戦闘不能として扱う)。","不思議なことが起こった。あなたのFPが完全に回復する。"];
static JC: &[&str] = &["【国王／女王】国レベルの代表者が現われて、あなたの主張を支持してくれる。","【王子／王女】王子や王女といった国で知らぬ者がないような存在が、あなたの主張を支持してくれる。","【高位聖職者】高位の聖職者が、あなたの主張を支持してくれる。","【有力貴族】有力貴族が、あなたの主張を支持してくれる。","【有力市民】有力市民が、あなたの主張を支持してくれる。","【豪商】豪商が、あなたの主張を支持してくれる。","【現役学生たち】アカデミーの学生たちが、あなたの主張を支持してくれる。","【OB、OGたち】アカデミーのOBやOGが、あなたの主張を支持してくれる。","【多くの人々】名も知れぬ多くの人々が、あなたの主張を支持してくれる。","【外国の王侯貴族】外国の代表者が現われて、あなたの主張を支持してくれる。"];
static RDF: &[&str] = &[
"【水中で拘束】\n演出：水中で長い髪の毛が全身に絡みついて動きが重くなるような感覚。\nルーインブレイク成功：重い拘束から解き放たれたような快感。","【鈍痛】\n演出：こめかみから長い釘を差し込まれているような感覚。\nルーインブレイク成功：痛みが消えてなくなる安堵感。","【酸欠】\n演出：空気が薄くなり呼吸をしても息苦しさが消えない感覚。\nルーインブレイク成功：清浄な空気を吸った時の快感。","【ヘッドロック】\n演出：頭を締め上げられているような感覚。\nルーインブレイク成功：痛みから逃れられた安心感。","【悪寒】\n演出：背中が冷やりとして悪寒が全身を突き抜けるような感覚。\nルーインブレイク成功：悪寒が鎮まった平穏感。","【熱病】\n演出：熱病で浮かされたように頭がぼうっとする感覚。\nルーインブレイク成功：落ち着きを取り戻した安息感。","【高所恐怖】\n演出：目もくらむような断崖の際に立たされたような感覚。\nルーインブレイク成功：落下の恐怖から逃れた安堵感。","【ガラスの破片】\n演出：砕けた散ったガラスの破片を踏み続けるような感覚。\nルーインブレイク成功：幻の痛みが消えていく安心感。","【ジャリ感】\n演出：口の中に砂を詰め込まれたような感覚。\nルーインブレイク成功：口の中がすっきりしたような清浄感。","【耳鳴り】\n演出：耳をふさいでも聞こえる耳鳴りが響き続けているような感覚。\nルーインブレイク成功：異音が消えた平安感。","【孤独】\n演出：虚空にただひとり浮かんでいるような孤独な感覚。\nルーインブレイク成功：孤立から脱した安心感。","【落下感】\n演出：高所から落ち続けているような感覚。\nルーインブレイク成功：地に足のついた安定感。","【暗所恐怖】\n演出：明るいはずなのに周囲が真っ暗で何も見えない不安な感覚。\nルーインブレイク成功：周囲がハッキリ見える安息感。","【擦過】\n演出：心の表面をザラザラとしたもので削られているような感覚。\nルーインブレイク成功：痛みから逃れられた安楽感。","【幻聴】\n演出：周囲に人がいて、絶えず自分の悪口を囁きあっているような感覚。\nルーインブレイク成功：周囲への恐怖が消えた平穏感。","【異臭】\n演出：不快な香りが漂ってくるような感覚。\nルーインブレイク成功：異臭を感じなくなった清浄感。","【健忘感】\n演出：何かを忘れていて、それが何かは思い出せないような感覚。\nルーインブレイク成功：忘れごとを思い出せたときの開放感。","【杞憂】\n演出：天が崩れていつ落ちてくるかわからない感覚。\nルーインブレイク成功：頭上がすっきりした痛快感。","【背後恐怖】\n演出：背後に人が立っているような感覚。\nルーインブレイク成功：後方に憂いのない安心感。","【夢中感】\n演出：夢の中にいるような不安な感覚。\nルーインブレイク成功：しっかりとした現実感。",
];
static TC: &[&str] = &["【暴れ馬／交通事故】\nトラブル：いきなり、暴れ馬がやってきて、キミは刎ねられた。\n解決：時間はかかったが、事故は処理された。","【突然の崩落／地下遺跡へ移動】\nトラブル：周辺ごと地面が陥没し、地下へと導かれる。\n解決：崩落した先は謎の古代文明の遺跡であった。","【暗殺者の襲撃】\nトラブル：凶刃がキャストを襲う。\n解決：何とか暗殺者の手を逃れ、キミは生還した。","【拉致・誘拐】\nトラブル：突然、キミは黒覆面の男たちに馬車に押し込まれ、誘拐される。\n解決：何とかして、キミは誘拐組織の手を逃れた。","【爆発！！】\nトラブル：爆発した！\n解決：奇跡的にキミは無傷だ、周囲には破壊されたガレキが転がっている。","【行きずりの強盗】\nトラブル：訪れていた店やレストラン、銀行などが強盗に襲われる。\n解決：通りすがりのヒーローが強盗を倒した。あれはいったい。","【テロリストの襲撃／撃退】\nトラブル：テロリストに襲われる。\n解決：テロリストは撃退された。","【交通マヒ／移動変更】\nトラブル：直接、事故に行きあったわけではない事故によって起こった交通マヒによって身動きが取れない。\n解決：交通機関を変更して移動することになった。","【軍・警察の封鎖／大捕物】\nトラブル：突如して軍や警察などの治安組織によって建物が封鎖されてしまった。\n解決：建物内にいる犯人を巡り、大捕物が始った。","【任意】\nGMと相談してトラブルの内容を決めよう。"];
static DA: &[&str] = &["【フードファイト（野菜）】大根ソードで切りつけ、カボチャハンマーで殴り抜け","【ホコリの雲】ドカッ、バキ、ボカッ。キュウ。","【リビングルームストーム】飛び交うソーサー、ポットの中には煎れたばかりの紅茶（抽出温度28度）が入っているぞ。","【廊下でランナウェイ】廊下を走っては行けません。","【図書館バトル】敏腕司書が、図書館の静寂を乱す者を残らず静かにさせていく。","【パーティーファイト】優雅に踊り、紳士淑女の助けを借りて悪漢を退治しよう。","【フードファイト（肉と骨）】ヒトに眠る野性を解き放て。羊の骨が最古の武器として再発見される。","【イスと机】イスは盾であり、武器であり悪漢をけん制し、拘束する。","【洗濯物ファイト】シーツで敵の動きを止めて、石鹸で転ばせよう。","【任意】GMと相談して、イメージをふくらませよう。"];

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "RuinBreakers",
            "RuinBreakers.toml",
            34,
        );
    }
}
