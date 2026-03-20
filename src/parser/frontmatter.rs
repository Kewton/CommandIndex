use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Frontmatter {
    pub tags: Vec<String>,
    pub raw: HashMap<String, serde_json::Value>,
}

#[derive(Deserialize)]
struct RawFrontmatter {
    #[serde(default)]
    tags: Option<TagsValue>,
    #[serde(flatten)]
    rest: HashMap<String, serde_yaml::Value>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TagsValue {
    List(Vec<String>),
    Single(String),
}

/// frontmatter 文字列（YAML）をパースする
pub fn parse_frontmatter(yaml_str: &str) -> Option<Frontmatter> {
    let raw_fm: RawFrontmatter = serde_yaml::from_str(yaml_str).ok()?;

    let tags = match raw_fm.tags {
        Some(TagsValue::List(list)) => list,
        Some(TagsValue::Single(s)) => s.split(',').map(|t| t.trim().to_string()).collect(),
        None => Vec::new(),
    };

    let raw: HashMap<String, serde_json::Value> = raw_fm
        .rest
        .into_iter()
        .filter_map(|(k, v)| {
            let json_str = serde_json::to_string(&serde_yaml_to_json(v)).ok()?;
            let json_val = serde_json::from_str(&json_str).ok()?;
            Some((k, json_val))
        })
        .collect();

    Some(Frontmatter { tags, raw })
}

/// Markdownテキストからfrontmatter部分を抽出する
/// 戻り値: (frontmatter文字列, frontmatter以降のコンテンツ, frontmatter行数)
pub fn extract_frontmatter(content: &str) -> (Option<String>, &str, usize) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (None, content, 0);
    }

    // Find the opening ---
    let start = content.find("---").unwrap();
    let after_first = start + 3;
    let rest = &content[after_first..];

    // Skip the newline after ---
    let rest = rest
        .strip_prefix('\n')
        .or_else(|| rest.strip_prefix("\r\n"))
        .unwrap_or(rest);

    // Find closing ---
    if let Some(end_pos) = rest.find("\n---") {
        let yaml_str = &rest[..end_pos];
        let after_close = end_pos + 4; // "\n---".len()
        let remaining = &rest[after_close..];
        let remaining = remaining
            .strip_prefix('\n')
            .or_else(|| remaining.strip_prefix("\r\n"))
            .unwrap_or(remaining);

        let fm_lines = content[..content.len() - remaining.len()].lines().count();
        (Some(yaml_str.to_string()), remaining, fm_lines)
    } else {
        (None, content, 0)
    }
}

fn serde_yaml_to_json(val: serde_yaml::Value) -> serde_json::Value {
    match val {
        serde_yaml::Value::Null => serde_json::Value::Null,
        serde_yaml::Value::Bool(b) => serde_json::Value::Bool(b),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                serde_json::Value::Number(i.into())
            } else if let Some(f) = n.as_f64() {
                serde_json::Number::from_f64(f)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null)
            } else {
                serde_json::Value::Null
            }
        }
        serde_yaml::Value::String(s) => serde_json::Value::String(s),
        serde_yaml::Value::Sequence(seq) => {
            serde_json::Value::Array(seq.into_iter().map(serde_yaml_to_json).collect())
        }
        serde_yaml::Value::Mapping(map) => {
            let obj: serde_json::Map<String, serde_json::Value> = map
                .into_iter()
                .map(|(k, v)| {
                    let key = match k {
                        serde_yaml::Value::String(s) => s,
                        other => format!("{other:?}"),
                    };
                    (key, serde_yaml_to_json(v))
                })
                .collect();
            serde_json::Value::Object(obj)
        }
        serde_yaml::Value::Tagged(tagged) => serde_yaml_to_json(tagged.value),
    }
}
