//! Static product-message catalogs. User data and technical error details are never rewritten.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    En,
    Ru,
    Fr,
    De,
    Es,
    It,
}

impl Language {
    pub const ALL: [Self; 6] = [Self::En, Self::Ru, Self::Fr, Self::De, Self::Es, Self::It];
    pub fn native_name(self) -> &'static str {
        match self {
            Self::En => "English",
            Self::Ru => "Русский",
            Self::Fr => "Français",
            Self::De => "Deutsch",
            Self::Es => "Español",
            Self::It => "Italiano",
        }
    }
    fn index(self) -> Option<usize> {
        match self {
            Self::En => None,
            Self::Ru => Some(0),
            Self::Fr => Some(1),
            Self::De => Some(2),
            Self::Es => Some(3),
            Self::It => Some(4),
        }
    }
    pub fn t(self, english: &str) -> &str {
        self.index()
            .and_then(|index| catalog().get(english).map(|row| row[index]))
            .unwrap_or(english)
    }
    /// Format an explicitly selected product template, preserving every supplied value.
    pub fn format(self, english: &str, values: &[&str]) -> String {
        let mut pieces = self.t(english).split("{}");
        let mut result = pieces.next().unwrap_or_default().to_string();
        for (piece, value) in pieces.zip(values) {
            result.push_str(value);
            result.push_str(piece);
        }
        result
    }
    /// Translate only known canonical status/error messages produced by the workers.
    pub fn message(self, message: &str) -> String {
        if self == Self::En {
            return message.into();
        }
        if catalog().contains_key(message) {
            return self.t(message).into();
        }
        for template in catalog().keys().filter(|key| key.contains("{}")) {
            if let Some(values) = match_template(template, message) {
                return self.format(template, &values);
            }
        }
        self.format("Could not complete this action: {}", &[message])
    }
}

fn catalog() -> &'static BTreeMap<&'static str, [&'static str; 5]> {
    static CATALOG: OnceLock<BTreeMap<&'static str, [&'static str; 5]>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        include_str!("catalog.txt")
            .lines()
            .map(|line| {
                let row: Vec<_> = line.split('|').collect();
                assert_eq!(row.len(), 6, "Invalid translation row: {line}");
                (row[0], [row[1], row[2], row[3], row[4], row[5]])
            })
            .collect()
    })
}

fn match_template<'a>(template: &str, value: &'a str) -> Option<Vec<&'a str>> {
    let parts: Vec<_> = template.split("{}").collect();
    let mut rest = value.strip_prefix(parts[0])?;
    let mut values = Vec::new();
    for (index, part) in parts.iter().enumerate().skip(1) {
        if index == parts.len() - 1 {
            values.push(rest.strip_suffix(part)?);
            return Some(values);
        }
        let end = rest.find(part)?;
        values.push(&rest[..end]);
        rest = &rest[end + part.len()..];
    }
    None
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Light,
    Dark,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_catalogs_have_complete_messages_and_matching_placeholders() {
        let mut seen = std::collections::HashSet::new();
        for line in include_str!("catalog.txt").lines() {
            let row: Vec<_> = line.split('|').collect();
            assert_eq!(row.len(), 6);
            assert!(seen.insert(row[0]), "Duplicate: {}", row[0]);
            for translation in &row[1..] {
                assert!(!translation.trim().is_empty(), "Empty: {}", row[0]);
                assert_eq!(
                    row[0].matches("{}").count(),
                    translation.matches("{}").count(),
                    "Placeholders: {}",
                    row[0]
                );
            }
        }
    }
    #[test]
    fn message_values_and_unknown_details_are_preserved() {
        assert_eq!(
            Language::Ru.message("Version 2.4.7 is available."),
            "Доступна версия 2.4.7."
        );
        assert!(
            Language::De
                .message("Settings are busy. Try again: C:\\data\\file")
                .ends_with("C:\\data\\file")
        );
        assert_eq!(Language::En.t("Future message"), "Future message");
        assert!(
            Language::Fr
                .message("OS error 123")
                .ends_with("OS error 123")
        );
    }
}
