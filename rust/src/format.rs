use num_traits::Zero;

use crate::normalize::CmpOp;

/// 比較演算子を文字列表記にする。Ruby `Format.comparison_operator`。
///
/// Ruby側は `nil` を返しうるが、返り値は必ず文字列補間に渡されるため
/// （`"#{nil}" == ""`）、ここでは `None` を空文字列に畳んで返す。
pub fn comparison_operator(op: Option<CmpOp>) -> &'static str {
    match op {
        Some(CmpOp::Eq) => "=",
        Some(CmpOp::Ne) => "<>",
        Some(other) => other.symbol_str(),
        None => "",
    }
}

/// 修正値を文字列表記にする。Ruby `Format.modifier`。
///
/// Ruby側の `number.nil? -> nil` 分岐は、P1で使う呼び出し元
/// （`Command::Parsed#to_s` / `UpperDice`）ではいずれも整数が確定しているため
/// 到達しない。到達しうる呼び出しがP4で現れたら `Option` 版を足すこと。
pub fn modifier(number: &crate::Int) -> String {
    if number.is_zero() {
        String::new()
    } else if number > &crate::Int::ZERO {
        format!("+{number}")
    } else {
        number.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{comparison_operator, modifier};
    use crate::normalize::CmpOp;

    #[test]
    fn formats_comparison_operator() {
        assert_eq!(comparison_operator(Some(CmpOp::Eq)), "=");
        assert_eq!(comparison_operator(Some(CmpOp::Ne)), "<>");
        assert_eq!(comparison_operator(Some(CmpOp::Ge)), ">=");
        assert_eq!(comparison_operator(None), "");
    }

    #[test]
    fn formats_modifier() {
        assert_eq!(modifier(&0.into()), "");
        assert_eq!(modifier(&3.into()), "+3");
        assert_eq!(modifier(&(-3).into()), "-3");
    }
}
