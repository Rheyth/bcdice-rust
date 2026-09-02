import pathlib, re, subprocess

base = pathlib.Path("src/game_system/generated")
# verify: test-mod line count at baseline vs now, and remaining test mods are helper calls or unique tests
r = subprocess.run(["git", "show", "4322a70b:rust/src/game_system/generated/ChaosFlare.rs"],
                   capture_output=True, text=True, cwd="..")
baseline_sample = r.stdout
i = baseline_sample.find("#[cfg(test)]")
print("baseline ChaosFlare test lines:", baseline_sample[i:].count("\n") if i>=0 else 0)

# remaining test mods classification
mods = {"helper_only": [], "unique_tests": [], "empty": []}
for p in sorted(base.glob("*.rs")):
    t = p.read_text()
    i = t.find("#[cfg(test)]")
    if i < 0:
        mods["empty"].append(p.name)
        continue
    tail = t[i:]
    n = tail.count("#[test]")
    if n == 0:
        mods["empty"].append(p.name)
    elif n == 1 and "test_support::assert_toml_cases" in tail:
        mods["helper_only"].append(p.name)
    else:
        mods["unique_tests"].append(p.name)
print("no test mod:", len(mods["empty"]))
print("helper-only:", len(mods["helper_only"]))
print("with unique tests:", len(mods["unique_tests"]))
