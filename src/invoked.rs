// The invoked payload. Mirrors @radiantabyss/neutralino Invoked.js, plus the reads every
// action repeats: a required string, a lenient bool or number, or the whole payload as a struct.

use serde::de::DeserializeOwned;
use serde_json::Value;

pub struct Invoked(Value);

impl Invoked {
    pub fn new(payload: Value) -> Invoked {
        Invoked(payload)
    }

    pub fn all(&self) -> &Value {
        &self.0
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }

    pub fn has(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// A string that has to be there and not be empty.
    pub fn str(&self, key: &str) -> Result<&str, String> {
        match self.get(key).and_then(|v| v.as_str()) {
            Some(s) if !s.is_empty() => Ok(s),
            _ => Err(format!("{key} is required.")),
        }
    }

    /// Lenient: true, any non-zero number, "1" and "true" are true; anything else, or
    /// nothing, is false.
    pub fn bool(&self, key: &str) -> bool {
        match self.get(key) {
            Some(Value::Bool(b)) => *b,
            Some(Value::Number(n)) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
            Some(Value::String(s)) => s == "1" || s.eq_ignore_ascii_case("true"),
            _ => false,
        }
    }

    /// A whole number, sent as a number or a numeric string.
    pub fn int(&self, key: &str) -> Result<i64, String> {
        let int = match self.get(key) {
            Some(Value::Number(n)) => n.as_i64(),
            Some(Value::String(s)) => s.trim().parse().ok(),
            _ => None,
        };

        int.ok_or_else(|| format!("{key} must be a number."))
    }

    /// The whole payload as a struct, for actions that read many fields.
    pub fn parse<T: DeserializeOwned>(&self) -> Result<T, String> {
        serde_json::from_value(self.0.clone()).map_err(|e| e.to_string())
    }
}
