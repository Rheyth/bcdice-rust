//! `test/data/*.toml` のモデルと読み込み。
//!
//! TOML形式（test/test_game_system_commands.rb が解釈するものと同一）:
//!
//! ```toml
//! [[ test ]]
//! game_system = "AFF2e"
//! input = "FF8"
//! output = "(2D6<=8) ＞ 5[4,1] ＞ 成功"
//! rands = [ { sides = 6, value = 4 }, { sides = 6, value = 1 } ]
//! success = true   # 省略時 false
//! ```
//!
//! TOMLではnilを表現できないため、Ruby側は `output = ""` を nil として扱う
//! （test_game_system_commands.rb:56）。本モデルも同じ規約に従う。

use serde::Deserialize;

/// 1件のテストケース。
#[derive(Debug, Clone, Deserialize)]
pub struct TestCase {
    pub game_system: String,
    pub input: String,
    /// 期待出力。空文字列は「nil（評価結果なし）」を意味する。
    #[serde(default)]
    pub output: String,
    #[serde(default)]
    pub secret: bool,
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub failure: bool,
    #[serde(default)]
    pub critical: bool,
    #[serde(default)]
    pub fumble: bool,
    #[serde(default)]
    pub rands: Vec<Rand>,
}

impl TestCase {
    /// 期待出力が「nil」（空文字規約）かどうか。
    pub fn expects_nil(&self) -> bool {
        self.output.is_empty()
    }
}

/// 注入する出目。TOMLでは `{ value = 4, sides = 6 }`。
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Rand {
    pub value: i64,
    pub sides: i64,
}

/// 1ファイル分のTOML。
#[derive(Debug, Clone, Deserialize)]
pub struct TestDataFile {
    #[serde(rename = "test", default)]
    pub tests: Vec<TestCase>,
}

/// 読み込みエラー。
#[derive(Debug)]
pub enum LoadError {
    Io(std::io::Error),
    Parse {
        path: std::path::PathBuf,
        message: String,
    },
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Io(e) => write!(f, "IO error: {e}"),
            LoadError::Parse { path, message } => {
                write!(f, "TOML parse error in {path:?}: {message}")
            }
        }
    }
}

impl TestDataFile {
    /// TOML文字列をパースする。
    pub fn parse_str(path: &std::path::Path, s: &str) -> Result<Self, LoadError> {
        toml::from_str(s).map_err(|e| LoadError::Parse {
            path: path.to_path_buf(),
            message: e.to_string(),
        })
    }

    /// ファイルから読み込む。
    pub fn load(path: &std::path::Path) -> Result<Self, LoadError> {
        let s = std::fs::read_to_string(path).map_err(LoadError::Io)?;
        Self::parse_str(path, &s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
[[ test ]]
game_system = "AFF2e"
input = "FF8"
output = "(2D6<=8) ＞ 5[4,1] ＞ 成功"
rands = [
  { sides = 6, value = 4 },
  { sides = 6, value = 1 },
]

[[ test ]]
game_system = "Foo"
input = "1D100"
output = ""
"#;

    #[test]
    fn parse_sample() {
        let data = TestDataFile::parse_str(std::path::Path::new("sample.toml"), SAMPLE).unwrap();
        assert_eq!(data.tests.len(), 2);
        let t0 = &data.tests[0];
        assert_eq!(t0.game_system, "AFF2e");
        assert_eq!(t0.rands.len(), 2);
        assert_eq!(t0.rands[0], Rand { value: 4, sides: 6 });
        assert!(!t0.success);
        let t1 = &data.tests[1];
        assert!(t1.expects_nil());
    }
}
