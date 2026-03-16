pub mod elements;
pub mod engine;
pub mod native;
pub mod renderer;

pub use elements::Element;
pub use engine::Engine;

pub struct Manifest {
    pub name: String,
    pub version: String,
    pub entry: String,
    pub permissions: Vec<String>,
}

impl Manifest {
    pub fn has_permission(&self, perm: &str) -> bool {
        self.permissions.iter().any(|p| p == perm)
    }
}

pub fn parse_manifest(json_str: &str) -> anyhow::Result<Manifest> {
    let val: serde_json::Value =
        serde_json::from_str(json_str).map_err(|e| anyhow::anyhow!("invalid manifest JSON: {e}"))?;
    let permissions = val
        .get("permissions")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    Ok(Manifest {
        name: val
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string(),
        version: val
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("0.0.0")
            .to_string(),
        entry: val
            .get("entry")
            .and_then(|v| v.as_str())
            .unwrap_or("bundle.js")
            .to_string(),
        permissions,
    })
}
