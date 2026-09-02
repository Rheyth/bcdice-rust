//! P4で手書き移植した `lib/bcdice/game_system/DetatokoSaga_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `DetatokoSaga` を継承し、`TABLES` を `translate_tables(:ko_kr)` に差し替えて
//! `@locale` を `:ko_kr` に変えるだけ（判定メソッドの上書きは無い）なので、
//! 実装は [`super::DetatokoSaga`] のものをそのまま使い、
//! ここには `ko_kr` ロケールの表と定型文だけを置く。
//!
//! 表と文言は `i18n/DetatokoSaga/ko_kr.yml` と `i18n/ko_kr.yml`（`success` / `failure`）から
//! 機械的に書き出したもので、値は1文字も変えていない。

use super::DetatokoSaga::{eval_specific_command, SystemTables};
use crate::dice_table::Table;
use crate::enums::D66SortType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// i18n `ko_kr.success`。`DetatokoSaga` の [`SystemTables`] は判定コマンド用の文言しか
/// 持たないので、`Base#result_ndx` が使う分だけここに置く。
const KO_SUCCESS: &str = "성공";
/// i18n `ko_kr.failure`。
const KO_FAILURE: &str = "실패";

/// i18n `DetatokoSaga.table.SST.items`。
static KO_SST_ITEMS: &[&str] = &[
    "당신은 【낙인】을 2개 받는다. 이 표를 다시 2번 굴려 받을 【낙인】을 정한다(그 경우, 다시 이 눈이 나와도 【낙인】은 늘어나지 않는다).",
    "【상처】 심한 상처를 입었다. 어떻게든 싸울 수는 있지만…….",
    "【출혈】 피가 흘러넘쳐, 눈이 흐릿하다…….",
    "【쇠약】 몸이 약해져, 그 마음마저도 시들어버릴 거 같다.",
    "【고통】 아픔과 괴로움, 한심함. 눈에서 눈물이 새어 나온다.",
    "【충격】 날려져서, 벽이나 나무에 부딪힌다. 빨리 일어서지 않으면.",
    "【피로】 당신의 얼굴에 피로의 색이 강해진다……이 싸움이 힘겨워졌다.",
    "【노호】 성가신 공격에 분노의 함성을 지른다. 분노가 싸움을 어렵게 할까?",
    "【부상】 상처를 입었다…….",
    "【경상】 당신의 피부에 상처가 남았다. 이것만이라면 아무렇지도 않다.",
    "기적적으로 당신은 【낙인】을 받지 않았다.",
];
/// i18n `DetatokoSaga.table.SST`（체력 낙인표 / 2D6）。
static KO_SST: Table = Table::from_dice("체력 낙인표", 2, 6, KO_SST_ITEMS);

/// i18n `DetatokoSaga.table.WST.items`。
static KO_WST_ITEMS: &[&str] = &[
    "당신은 【낙인】을 2개 받는다. 이 표를 다시 2번 굴려 받을 【낙인】을 정한다(그 경우, 다시 이 눈이 나와도 【낙인】은 늘어나지 않는다).",
    "【절망】 어떻게 하지 못하는 상황. 희망은 사라지고……무릎을 꿇을 수밖에 없다.",
    "【통곡】 너무도 부조리함에, 어린아이처럼 울음을 터트릴 수밖에 없다.",
    "【후회】 이럴 생각은 아니었는데. 하지만 현실은 비정했다.",
    "【공포】 공포에 사로잡혔다! 적이, 자신의 손이, 무서워서 참을 수 없다!",
    "【갈등】 정말로 이걸로 괜찮은 걸까? 몇 번이고 자신에게 의문이 일어난다…….",
    "【증오】 분노와 증오에 사로잡힌 당신은, 본래의 힘을 발휘할 수 있을까?",
    "【망연】 이것은 현실인가? 몽롱한 정신으로 당신은 생각한다.",
    "【주저】 망설임을 가졌다. 그것은 싸울 의지를 둔하게 할 것인가?",
    "【악몽】 이제부터 때때로, 당신은 이 순간을 악몽으로 볼 것이다.",
    "기적적으로 당신은 【낙인】을 받지 않았다.",
];
/// i18n `DetatokoSaga.table.WST`（기력 낙인표 / 2D6）。
static KO_WST: Table = Table::from_dice("기력 낙인표", 2, 6, KO_WST_ITEMS);

/// i18n `DetatokoSaga.table.SBET.items`。
static KO_SBET_ITEMS: &[&str] = &[
    "【사망】 당신은 죽었다. 다음 세션에 참가하기 위해서는, 클래스 1개를 『몬스터』나 『암흑』으로 클래스 체인지해야만 한다.",
    "【목숨 구걸】 당신은 공포를 느껴, 목숨을 구걸했다! 다음 세션 개시 시에, 클래스 1개가 『자코』로 변경된다!",
    "【망각】 당신은 기억을 잃고, 우두커니 섰다. 다음 세션에 참가하기 위해서는, 클래스 1개를 변경해야만 한다.",
    "【비극】 당신의 공격은 적이 아니라 아군을 맞췄다! 모든 것이 끝날 때까지 당신은 우두커니 서 있게 된다. 임의의 아군의 【체력】을 1D6점 감소시킨다.",
    "【폭주】 당신은 이성을 잃고, 충동에 따라 폭주한다! 같은 씬에 있는 전원의 【체력】을 1D6점 감소시킨다.",
    "【전락】 당신은 단애절벽에서 떨어진다.",
    "【포로】 당신은 적에게 사로잡혔다.",
    "【도주】 당신은 겁에 질려, 동료를 버리고 도망쳤다.",
    "【중상】 당신은 어찌할 수 없는 상처를 입고, 쓰러졌다.",
    "【기절】 당신은 의식을 잃었다. 그리고 정신이 들면 모든 것이 끝나있었다.",
    "그래도 아직 일어선다! 당신은 배드엔드를 맞이하지 않았다. 체력의 【낙인】을 1개 지워도 좋다.",
];
/// i18n `DetatokoSaga.table.SBET`（체력 배드엔딩표 / 2D6）。
static KO_SBET: Table = Table::from_dice("체력 배드엔딩표", 2, 6, KO_SBET_ITEMS);

/// i18n `DetatokoSaga.table.WBET.items`。
static KO_WBET_ITEMS: &[&str] = &[
    "【자해】 당신은 스스로 죽음을 골랐다. 다음 세션에 참가하기 위해서는 클래스 1개를 『암흑』으로 클래스 체인지해야만 한다.",
    "【타락】 당신은 마음속의 어둠에 먹혔다. 다음 세션 개시 시에, 클래스 1개가 『암흑』이나 『몬스터』로 변경된다!",
    "【예속】 당신은 적의 말에 거스를 수 없다. 다음 세션에 당신의 스탠스는 『종속』이 된다.",
    "【배반】 배반의 충동. 임의의 아군의 【체력】을 1D6점 감소시키고, 그 자리에서 도망친다.",
    "【폭주】 당신은 이성을 잃고, 충동에 따라 폭주한다! 같은 씬에 있는 전원의 【체력】을 1D6점 감소시킨다.",
    "【저주】 마음의 어둠이 현재화한 것인가. 적의 원한인가. 저주에 삼켜진 당신은, 그저 고통에 몸부림칠 수밖에 없다.",
    "【포로】 당신은 적에게 사로잡혀, 그 자리에서 끌려갔다.",
    "【도주】 당신은 겁에 질려, 동료를 버리고 도망쳤다.",
    "【방심】 당신은 그저 멍하니 서 있을 수밖에 없다. 정신을 차렸을 때, 모든 것은 끝나있었다.",
    "【기절】 당신은 의식을 잃었다. 그리고 정신이 들면 모든 것이 끝나있었다.",
    "그래도 아직 포기하지 않아! 당신은 배드엔드를 맞이하지 않았다. 기력의 【낙인】을 1개 지워도 좋다.",
];
/// i18n `DetatokoSaga.table.WBET`（기력 배드엔딩표 / 2D6）。
static KO_WBET: Table = Table::from_dice("기력 배드엔딩표", 2, 6, KO_WBET_ITEMS);

/// `ko_kr` ロケールの表と定型文一式。
static KO_SYSTEM: SystemTables = SystemTables {
    tables: &[
        ("SST", &KO_SST),
        ("WST", &KO_WST),
        ("SBET", &KO_SBET),
        ("WBET", &KO_WBET),
    ],
    ds_input_options: "판정！　스킬레벨：%{skill}　플래그：%{flag}　목표치：%{target}",
    ds_modifier: "　수정치：%{modifier}",
    ds_success: "목표치 이상！【성공】",
    ds_failure: "목표치 미달… 【실패】",
    jd_input_options: "판정！　스킬레벨：%{skill}　플래그：%{flag}",
    jd_modifier: "　수정치：%{modifier}",
    total_value: "판정치：%{total}",
    less_than_flag: ", 플래그 이하！ 【기력%{will}점 감소】 【판정치 변경 불가】",
    division_by_zero_error: "0으로는 나누어지지 않습니다",
};

/// Ruby `BCDice::GameSystem::DetatokoSaga_Korean`（ID: `DetatokoSaga:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetatokoSaga_Korean;

impl GameSystem for DetatokoSaga_Korean {
    fn id(&self) -> &'static str {
        "DetatokoSaga:Korean"
    }

    fn name(&self) -> &'static str {
        "데타토코 사가"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:데타토코 사가"
    }

    fn help_message(&self) -> &'static str {
        r"・통상판정　xDS or xDSy or xDS>=t or xDSy>=t or xDS+z>=t or xDSy+z>=t
　(x＝스킬레벨, y＝현재 플래그(생략=0), z＝수정치(생략=０), t＝목표치(생략=８))
　예）3DS　2DS5　0DS　3DS>=10　3DS7>=12 2DS3+1 3DS2+1>=10
・판정치　xJD or xJDy or xJDy+z or xJDy-z or xJDy/z
　(x＝스킬레벨, y＝현재 플래그(생략=0), z＝수정치(생략=０))
　예）3JD　2JD5　3JD7+1　4JD/3
・체력 낙인표　SST (StrengthStigmaTable)
・기력 낙인표　WST (WillStigmaTable)
・체력 배드엔딩표　SBET (StrengthBadEndTable)
・기력 배드엔딩표　WBET (WillBadEndTable)
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            r"\d+DS",
            r"\d+JD",
            "SST",
            "WST",
            "SBET",
            "WBET",
            "STRENGTHSTIGMATABLE",
            "WILLSTIGMATABLE",
            "STRENGTHBADENDTABLE",
            "WILLBADENDTABLE",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `DetatokoSaga#initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `DetatokoSaga#initialize` の `@d66_sort_type = D66SortType::ASC`。
    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }

    /// Ruby `Base#result_ndx`（`ko_kr` の定型文で）。
    ///
    /// Ruby側は `translate("success")` が `@locale`（このクラスでは `:ko_kr`）を見るため
    /// `성공` / `실패` になる。トレイトの既定実装は `ja_jp` 固定の
    /// `成功` / `失敗` を返すので、ここで上書きする。
    /// 接頭辞に一致しない加算ダイス判定がこの経路を通る。
    fn result_ndx(&self, total: crate::Int, cmp_op: CmpOp, target: Target) -> Option<EvalResult> {
        // Ruby: return nil if target.is_a?(String)（目標値 "?"）
        let Target::Number(target) = target else {
            return None;
        };
        if cmp_op.apply(&total, &target) {
            Some(EvalResult::success(KO_SUCCESS))
        } else {
            Some(EvalResult::failure(KO_FAILURE))
        }
    }

    /// Ruby `DetatokoSaga#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&KO_SYSTEM, command, rng)
    }
}

#[cfg(test)]
mod tests {

    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;

    /// `test/data/DetatokoSaga_Korean.toml` の全ケースが通ること（共通ハーネス）。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "DetatokoSaga:Korean",
            "DetatokoSaga_Korean.toml",
            42,
        );
    }

    /// 判定コマンドを通らない加算ダイス判定が `ko_kr` の定型文になること。
    ///
    /// Ruby は `Base#result_ndx` の `translate("success")` が `@locale = :ko_kr` を見るため
    /// `성공` / `실패` になる。TOMLにこの経路のケースが無いのでここで固定する。
    #[test]
    fn result_ndx_uses_ko_kr_wording() {
        let cases = [
            (
                "2D6>=7",
                vec![(4, 6), (5, 6)],
                "(2D6>=7) ＞ 9[4,5] ＞ 9 ＞ 성공",
            ),
            (
                "2D6>=10",
                vec![(4, 6), (5, 6)],
                "(2D6>=10) ＞ 9[4,5] ＞ 9 ＞ 실패",
            ),
        ];
        for (input, rands, expected) in cases {
            let mut src = SeededRandomizer::new(rands);
            let result = eval_command(&GameSystemId::new("DetatokoSaga:Korean"), input, &mut src)
                .expect("eval")
                .expect("result");
            assert_eq!(result.text, expected, "input {input:?}");
            assert!(src.is_empty(), "unconsumed rands for {input:?}");
        }
    }
}
