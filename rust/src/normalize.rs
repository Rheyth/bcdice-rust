//! 入力の正規化。Ruby `lib/bcdice/normalize.rb` の移植。

/// 比較演算子。Ruby側はシンボル（`:<=` など）で持ち回る。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Le,
    Ge,
    Ne,
    Lt,
    Gt,
    Eq,
}

impl CmpOp {
    /// 比較を実行する。Ruby `total.send(cmp_op, target)` 相当。
    pub fn apply(self, lhs: &crate::Int, rhs: &crate::Int) -> bool {
        match self {
            CmpOp::Le => lhs <= rhs,
            CmpOp::Ge => lhs >= rhs,
            CmpOp::Ne => lhs != rhs,
            CmpOp::Lt => lhs < rhs,
            CmpOp::Gt => lhs > rhs,
            CmpOp::Eq => lhs == rhs,
        }
    }

    /// Rubyシンボルの `to_s` 相当。`Format::comparison_operator` の
    /// 「Symbolならそのまま」分岐で使う表記。
    pub fn symbol_str(self) -> &'static str {
        match self {
            CmpOp::Le => "<=",
            CmpOp::Ge => ">=",
            CmpOp::Ne => "!=",
            CmpOp::Lt => "<",
            CmpOp::Gt => ">",
            CmpOp::Eq => "==",
        }
    }
}

/// 比較演算子をシンボルに正規化する。Ruby `Normalize.comparison_operator`。
///
/// Ruby側は `case op when /<=|=</ ... end` と**部分一致**の正規表現で判定しており、
/// 判定順序がそのまま優先順位になる（例: `"<=>"` は最初の `/<=|=</` に当たって `:<=`）。
/// ここでは同じ順序の部分一致で再現する。
pub fn comparison_operator(op: &str) -> Option<CmpOp> {
    if op.contains("<=") || op.contains("=<") {
        Some(CmpOp::Le)
    } else if op.contains(">=") || op.contains("=>") {
        Some(CmpOp::Ge)
    } else if op.contains("<>") || op.contains("!=") || op.contains("=!") {
        Some(CmpOp::Ne)
    } else if op.contains('<') {
        Some(CmpOp::Lt)
    } else if op.contains('>') {
        Some(CmpOp::Gt)
    } else if op.contains('=') {
        Some(CmpOp::Eq)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_comparison_operators() {
        assert_eq!(comparison_operator("<="), Some(CmpOp::Le));
        assert_eq!(comparison_operator("=<"), Some(CmpOp::Le));
        assert_eq!(comparison_operator(">="), Some(CmpOp::Ge));
        assert_eq!(comparison_operator("=>"), Some(CmpOp::Ge));
        assert_eq!(comparison_operator("<>"), Some(CmpOp::Ne));
        assert_eq!(comparison_operator("!="), Some(CmpOp::Ne));
        assert_eq!(comparison_operator("=!"), Some(CmpOp::Ne));
        assert_eq!(comparison_operator("<"), Some(CmpOp::Lt));
        assert_eq!(comparison_operator(">"), Some(CmpOp::Gt));
        assert_eq!(comparison_operator("="), Some(CmpOp::Eq));
        assert_eq!(comparison_operator("=="), Some(CmpOp::Eq));
        // Ruby側の部分一致順序の再現確認
        assert_eq!(comparison_operator("<=>"), Some(CmpOp::Le));
        assert_eq!(comparison_operator("!"), None);
    }
}
