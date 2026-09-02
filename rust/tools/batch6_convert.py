import pathlib, re

base = pathlib.Path("src/game_system/generated")

# --- 1. Remove local helper defs, replace with thin test mod calling test_support ---
# SwordWorld.rs: helper at 466..539 approx; uses count 230, no surplus; SW_SC calls with no count.
# Replacement: test mod with assert_toml_cases_strict("SwordWorld","SwordWorld.toml",230)
# But SwordWorld_SimplifiedChinese calls SwordWorld::assert_toml_cases(system,file) with no count.
# -> SC variant gets its own count: SwordWorld_SimplifiedChinese.toml = 230 cases too.

# SwordWorld2_0.rs: helper w/ count arg + green fixture allowance (matches system 2.0/2.5)
# Callers pass count. Replace helper with test_support call + surplus &[(60,2)] for 2.0;
# for 2.5 green is case 107 -> surplus &[(107,2)]; SC variants have no green case.
# Callers:
#   SwordWorld2_0.rs:      ("SwordWorld2.0","SwordWorld2_0.toml",75)      + surplus [(60,2)]
#   SwordWorld2_5.rs:      ("SwordWorld2.5","SwordWorld2_5.toml",144)     + surplus [(107,2)]
#   SwordWorld2_0_SC.rs:   ("SwordWorld2.0:SimplifiedChinese",...,"..._toml",73)  no surplus
#   SwordWorld2_5_SC.rs:   ("SwordWorld2.5:SimplifiedChinese",...,92)     no surplus
#   Chill3.rs: 17, Elric.rs: 12, Dracurouge.rs 147, Dracurouge_Korean 147 -> strict
#   (nil cases in Dracurouge have rands=[] so remaining 0 -> strict fine)
# TokyoNova.rs helper: strict-style; callers: FutariSousa 172, FS_Korean 144,
#   MagicaLogia 155, ML_Korean 155, ML_SC 155, WARPS 31, WaresBlade 13 -> strict
# SwordWorld.rs helper: count 230 hardcoded; self + SC -> strict with 230.

def rewrite_testmod(rel, newmod):
    p = base / rel
    t = p.read_text()
    idx = t.find("#[cfg(test)]")
    assert idx > 0, rel
    p.write_text(t[:idx] + newmod)
    print("rewrote", rel)

def argsmod(sysid, toml, count, surplus=None, doc=None):
    call = f'''crate::game_system::test_support::assert_toml_cases(
            "{sysid}",
            "{toml}",
            {count},
            {surplus},
        );''' if surplus else f'''crate::game_system::test_support::assert_toml_cases_strict(
            "{sysid}",
            "{toml}",
            {count},
        );'''
    d = doc or f'''/// `test/data/{toml}` の全ケースが通ること（共通ハーネス）。'''
    return f'''#[cfg(test)]
mod tests {{
    {d}
    #[test]
    fn all_toml_cases_pass() {{
        {call}
    }}
}}
'''

# 3 helper-def files
rewrite_testmod("SwordWorld.rs", argsmod("SwordWorld", "SwordWorld.toml", 230))
rewrite_testmod("SwordWorld2_0.rs", argsmod(
    "SwordWorld2.0", "SwordWorld2_0.toml", 75, "&[(60, 2)]",
    doc='''/// `test/data/SwordWorld2_0.toml` の全ケースが通ること（共通ハーネス）。
    ///
    /// ケース 60（無効コマンド `green` の暴発確認fixture）は出目が消費されない
    /// 既知のTOML不整合。'''))
rewrite_testmod("TokyoNova.rs", argsmod("TokyoNova", "TokyoNova.toml", 8))

# 15 caller files
rewrite_testmod("Chill3.rs", argsmod("Chill3", "Chill3.toml", 17))
rewrite_testmod("Dracurouge.rs", argsmod("Dracurouge", "Dracurouge.toml", 147))
rewrite_testmod("Dracurouge_Korean.rs", argsmod("Dracurouge:Korean", "Dracurouge_Korean.toml", 147))
rewrite_testmod("Elric.rs", argsmod("Elric", "Elric.toml", 12))
rewrite_testmod("SwordWorld_SimplifiedChinese.rs", argsmod("SwordWorld:SimplifiedChinese", "SwordWorld_SimplifiedChinese.toml", 230))
rewrite_testmod("SwordWorld2_0_SimplifiedChinese.rs", argsmod("SwordWorld2.0:SimplifiedChinese", "SwordWorld2_0_SimplifiedChinese.toml", 73))
rewrite_testmod("SwordWorld2_5.rs", argsmod(
    "SwordWorld2.5", "SwordWorld2_5.toml", 144, "&[(107, 2)]",
    doc='''/// `test/data/SwordWorld2_5.toml` の全ケースが通ること（共通ハーネス）。
    ///
    /// ケース 107（無効コマンド `green` の暴発確認fixture）は出目が消費されない
    /// 既知のTOML不整合。'''))
rewrite_testmod("SwordWorld2_5_SimplifiedChinese.rs", argsmod("SwordWorld2.5:SimplifiedChinese", "SwordWorld2_5_SimplifiedChinese.toml", 92))
rewrite_testmod("FutariSousa.rs", argsmod("FutariSousa", "FutariSousa.toml", 172))
rewrite_testmod("FutariSousa_Korean.rs", argsmod("FutariSousa:Korean", "FutariSousa_Korean.toml", 144))
rewrite_testmod("MagicaLogia.rs", argsmod("MagicaLogia", "MagicaLogia.toml", 155))
rewrite_testmod("MagicaLogia_Korean.rs", argsmod("MagicaLogia:Korean", "MagicaLogia_Korean.toml", 155))
rewrite_testmod("MagicaLogia_SimplifiedChinese.rs", argsmod("MagicaLogia:SimplifiedChinese", "MagicaLogia_SimplifiedChinese.toml", 155))
rewrite_testmod("WARPS.rs", argsmod("WARPS", "WARPS.toml", 31))
rewrite_testmod("WaresBlade.rs", argsmod("WaresBlade", "WaresBlade.toml", 13))
