//! The owned value tree a config leaves behind.
//!
//! The Lua VM is torn down as soon as `init.lua` has run, so nothing here
//! borrows from it: a composition holds plain Rust data and never links
//! against a garbage collector at draw time. Values keep Lua's own
//! integer/float split, because a colour component and a scale factor want
//! different things from `15`.

use std::collections::BTreeMap;

use patin::ui::Color;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Boolean(bool),
    Integer(i64),
    Number(f64),
    String(String),
    Table(Table),
    /// A Lua function the config registered. Functions cannot leave the VM,
    /// so this records only that one was there — enough to tell a config
    /// author their callback was seen, and to refuse it with a clear message
    /// where a value was wanted.
    Function,
}

/// A harvested Lua table: the array part in order, the named part sorted.
///
/// Sorted rather than insertion-ordered because Lua's own iteration order is
/// unspecified, so insertion order was never recoverable. Anything that needs
/// a deliberate order — the session menu's rows — carries its own `order`
/// field instead of relying on how the file happened to be written.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Table {
    pub array: Vec<Value>,
    pub map: BTreeMap<String, Value>,
}

impl Table {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.map.get(key)
    }

    /// Resolve a dotted path such as `bar.pill.width`.
    pub fn path(&self, path: &str) -> Option<&Value> {
        let mut current = self;
        let mut parts = path.split('.').peekable();
        while let Some(part) = parts.next() {
            let value = current.map.get(part)?;
            if parts.peek().is_none() {
                return Some(value);
            }
            match value {
                Value::Table(table) => current = table,
                _ => return None,
            }
        }
        None
    }

    pub fn is_empty(&self) -> bool {
        self.array.is_empty() && self.map.is_empty()
    }

    /// Every leaf path in this table, dotted, for unknown-key reporting.
    pub(crate) fn leaf_paths(&self, prefix: &str, out: &mut Vec<String>) {
        for (key, value) in &self.map {
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            match value {
                // A namespace Patin parked and the config never touched is
                // not a setting anybody wrote, so it is not a typo either.
                Value::Table(table) if table.is_empty() => {}
                Value::Table(table) if !table.map.is_empty() => table.leaf_paths(&path, out),
                _ => out.push(path),
            }
        }
    }
}

impl Value {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Boolean(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Integer(value) => Some(*value as f64),
            Value::Number(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_table(&self) -> Option<&Table> {
        match self {
            Value::Table(table) => Some(table),
            _ => None,
        }
    }

    /// `"#rrggbb"`, `"#rrggbbaa"`, or `{ 124, 58, 237, 255 }`.
    ///
    /// Alpha defaults to opaque, because a config that names a colour almost
    /// never means "and make it invisible", and a three-component list is the
    /// obvious way to write one.
    pub fn as_color(&self) -> Option<Color> {
        match self {
            Value::String(text) => parse_hex(text),
            Value::Table(table) => {
                let mut parts = [0u8; 4];
                parts[3] = 255;
                if table.array.len() < 3 || table.array.len() > 4 {
                    return None;
                }
                for (slot, value) in parts.iter_mut().zip(&table.array) {
                    let number = value.as_f64()?;
                    if !(0.0..=255.0).contains(&number) {
                        return None;
                    }
                    *slot = number.round() as u8;
                }
                Some(Color(parts[0], parts[1], parts[2], parts[3]))
            }
            _ => None,
        }
    }

    /// The type name to quote back in a warning, in Lua's own vocabulary.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Boolean(_) => "boolean",
            Value::Integer(_) | Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Table(_) => "table",
            Value::Function => "function",
        }
    }
}

fn parse_hex(text: &str) -> Option<Color> {
    let digits = text.strip_prefix('#')?;
    if digits.len() != 6 && digits.len() != 8 {
        return None;
    }
    let mut parts = [255u8; 4];
    for (index, slot) in parts.iter_mut().enumerate().take(digits.len() / 2) {
        let pair = digits.get(index * 2..index * 2 + 2)?;
        *slot = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(Color(parts[0], parts[1], parts[2], parts[3]))
}

#[cfg(test)]
mod tests {
    use super::{Table, Value};
    use patin::ui::Color;

    fn table(pairs: Vec<(&str, Value)>) -> Table {
        Table {
            array: Vec::new(),
            map: pairs
                .into_iter()
                .map(|(key, value)| (key.to_string(), value))
                .collect(),
        }
    }

    #[test]
    fn dotted_paths_walk_nested_tables_and_stop_at_non_tables() {
        let root = table(vec![(
            "bar",
            Value::Table(table(vec![(
                "pill",
                Value::Table(table(vec![("width", Value::Integer(32))])),
            )])),
        )]);
        assert_eq!(root.path("bar.pill.width"), Some(&Value::Integer(32)));
        assert_eq!(root.path("bar.pill.height"), None);
        assert_eq!(root.path("bar.pill.width.deeper"), None);
        assert_eq!(root.path("nothing"), None);
    }

    #[test]
    fn colors_accept_hex_with_and_without_alpha_and_component_lists() {
        assert_eq!(
            Value::String("#7c3aed".into()).as_color(),
            Some(Color(124, 58, 237, 255))
        );
        assert_eq!(
            Value::String("#7c3aed80".into()).as_color(),
            Some(Color(124, 58, 237, 128))
        );
        assert_eq!(
            Value::Table(Table {
                array: vec![Value::Integer(124), Value::Integer(58), Value::Integer(237)],
                map: Default::default(),
            })
            .as_color(),
            Some(Color(124, 58, 237, 255))
        );
        assert_eq!(Value::String("7c3aed".into()).as_color(), None);
        assert_eq!(Value::String("#7c3ae".into()).as_color(), None);
        assert_eq!(Value::Integer(3).as_color(), None);
    }

    #[test]
    fn leaf_paths_report_every_assignment_but_not_the_tables_holding_them() {
        let root = table(vec![
            (
                "theme",
                Value::Table(table(vec![("accent", Value::Integer(1))])),
            ),
            ("height", Value::Integer(14)),
        ]);
        let mut paths = Vec::new();
        root.leaf_paths("", &mut paths);
        assert_eq!(
            paths,
            vec!["height".to_string(), "theme.accent".to_string()]
        );
    }
}
