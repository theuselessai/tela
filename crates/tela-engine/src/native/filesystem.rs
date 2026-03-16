use rquickjs::Ctx;

const FILESYSTEM_BOOTSTRAP: &str = r#"
globalThis.Tela.readFile = function(path) {
    return __tela_read_file__(String(path));
};
"#;

pub fn register_filesystem(ctx: &Ctx<'_>, permissions: &[String]) -> anyhow::Result<()> {
    if !permissions.contains(&"filesystem".to_string()) {
        return Ok(());
    }

    let read_file = move |path: String| -> Option<String> {
        let expanded = if path.starts_with("~/") {
            if let Some(home) = dirs_next::home_dir() {
                home.join(&path[2..]).to_string_lossy().to_string()
            } else {
                path
            }
        } else {
            path
        };
        std::fs::read_to_string(expanded).ok()
    };

    ctx.globals().set(
        "__tela_read_file__",
        rquickjs::Function::new(ctx.clone(), read_file)?,
    )?;

    ctx.eval::<(), _>(FILESYSTEM_BOOTSTRAP)
        .map_err(|e| anyhow::anyhow!("failed to register filesystem bootstrap: {e}"))?;

    Ok(())
}
