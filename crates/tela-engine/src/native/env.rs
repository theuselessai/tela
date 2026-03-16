use rquickjs::Ctx;

const ENV_BOOTSTRAP: &str = r#"
globalThis.env = {
    get: function(key) {
        return __tela_env_get__(String(key));
    },
};
"#;

pub fn register_env(ctx: &Ctx<'_>) -> anyhow::Result<()> {
    let get = |key: String| -> Option<String> { std::env::var(&key).ok() };

    ctx.globals().set(
        "__tela_env_get__",
        rquickjs::Function::new(ctx.clone(), get)?,
    )?;

    ctx.eval::<(), _>(ENV_BOOTSTRAP)
        .map_err(|e| anyhow::anyhow!("failed to register env bootstrap: {e}"))?;

    Ok(())
}
