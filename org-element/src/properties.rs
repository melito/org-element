//! Common properties shared by all Org elements and objects.

use std::collections::HashMap;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Standard properties present on all Org syntax nodes.
///
/// These correspond to the standard properties defined in org-element.el.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct StandardProperties {
    /// Buffer position where the element or object starts.
    pub begin: usize,

    /// Buffer position where the element or object ends.
    pub end: usize,

    /// Number of blank lines or white space following the element.
    pub post_blank: usize,

    /// Buffer position after affiliated keywords (elements only).
    /// For objects, this is the same as `begin`.
    pub post_affiliated: usize,

    /// Buffer position where the element's contents start (if applicable).
    pub contents_begin: Option<usize>,

    /// Buffer position where the element's contents end (if applicable).
    pub contents_end: Option<usize>,
}

impl StandardProperties {
    /// Create new standard properties.
    pub fn new(begin: usize, end: usize) -> Self {
        Self {
            begin,
            end,
            post_blank: 0,
            post_affiliated: begin,
            contents_begin: None,
            contents_end: None,
        }
    }

    /// Set the contents region.
    pub fn with_contents(mut self, begin: usize, end: usize) -> Self {
        self.contents_begin = Some(begin);
        self.contents_end = Some(end);
        self
    }

    /// Set post-affiliated position.
    pub fn with_post_affiliated(mut self, pos: usize) -> Self {
        self.post_affiliated = pos;
        self
    }

    /// Set post-blank count.
    pub fn with_post_blank(mut self, count: usize) -> Self {
        self.post_blank = count;
        self
    }
}

/// Dynamic properties container for element/object-specific properties.
///
/// This stores properties beyond the standard set, allowing each element
/// type to have its own custom properties while maintaining a uniform interface.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Properties {
    properties: HashMap<String, PropertyValue>,
}

/// Possible values for properties.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(untagged))]
pub enum PropertyValue {
    /// String value.
    String(String),
    /// Integer value.
    Integer(i64),
    /// Boolean value.
    Boolean(bool),
    /// List of strings.
    StringList(Vec<String>),
    /// Nested properties.
    Properties(Properties),
}

impl Properties {
    /// Create a new empty properties container.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a string property.
    pub fn set_string(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.properties
            .insert(key.into(), PropertyValue::String(value.into()));
    }

    /// Insert an integer property.
    pub fn set_integer(&mut self, key: impl Into<String>, value: i64) {
        self.properties
            .insert(key.into(), PropertyValue::Integer(value));
    }

    /// Insert a boolean property.
    pub fn set_boolean(&mut self, key: impl Into<String>, value: bool) {
        self.properties
            .insert(key.into(), PropertyValue::Boolean(value));
    }

    /// Insert a string list property.
    pub fn set_string_list(&mut self, key: impl Into<String>, value: Vec<String>) {
        self.properties
            .insert(key.into(), PropertyValue::StringList(value));
    }

    /// Get a property value.
    pub fn get(&self, key: &str) -> Option<&PropertyValue> {
        self.properties.get(key)
    }

    /// Get a string property.
    pub fn get_string(&self, key: &str) -> Option<&str> {
        match self.properties.get(key) {
            Some(PropertyValue::String(s)) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Get an integer property.
    pub fn get_integer(&self, key: &str) -> Option<i64> {
        match self.properties.get(key) {
            Some(PropertyValue::Integer(i)) => Some(*i),
            _ => None,
        }
    }

    /// Get a boolean property.
    pub fn get_boolean(&self, key: &str) -> Option<bool> {
        match self.properties.get(key) {
            Some(PropertyValue::Boolean(b)) => Some(*b),
            _ => None,
        }
    }

    /// Get a string list property.
    pub fn get_string_list(&self, key: &str) -> Option<&[String]> {
        match self.properties.get(key) {
            Some(PropertyValue::StringList(list)) => Some(list.as_slice()),
            _ => None,
        }
    }

    /// Check if a property exists.
    pub fn contains(&self, key: &str) -> bool {
        self.properties.contains_key(key)
    }

    /// Remove a property.
    pub fn remove(&mut self, key: &str) -> Option<PropertyValue> {
        self.properties.remove(key)
    }

    /// Get all property keys.
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.properties.keys()
    }

    /// Iterate over all properties.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &PropertyValue)> {
        self.properties.iter()
    }
}

impl PropertyValue {
    /// Try to get the value as a string.
    pub fn as_string(&self) -> Option<&str> {
        match self {
            PropertyValue::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Try to get the value as an integer.
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            PropertyValue::Integer(i) => Some(*i),
            _ => None,
        }
    }

    /// Try to get the value as a boolean.
    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            PropertyValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// Try to get the value as a string list.
    pub fn as_string_list(&self) -> Option<&[String]> {
        match self {
            PropertyValue::StringList(list) => Some(list.as_slice()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_properties() {
        let props = StandardProperties::new(0, 100)
            .with_contents(10, 90)
            .with_post_blank(2);

        assert_eq!(props.begin, 0);
        assert_eq!(props.end, 100);
        assert_eq!(props.contents_begin, Some(10));
        assert_eq!(props.contents_end, Some(90));
        assert_eq!(props.post_blank, 2);
    }

    #[test]
    fn test_properties() {
        let mut props = Properties::new();
        props.set_string("title", "Hello");
        props.set_integer("level", 1);
        props.set_boolean("archived", false);
        props.set_string_list("tags", vec!["tag1".to_string(), "tag2".to_string()]);

        assert_eq!(props.get_string("title"), Some("Hello"));
        assert_eq!(props.get_integer("level"), Some(1));
        assert_eq!(props.get_boolean("archived"), Some(false));
        assert_eq!(
            props.get_string_list("tags"),
            Some(&["tag1".to_string(), "tag2".to_string()][..])
        );
    }
}
