import pathlib, re

base = pathlib.Path("src/game_system/generated")
# Check semantics of local helpers vs common test_support:
# 1. TokyoNova helper: nil Ok(None) allowed silently; unconsumed rands -> failure ALWAYS (no surplus)
# 2. SwordWorld2_0 helper: known 'green' fixture allowed surplus
# 3. SwordWorld helper: same as TokyoNova but count hardcoded 230
# 4. test_support: surplus_rands_allowed list; also nil not special-cased for rands
# For callers: nil cases with rands=[] consume nothing => remaining 0 == ok. Good.
# BladeOfArcana nil cases each have 1 rand unconsumed => surplus allowed (24..34, 1)
# BlackJacket nil cases: (2,2),(41,1); BUT old harness requires nil path consumes 0 rands
#   (remaining == rands.len()). Common helper with surplus (2,2) allows remaining<=2 but
#   also would allow remaining 1 (dice rolled but some left) - weaker but only matters if
#   implementation changed. Equivalent enough? Old: remaining == 2. New: remaining == 2 allowed.
#   Actually helper requires remaining == allowed_surplus exactly. So (2,2) => remaining must be 2 = same.
# GURPS: no nil cases; unconsumed always failure; remaining must be 0. strict works.

# Emit replacement test mods for the batch5 files.
repl = {
"BladeOfArcana.rs": '''#[cfg(test)]
mod tests {
    /// `test/data/BladeOfArcana.toml` の全ケースが通ること（共通ハーネス）。
    /// ケース末尾のnil 11件は無効コマンドで、注入済み出目1つが余る。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases(
            "BladeOfArcana",
            "BladeOfArcana.toml",
            34,
            &[
                (24, 1),
                (25, 1),
                (26, 1),
                (27, 1),
                (28, 1),
                (29, 1),
                (30, 1),
                (31, 1),
                (32, 1),
                (33, 1),
                (34, 1),
            ],
        );
    }
}
''',
"GURPS.rs": '''#[cfg(test)]
mod tests {
    /// `test/data/GURPS.toml` の全ケースが通ること（共通ハーネス）。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("GURPS", "GURPS.toml", 34);
    }
}
''',
"BlackJacket.rs": '''#[cfg(test)]
mod tests {
    /// `test/data/BlackJacket.toml` の全ケースが通ること（共通ハーネス）。
    ///
    /// nil を返すケース（2, 41）の `rands` は上流のTOMLに残った書き換え漏れで、
    /// 出目のオラクルにならない。nil経路ではダイスを消費しないため全量が余る。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases(
            "BlackJacket",
            "BlackJacket.toml",
            89,
            &[(2, 2), (41, 1)],
        );
    }
}
''',
"BlackJacket_Korean.rs": '''#[cfg(test)]
mod tests {
    /// `test/data/BlackJacket_Korean.toml` の全ケースが通ること（共通ハーネス）。
    ///
    /// nil を返すケース（2, 41）の `rands` は上流のTOMLに残った書き換え漏れで、
    /// 出目のオラクルにならない。nil経路ではダイスを消費しないため全量が余る。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases(
            "BlackJacket:Korean",
            "BlackJacket_Korean.toml",
            89,
            &[(2, 2), (41, 1)],
        );
    }
}
''',
}
for rel, newmod in repl.items():
    p = base / rel
    t = p.read_text()
    idx = t.find("#[cfg(test)]")
    p.write_text(t[:idx] + newmod)
    print("rewrote", rel)
