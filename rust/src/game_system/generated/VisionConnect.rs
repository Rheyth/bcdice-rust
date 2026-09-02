//! `lib/bcdice/game_system/VisionConnect.rb` の手書き移植。

use crate::command_parser::{Parser, SuffixPosition};
use crate::dice_table::{D66RangeTable, RangeInc, RollableTable, Table};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format::modifier;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisionConnect;

impl GameSystem for VisionConnect {
    fn id(&self) -> &'static str {
        "VisionConnect"
    }
    fn name(&self) -> &'static str {
        "ヴィジョンコネクト"
    }
    fn sort_key(&self) -> &'static str {
        "ういしよんこねくと"
    }
    fn help_message(&self) -> &'static str {
        r"・判定(VC+x@c#f>=y)
  !：コマンドの最初に付けると致命的失敗が全てアクシデントになる。
  x：修正値。能力値、戦闘値、その他修正値など。省略可。
  y：目標値。省略時は決定的成功/致命的失敗のみ表示。
  c：クリティカル値。@ごと省略可。省略時は12。
  f：ファンブル値。#ごと省略可。省略時は3。
  (例)VC+3>=8
      VC+7@11>=12
      !VC+6-1#4>=10

・各種表
  アクシデント表 AT
  トラブル表 TT
"
    }
    fn prefixes(&self) -> &'static [&'static str] {
        &["!?VC", "AT", "TT"]
    }
    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(parsed) = Parser::new(&["!?VC"], RoundType::Floor)
            .enable_critical()
            .enable_fumble()
            .restrict_cmp_op_to(&[None, Some(CmpOp::Ge)])
            .parse(command)
        {
            let stamina_zero = parsed.command.starts_with('!');
            let critical_target = parsed
                .critical
                .as_ref()
                .map(crate::randomizer::sat_i64)
                .unwrap_or(12);
            let fumble_target = parsed
                .fumble
                .as_ref()
                .map(crate::randomizer::sat_i64)
                .unwrap_or(3);
            let accident_target = if stamina_zero { fumble_target } else { 2 };
            let dice = rng.roll_barabara(2, 6)?;
            let dice_sum: i64 = dice.iter().sum();
            let result_sum = dice_sum + parsed.modify_number.clone();
            let critical = dice_sum >= critical_target;
            let fumble = dice_sum <= fumble_target;
            let accident = dice_sum <= accident_target;
            let (condition, result_str) = if critical {
                (Some(true), Some("決定的成功"))
            } else if accident {
                (Some(false), Some("致命的失敗(アクシデント)"))
            } else if fumble {
                (Some(false), Some("致命的失敗(トラブル)"))
            } else if let Some(target) = parsed.target_number.clone() {
                if result_sum >= target {
                    (Some(true), Some("成功"))
                } else {
                    (Some(false), Some("失敗"))
                }
            } else {
                (None, None)
            };
            let mut sequence = vec![
                format!("({})", parsed.to_s(SuffixPosition::AfterModifyNumber)),
                format!(
                    "{}[{},{}]{}",
                    dice_sum,
                    dice[0],
                    dice[1],
                    modifier(&parsed.modify_number)
                ),
                result_sum.to_string(),
            ];
            if let Some(text) = result_str {
                sequence.push(text.to_owned());
            }
            let mut result = EvalResult::with_text(sequence.join(" ＞ "));
            result.critical = critical;
            result.fumble = fumble;
            if let Some(success) = condition {
                result.set_condition(success);
            }
            return Ok(Some(SpecificCommandOutput::result(result)));
        }

        let table: Option<&dyn RollableTable> = match command {
            "AT" => Some(&AT),
            "TT" => Some(&TT),
            _ => None,
        };
        match table {
            Some(table) => Ok(Some(SpecificCommandOutput::text(
                table.roll(rng)?.to_string(),
            ))),
            None => Ok(None),
        }
    }
}

static AT: Table = Table::from_dice("アクシデント表", 1, 6, &[
    "頭がぼんやりして、まぶたが重くなってきた……。これは睡魔の襲来？　キャラクターの操作がおぼつかなくなる。シーン終了まで能力値判定、戦闘値判定の達成値に-3される。スタミナを3点消費することで、この効果を打ち消すことができる。",
    "キーボード、マウス、ゲームパッドなどが操作不能になった！　キャラクターを操作することができない。戦闘中の場合は次のラウンドの準備プロセス終了までキャラクターアクションを行うことができず、スキルや特技の使用もできない。スタミナを3点消費することで、この効果を打ち消すことができる。",
    "急に画面が真っ暗に！　パソコンやゲーム機を見ると、動作はしている。これはモニタの問題かっ！　キャラクターを操作することができない。戦闘中の場合は次のラウンドの終了までキャラクターアクションを行うことができず、スキル、特技の使用もできない。スタミナを4点消費することで、この効果を打ち消すことができる。",
    "突然、通信回線が不調となり、切断されてしまった！　急いで再ログインしなければ！　シーンから自動的に退場となる。戦闘中の場合は次のラウンドの準備プロセス終了後、登場できる。スタミナを4点消費することで、この効果を打ち消すことができる。",
    "いきなり画面が真っ黒になり、パソコンやゲーム機が再起動し始めた……。シーンから自動的に退場となる。戦闘中の場合は次のラウンドの終了後、登場できる。スタミナを5点消費することで、この効果を打ち消すことができる。",
    "突然、画面が消えた。いや、画面だけじゃない。電化製品がすべて止まっているようだ。もしや、これは停電！？　シーンから自動的に退場となる。次のシーンの開始時に登場できる。スタミナを5点消費することで、この効果を打ち消すことができる。",
]);
static TT: D66RangeTable = D66RangeTable::new("トラブル表", &[
    (RangeInc::new(11, 13), "チャットで誤爆(発言ミス)をしてしまった。恥ずかしさで、スタミナが1点減少する。"),
    (RangeInc::new(14, 16), "かまってほしいのか、ペットがちょっとした悪戯をしてきた。ごめん、今は忙しいのだ。ペットを取得していない場合は何も起こらない。ペットを取得していた場合、罪悪感によりスタミナが1点減少する。"),
    (RangeInc::new(21, 23), "何かの用事があるのか、それとも食事の時間なのか、家族から声を掛けられた。家族を取得していない場合は何も起こらない。家族を取得していた場合、気が焦ってスタミナが2点減少する。"),
    (RangeInc::new(24, 26), "玄関のチャイムが鳴り、「宅配便でーす」の声が外から聞こえてきた。こんな時にっ！？　家族がいれば、荷物を受け取ってもらえるのだが……。家族を取得している場合は何も起こらない。家族を取得していない場合、スタミナが2点減少する。"),
    (RangeInc::new(31, 33), "操作中に勢い余って腕が飲み物に当たってしまい、中身がこぼれてしまった。あとで掃除しないと……。ドリンクを取得していない、あるいはすべて使用済みである場合は何も起こらない。ドリンクを1個失う。"),
    (RangeInc::new(34, 36), "キーボードやゲームパッドの調子があまりよくない。やっぱり、ゲーミングデバイスに買い換えた方がいいか……。デバイスを取得している場合は何も起こらない。デバイスを取得していない場合、ストレスによりスタミナが2点減少する。"),
    (RangeInc::new(41, 43), "知り合いから電話が掛かってきた。電話しながらの操作はちょっと大変だ。より集中しなければならないため、スタミナが2点減少する。"),
    (RangeInc::new(44, 46), "急にお手洗いに行きたくなってきた。ちょっと我慢しなければならないため、スタミナが2点減少する。"),
    (RangeInc::single(51), "レアモンスターがポップ(出現)したとチャットで通知が来た！　でも、今は行くことができない……。ブレイブを取得していない場合は何も起こらない。ブレイブを取得している場合、悔しさでスタミナが3点減少する。"),
    (RangeInc::single(52), "出品しているアイテムのマーケットでの相場が下がったと知り合いからチャットが飛んできた。マイスターを取得していない場合は何も起こらない。マイスターを取得している場合、悲しさでスタミナが3点減少する。"),
    (RangeInc::single(53), "操作の方法が分からなくなって、焦りまくる。ノービスを取得していない場合は何も起こらない。ノービスを取得している場合、混乱でスタミナが3点減少する。"),
    (RangeInc::single(54), "ギルドのメンバーからギルドを抜けたいという相談のチャットが飛んできた。リーダーを取得していない場合は何も起こらない。リーダーを取得している場合、驚きのあまりスタミナが3点減少する。"),
    (RangeInc::single(55), "知り合いから攻略の手伝いを頼むチャットが飛んできた。ごめんなさい、今はちょっと無理……。ヘルパーを取得していない場合は何も起こらない。ヘルパーを取得している場合、申し訳なさでスタミナが3点減少する。"),
    (RangeInc::single(56), "つきまとってくるユーザーから、しつこくチャットが飛んでくる。面倒くさいなぁ。フェイバリットを取得していない場合は何も起こらない。フェイバリットを取得している場合、煩わしさでスタミナが3点減少する。"),
    (RangeInc::single(61), "誰かと一緒にプレイするのに慣れていないためか、ちょっと緊張しているかもしれない。ローンウルフを取得していない場合は何も起こらない。ローンウルフを取得している場合、緊張でスタミナが3点減少する。"),
    (RangeInc::single(62), "事前に入手していた情報が間違っていた！？　どう対応してよいか分からず、焦りまくる。ブレインを取得していない場合は何も起こらない。ブレインを取得している場合、焦りのあまりスタミナが3点減少する。"),
    (RangeInc::single(63), "配信でトラブルが発生！？　対応に慌ててしまう。ストリーマーを取得していない場合は何も起こらない。ストリーマーを取得している場合、狼狽によってスタミナが3点減少する。"),
    (RangeInc::single(64), "使っているゲーミングデバイスの調子がよくない。ガジェッターを取得していない場合は何も起こらない。ガジェッターを取得している場合、いらだちでスタミナが3点減少する。"),
    (RangeInc::single(65), "合間にプレイしている別のゲームや流し見していた動画に注意が向いて、操作をミスしてしまう。カジュアルを取得していない場合は何も起こらない。カジュアルを取得している場合、後悔でスタミナが3点減少する。"),
    (RangeInc::single(66), "ハードコアを取得していない場合は何も起こらない。トラブル表の51～65の項目の効果を受ける。ハードコア以外に取得しているスタイルに合わせて、効果を適用すること(たとえば、ブレイブならば51、マイスターなら52となる)。"),
]);

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "VisionConnect",
            "VisionConnect.toml",
            54,
        );
    }
}
