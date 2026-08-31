//! 表を振った結果。Ruby `BCDice::DiceTable::RollResult`（lib/bcdice/dice_table/roll_result.rb）。

/// 表を振った結果。`to_s` は `"表名(値) ＞ 内容"`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollResult {
    table_name: &'static str,
    value: i64,
    body: RollBody,
}

/// [`RollResult`] の本体。Ruby では `String` か `RollResult`（ネストした表の結果）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RollBody {
    /// 文字列。該当項目がない場合（Ruby の `nil`）は空文字列で表す。
    Text(&'static str),
    /// ネストした表の結果。
    Nested(Box<RollResult>),
}

impl std::fmt::Display for RollBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RollBody::Text(t) => f.write_str(t),
            RollBody::Nested(r) => write!(f, "{r}"),
        }
    }
}

impl RollResult {
    /// Ruby `RollResult.new(table_name, value, body)`。
    pub fn new(table_name: &'static str, value: i64, body: RollBody) -> Self {
        Self {
            table_name,
            value,
            body,
        }
    }

    /// 文字列本体の結果を作る。
    pub fn text(table_name: &'static str, value: i64, body: &'static str) -> Self {
        Self::new(table_name, value, RollBody::Text(body))
    }

    /// Ruby `#table_name`。
    pub fn table_name(&self) -> &'static str {
        self.table_name
    }

    /// Ruby `#value`。
    pub fn value(&self) -> i64 {
        self.value
    }

    /// Ruby `#body`。
    pub fn body(&self) -> &RollBody {
        &self.body
    }

    /// Ruby `#last_body`。ネストの一番内側の文字列を返す。
    pub fn last_body(&self) -> &'static str {
        match &self.body {
            RollBody::Text(t) => t,
            RollBody::Nested(r) => r.last_body(),
        }
    }
}

impl std::fmt::Display for RollResult {
    /// Ruby `#to_s`: `"#{@table_name}(#{@value}) ＞ #{@body}"`。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({}) ＞ {}", self.table_name, self.value, self.body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_like_ruby_to_s() {
        let r = RollResult::text("致命的命中表", 7, "致命的命中はなかった");
        assert_eq!(r.to_string(), "致命的命中表(7) ＞ 致命的命中はなかった");
        assert_eq!(r.last_body(), "致命的命中はなかった");
    }

    #[test]
    fn nested_result_is_flattened_by_last_body() {
        let inner = RollResult::text("内側表", 3, "内側の内容");
        let outer = RollResult::new("外側表", 5, RollBody::Nested(Box::new(inner)));
        assert_eq!(outer.to_string(), "外側表(5) ＞ 内側表(3) ＞ 内側の内容");
        assert_eq!(outer.last_body(), "内側の内容");
    }

    #[test]
    fn missing_item_renders_as_empty_body() {
        // Ruby: @items[index] が nil のとき "表名(値) ＞ " になる
        let r = RollResult::text("表", 1, "");
        assert_eq!(r.to_string(), "表(1) ＞ ");
    }
}
