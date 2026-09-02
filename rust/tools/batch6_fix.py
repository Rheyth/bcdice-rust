import pathlib, re

base = pathlib.Path("src/game_system/generated")

# For the 3 helper-def files and any mixed files in this batch, only replace the
# all_toml_cases_pass test (and remove the local helper fn), PRESERVING other tests.
# Files at risk: SwordWorld2_5.rs (extra tests), Dracurouge.rs (extra tests).
# The other files replaced wholesale only had the harness (verify below).

def strip_local_helper(t):
    """Remove a local `#[cfg(test)]\npub(crate) fn assert_toml_cases...}` block."""
    m = re.search(r"#\[cfg\(test\)\]\npub\(crate\) fn assert_toml_cases\(", t)
    if not m:
        return t, False
    start = m.start()
    # find end of fn by brace matching from first '{' after signature
    brace = t.index("{", m.end())
    depth = 0
    for i in range(brace, len(t)):
        if t[i] == "{":
            depth += 1
        elif t[i] == "}":
            depth -= 1
            if depth == 0:
                end = i + 1
                break
    # also swallow trailing newlines
    while end < len(t) and t[end] == "\n":
        end += 1
    return t[:start] + t[end:], True

def replace_only_harness_test(t, rel):
    """Replace the single all_toml_cases_pass test body; keep other tests intact."""
    # find the test fn
    m = re.search(r"( *)#\[test\]\n *fn all_toml_cases_pass\(\) \{.*?\n    \}\n", t, re.S)
    if not m:
        return None
    return m

# We'll regenerate per-file from git HEAD content.
import subprocess
for rel in ["SwordWorld2_5.rs", "Dracurouge.rs"]:
    head = subprocess.run(["git", "show", f"HEAD:rust/src/game_system/generated/{rel}"],
                          capture_output=True, text=True, cwd="..").stdout
    t, removed = strip_local_helper(head)
    print(rel, "helper removed:", removed)
    # now replace the harness test with a call to test_support
    if rel == "SwordWorld2_5.rs":
        newtest = '''    /// `test/data/SwordWorld2_5.toml` の全ケースが通ること（共通ハーネス）。
    ///
    /// ケース 107（無効コマンド `green` の暴発確認fixture）は出目が消費されない
    /// 既知のTOML不整合。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases(
            "SwordWorld2.5",
            "SwordWorld2_5.toml",
            144,
            &[(107, 2)],
        );
    }
'''
    else:
        newtest = '''    /// `test/data/Dracurouge.toml` の全ケースが通ること（共通ハーネス）。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "Dracurouge",
            "Dracurouge.toml",
            147,
        );
    }
'''
    m = re.search(r"    #\[test\]\n    fn all_toml_cases_pass\(\) \{.*?\n    \}\n", t, re.S)
    assert m, rel
    t = t[:m.start()] + newtest + t[m.end():]
    (base / rel).write_text(t)
    print("rewrote preserving extra tests:", rel)
