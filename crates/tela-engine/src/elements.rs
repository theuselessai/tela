use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A node in the element tree produced by `h()` calls in JS.
/// Special tags: `__fragment__` (inlined during render), `__text__` (raw text from JSX strings).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Element {
    pub tag: String,

    #[serde(default)]
    pub props: HashMap<String, serde_json::Value>,

    #[serde(default)]
    pub children: Vec<Element>,
}

impl Element {
    pub fn is_fragment(&self) -> bool {
        self.tag == "__fragment__"
    }

    pub fn is_text(&self) -> bool {
        self.tag == "__text__"
    }

    pub fn text_content(&self) -> Option<&str> {
        if self.is_text() {
            self.props.get("content").and_then(|v| v.as_str())
        } else {
            None
        }
    }
}
