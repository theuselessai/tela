use rquickjs::Ctx;
use std::path::PathBuf;

const STORAGE_BOOTSTRAP: &str = r#"
globalThis.storage = {
    get: function(key) {
        var val = __tela_storage_get__(String(key));
        if (val === null || val === undefined) return null;
        try { return JSON.parse(val); } catch(e) { return val; }
    },
    set: function(key, value) {
        __tela_storage_set__(String(key), JSON.stringify(value));
    },
    remove: function(key) {
        __tela_storage_remove__(String(key));
    },
};
"#;

pub fn register_storage(ctx: &Ctx<'_>, app_name: &str) -> anyhow::Result<()> {
    let storage_dir = storage_dir_for(app_name);
    std::fs::create_dir_all(&storage_dir).ok();

    let dir = storage_dir.clone();
    let get = move |key: String| -> Option<String> {
        let path = dir.join(sanitize_key(&key));
        std::fs::read_to_string(path).ok()
    };

    let dir = storage_dir.clone();
    let set = move |key: String, value: String| {
        let path = dir.join(sanitize_key(&key));
        std::fs::write(path, value).ok();
    };

    let dir = storage_dir;
    let remove = move |key: String| {
        let path = dir.join(sanitize_key(&key));
        std::fs::remove_file(path).ok();
    };

    ctx.globals().set(
        "__tela_storage_get__",
        rquickjs::Function::new(ctx.clone(), get)?,
    )?;
    ctx.globals().set(
        "__tela_storage_set__",
        rquickjs::Function::new(ctx.clone(), set)?,
    )?;
    ctx.globals().set(
        "__tela_storage_remove__",
        rquickjs::Function::new(ctx.clone(), remove)?,
    )?;

    ctx.eval::<(), _>(STORAGE_BOOTSTRAP)
        .map_err(|e| anyhow::anyhow!("failed to register storage bootstrap: {e}"))?;

    Ok(())
}

fn storage_dir_for(app_name: &str) -> PathBuf {
    let base = dirs_next::data_local_dir().unwrap_or_else(|| PathBuf::from(".tela"));
    base.join("tela").join(app_name)
}

fn sanitize_key(key: &str) -> String {
    key.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}
