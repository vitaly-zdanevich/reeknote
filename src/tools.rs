use std::collections::BTreeMap;

pub fn check_is_int(value: &str) -> bool {
    value.trim().parse::<i64>().is_ok()
}

pub fn strip_string(value: &str) -> String {
    value
        .trim_matches([' ', '\t', '\n', '\r', '"', '\''])
        .to_string()
}

pub fn strip_option(value: Option<String>) -> Option<String> {
    value.map(|item| strip_string(&item))
}

pub fn strip_vec(values: &[String]) -> Vec<String> {
    values.iter().map(|item| strip_string(item)).collect()
}

pub fn strip_map(values: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    values
        .iter()
        .map(|(key, value)| (strip_string(key), value.clone()))
        .collect()
}

pub fn decode_args(args: &[String]) -> Vec<String> {
    args.to_vec()
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Struct {
    pub entries: BTreeMap<String, String>,
}

impl Struct {
    pub fn new(entries: BTreeMap<String, String>) -> Self {
        Self { entries }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_integer_strings() {
        assert!(check_is_int("1"));
        assert!(check_is_int("-1"));
        assert!(!check_is_int("1.1"));
        assert!(!check_is_int("abc"));
    }

    #[test]
    fn strips_known_wrapping_characters() {
        assert_eq!(strip_string("text text \t\n\r\"'"), "text text");
        assert_eq!(
            strip_vec(&["key \t\n\r\"'".to_string(), "value \t\n\r\"'".to_string()]),
            vec!["key".to_string(), "value".to_string()]
        );
    }
}
