//! チョイスコマンド。Ruby `lib/bcdice/common_command/choice.rb` の移植。
//!
//! 共通Lexerを使わず `StringScanner` を直接舐めるコマンドなので、
//! ここでも小さなスキャナを自前で用意して `scan` / `scan_until` / `post_match` の
//! 意味論を再現する。

use crate::eval::{EvalError, EvalResult};
use crate::randomizer::Randomizer;

/// 項目の区切り方。Ruby `Choice::BlockDelimiter`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockDelimiter {
    /// `[` 始まり。`,` 区切り、`]` 終端
    Bracket,
    /// `(` 始まり。`,` 区切り、`)` 終端
    Paren,
    /// 空白始まり。`/\s+/` 区切り、行末終端
    Space,
}

impl BlockDelimiter {
    /// Ruby `DELIMITER_CHAR`（選択結果を連結する文字列）。
    fn delimiter_char(self) -> &'static str {
        match self {
            BlockDelimiter::Bracket | BlockDelimiter::Paren => ", ",
            BlockDelimiter::Space => " ",
        }
    }
}

/// Ruby `Choice.eval(command, _game_system, randomizer)`。
pub fn eval(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    match parse(command) {
        Some(cmd) => cmd.roll(rng).map(Some),
        None => Ok(None),
    }
}

/// Ruby `Choice` インスタンス。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    secret: bool,
    block_delimiter: BlockDelimiter,
    takes: usize,
    items: Vec<String>,
}

impl Choice {
    /// Ruby `#roll(randomizer)`。
    pub fn roll(&self, rng: &mut Randomizer) -> Result<EvalResult, EvalError> {
        if self.items.len() > 100 {
            // Ruby: Result.new(text) なので secret は設定されない
            return Ok(EvalResult::with_text("項目数は100以下としてください"));
        }

        let mut items = self.items.clone();
        let mut chosens = Vec::with_capacity(self.takes);
        for _ in 0..self.takes {
            let index = rng.roll_index(items.len() as i64)?;
            // Ruby: Array#delete_at は範囲外で nil（chosens に nil が入る）
            if index >= 0 && (index as usize) < items.len() {
                chosens.push(items.remove(index as usize));
            }
        }

        Ok(EvalResult {
            text: format!(
                "({}) ＞ {}",
                self.expr(),
                chosens.join(self.block_delimiter.delimiter_char())
            ),
            secret: self.secret,
            ..EvalResult::default()
        })
    }

    /// Ruby `#expr`。
    pub fn expr(&self) -> String {
        let takes = if self.takes == 1 {
            String::new()
        } else {
            self.takes.to_string()
        };
        match self.block_delimiter {
            BlockDelimiter::Space => format!("choice{takes} {}", self.items.join(" ")),
            BlockDelimiter::Bracket => format!("choice{takes}[{}]", self.items.join(",")),
            BlockDelimiter::Paren => format!("choice{takes}({})", self.items.join(",")),
        }
    }
}

/// Ruby `StringScanner` の最小再現。
struct Scanner<'a> {
    text: &'a str,
    pos: usize,
}

impl<'a> Scanner<'a> {
    fn new(text: &'a str) -> Self {
        Self { text, pos: 0 }
    }

    fn rest(&self) -> &'a str {
        &self.text[self.pos..]
    }

    /// Ruby `#skip(/\s+/)` 相当。
    fn skip_whitespace(&mut self) {
        let rest = self.rest();
        let n = rest.len() - rest.trim_start_matches(is_ruby_space).len();
        self.pos += n;
    }

    /// 先頭が `c`（大文字小文字無視）なら1文字進める。Ruby `#scan(/S/i)` 相当。
    fn scan_char_ci(&mut self, c: char) -> bool {
        match self.rest().chars().next() {
            Some(first) if first.eq_ignore_ascii_case(&c) => {
                self.pos += first.len_utf8();
                true
            }
            _ => false,
        }
    }

    /// 先頭がリテラル `lit`（大文字小文字無視）なら進める。
    fn scan_literal_ci(&mut self, lit: &str) -> bool {
        let rest = self.rest();
        if rest.len() >= lit.len()
            && rest.is_char_boundary(lit.len())
            && rest[..lit.len()].eq_ignore_ascii_case(lit)
        {
            self.pos += lit.len();
            true
        } else {
            false
        }
    }

    /// Ruby `#scan(/\d+/)`。
    fn scan_digits(&mut self) -> Option<&'a str> {
        let rest = self.rest();
        let end = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        if end == 0 {
            return None;
        }
        self.pos += end;
        Some(&rest[..end])
    }

    /// Ruby `#scan(/\(|\[|\s+/)`。マッチ文字列を返す。
    fn scan_block_open(&mut self) -> Option<&'a str> {
        let rest = self.rest();
        let first = rest.chars().next()?;
        if first == '(' || first == '[' {
            self.pos += first.len_utf8();
            return Some(&rest[..first.len_utf8()]);
        }
        if is_ruby_space(first) {
            let end = rest.len() - rest.trim_start_matches(is_ruby_space).len();
            self.pos += end;
            return Some(&rest[..end]);
        }
        None
    }

    /// Ruby `#scan_until(/,/)`。マッチ末尾までの文字列を返して進める。
    fn scan_until_comma(&mut self) -> Option<&'a str> {
        let rest = self.rest();
        let idx = rest.find(',')?;
        let end = idx + 1;
        self.pos += end;
        Some(&rest[..end])
    }

    /// Ruby `#scan_until(/\s+/)`。
    fn scan_until_whitespace(&mut self) -> Option<&'a str> {
        let rest = self.rest();
        let start = rest.find(is_ruby_space)?;
        let after = &rest[start..];
        let ws_len = after.len() - after.trim_start_matches(is_ruby_space).len();
        let end = start + ws_len;
        self.pos += end;
        Some(&rest[..end])
    }

    /// Ruby `#scan_until(/\]/)` / `#scan_until(/\)/)`。
    fn scan_until_char(&mut self, c: char) -> Option<&'a str> {
        let rest = self.rest();
        let idx = rest.find(c)?;
        let end = idx + c.len_utf8();
        self.pos += end;
        Some(&rest[..end])
    }

    /// Ruby `#scan_until(/$/)`。
    ///
    /// Rubyの `$` は行末アンカーなので、最初の改行の直前（なければ文末）まで。
    /// 空文字列にマッチしうるので `Some("")` を返すことがある（`nil` とは別物）。
    fn scan_until_line_end(&mut self) -> Option<&'a str> {
        let rest = self.rest();
        let end = rest.find('\n').unwrap_or(rest.len());
        self.pos += end;
        Some(&rest[..end])
    }
}

/// Rubyの `\s`（`[ \t\r\n\f\v]`）。
fn is_ruby_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n' | '\u{c}' | '\u{b}')
}

/// Ruby `String#strip`。
fn ruby_strip(s: &str) -> &str {
    s.trim_matches(|c: char| matches!(c, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | '\0'))
}

/// Ruby `Choice.parse(command)`。
pub fn parse(command: &str) -> Option<Choice> {
    let mut scanner = Scanner::new(command);
    scanner.skip_whitespace();

    let secret = scanner.scan_char_ci('S');
    if !scanner.scan_literal_ci("choice") {
        return None;
    }

    let takes: usize = match scanner.scan_digits() {
        Some(d) => d.parse::<usize>().unwrap_or(usize::MAX),
        None => 1,
    };
    if takes == 0 {
        return None;
    }

    let block_delimiter = match scanner.scan_block_open() {
        Some("[") => BlockDelimiter::Bracket,
        Some("(") => BlockDelimiter::Paren,
        Some(_) => BlockDelimiter::Space,
        None => return None,
    };

    let mut items: Vec<String> = Vec::new();
    loop {
        let item = match block_delimiter {
            BlockDelimiter::Bracket | BlockDelimiter::Paren => scanner.scan_until_comma(),
            BlockDelimiter::Space => scanner.scan_until_whitespace(),
        };
        match item {
            // Ruby: item.delete_suffix(",")
            Some(item) => items.push(item.strip_suffix(',').unwrap_or(item).to_string()),
            None => break,
        }
    }

    let last_item = match block_delimiter {
        BlockDelimiter::Bracket => scanner.scan_until_char(']'),
        BlockDelimiter::Paren => scanner.scan_until_char(')'),
        BlockDelimiter::Space => scanner.scan_until_line_end(),
    }?;

    // Ruby: last_item.delete_suffix(SUFFIX[type])（SPACEは "" なので変化なし）
    let last_item = match block_delimiter {
        BlockDelimiter::Bracket => last_item.strip_suffix(']').unwrap_or(last_item),
        BlockDelimiter::Paren => last_item.strip_suffix(')').unwrap_or(last_item),
        BlockDelimiter::Space => last_item,
    };
    items.push(last_item.to_string());

    let mut items: Vec<String> = items
        .iter()
        .map(|s| ruby_strip(s).to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if items.len() == 1 {
        items = parse_multi_item_shorthand(&items[0]);
    }

    if items.is_empty() || items.len() < takes {
        return None;
    }

    Some(Choice {
        secret,
        block_delimiter,
        takes,
        items,
    })
}

/// Ruby `#parse_multi_item_shorthand`。
fn parse_multi_item_shorthand(s: &str) -> Vec<String> {
    parse_multi_nums_shorthand(s)
        .or_else(|| parse_multi_chars_shorthand(s))
        .unwrap_or_default()
}

/// Ruby `#parse_multi_nums_shorthand`（`/^(\d+)-(\d+)$/`）。
fn parse_multi_nums_shorthand(s: &str) -> Option<Vec<String>> {
    let (a, b) = s.split_once('-')?;
    if a.is_empty()
        || b.is_empty()
        || !a.bytes().all(|c| c.is_ascii_digit())
        || !b.bytes().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let first: i64 = a.parse().ok()?;
    let last: i64 = b.parse().ok()?;
    if first > last {
        return None;
    }
    Some((first..=last).map(|n| n.to_string()).collect())
}

/// Ruby `#parse_multi_chars_shorthand`
/// （`/^([a-z])-([a-z])$/` または `/^([A-Z])-([A-Z])$/`）。
fn parse_multi_chars_shorthand(s: &str) -> Option<Vec<String>> {
    let bytes = s.as_bytes();
    if bytes.len() != 3 || bytes[1] != b'-' {
        return None;
    }
    let (first, last) = (bytes[0], bytes[2]);
    let same_case = (first.is_ascii_lowercase() && last.is_ascii_lowercase())
        || (first.is_ascii_uppercase() && last.is_ascii_uppercase());
    if !same_case || first > last {
        return None;
    }
    Some((first..=last).map(|c| (c as char).to_string()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(src: &str) -> Option<Vec<String>> {
        parse(src).map(|c| c.items)
    }

    #[test]
    fn parses_bracket_items() {
        assert_eq!(
            items("choice[abc,def]"),
            Some(vec!["abc".to_string(), "def".to_string()])
        );
        assert_eq!(
            items("choice[A(), B(), C()] カッコが終端として認識されない"),
            Some(vec!["A()".into(), "B()".into(), "C()".into()])
        );
    }

    #[test]
    fn parses_space_items() {
        assert_eq!(
            items("choice The Call of Cthulhu"),
            Some(vec![
                "The".into(),
                "Call".into(),
                "of".into(),
                "Cthulhu".into()
            ])
        );
        assert_eq!(
            items("choice A,B P,J Z,Y"),
            Some(vec!["A,B".into(), "P,J".into(), "Z,Y".into()])
        );
    }

    #[test]
    fn expands_shorthand_only_for_single_item() {
        assert_eq!(
            items("choice[A-D]"),
            Some(vec!["A".into(), "B".into(), "C".into(), "D".into()])
        );
        assert_eq!(
            items("choice[3-5]"),
            Some(vec!["3".into(), "4".into(), "5".into()])
        );
        assert_eq!(
            items("choice[A-F, Z] こういうケースでは展開しない"),
            Some(vec!["A-F".into(), "Z".into()])
        );
        assert_eq!(items("choice[F-A] 大小関係が逆"), None);
        assert_eq!(items("choice[a-zz] 複数文字では省略にならない"), None);
    }

    #[test]
    fn rejects_broken_syntax() {
        assert!(parse("choice[A,B,C,D) 終端記号が違う").is_none());
        assert!(parse("choice{A,B,C,D} 不正な範囲開始文字").is_none());
        assert!(parse("choice[] 要素数ゼロ").is_none());
        assert!(parse("choice0[abc,def]").is_none());
        assert!(parse("choice3[abc,def]").is_none());
    }
}
