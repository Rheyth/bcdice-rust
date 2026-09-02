//! Ruby `StringScanner` の最小再現。Ruby `lib/bcdice/common_command/choice.rb` が
//! `scan` / `scan_until` / `post_match` で使う操作を `choice` と `repeat` で共有する。
//!
//! `post_match`（直前のマッチ以降の文字列）は実装しない。両呼び出し側とも
//! 「現在位置以降の残り」で目的を達できるため、`rest()` を代わりに使う契約とする。

/// Rubyの `\s`（`[ \t\r\n\f\v]`）。
pub(crate) fn is_ruby_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n' | '\u{c}' | '\u{b}')
}

/// Ruby `String#strip`。
///
/// Rubyの strip は `\0` も落とす（`/ \s/` より広い）。[`RUBY_WHITESPACE`]（6文字版）
/// とは別物として区別すること。
///
/// [`RUBY_WHITESPACE`]: crate::common_command::lexer::RUBY_WHITESPACE
pub(crate) fn ruby_strip(s: &str) -> &str {
    s.trim_matches(|c: char| matches!(c, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | '\0'))
}

/// Ruby `StringScanner` の最小再現。
pub(crate) struct Scanner<'a> {
    text: &'a str,
    pos: usize,
}

impl<'a> Scanner<'a> {
    pub(crate) fn new(text: &'a str) -> Self {
        Self { text, pos: 0 }
    }

    pub(crate) fn rest(&self) -> &'a str {
        &self.text[self.pos..]
    }

    /// Ruby `#skip(/\s+/)` 相当。
    pub(crate) fn skip_whitespace(&mut self) {
        let rest = self.rest();
        let n = rest.len() - rest.trim_start_matches(is_ruby_space).len();
        self.pos += n;
    }

    /// 先頭が `c`（大文字小文字無視）なら1文字進める。Ruby `#scan(/S/i)` 相当。
    pub(crate) fn scan_char_ci(&mut self, c: char) -> bool {
        match self.rest().chars().next() {
            Some(first) if first.eq_ignore_ascii_case(&c) => {
                self.pos += first.len_utf8();
                true
            }
            _ => false,
        }
    }

    /// 先頭がリテラル `lit`（大文字小文字無視）なら進める。Ruby `#scan(/(?:lit)/i)` 相当。
    pub(crate) fn scan_literal_ci(&mut self, lit: &str) -> bool {
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
    pub(crate) fn scan_digits(&mut self) -> Option<&'a str> {
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
    pub(crate) fn scan_block_open(&mut self) -> Option<&'a str> {
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
    pub(crate) fn scan_until_comma(&mut self) -> Option<&'a str> {
        let rest = self.rest();
        let idx = rest.find(',')?;
        let end = idx + 1;
        self.pos += end;
        Some(&rest[..end])
    }

    /// Ruby `#scan_until(/\s+/)`。
    pub(crate) fn scan_until_whitespace(&mut self) -> Option<&'a str> {
        let rest = self.rest();
        let start = rest.find(is_ruby_space)?;
        let after = &rest[start..];
        let ws_len = after.len() - after.trim_start_matches(is_ruby_space).len();
        let end = start + ws_len;
        self.pos += end;
        Some(&rest[..end])
    }

    /// Ruby `#scan_until(/\]/)` / `#scan_until(/\)/)`。
    pub(crate) fn scan_until_char(&mut self, c: char) -> Option<&'a str> {
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
    pub(crate) fn scan_until_line_end(&mut self) -> Option<&'a str> {
        let rest = self.rest();
        let end = rest.find('\n').unwrap_or(rest.len());
        self.pos += end;
        Some(&rest[..end])
    }
}
