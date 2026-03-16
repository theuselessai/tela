use rquickjs::{function::Rest, Ctx, Object, Value};

fn format_values<'js>(ctx: Ctx<'js>, values: Rest<Value<'js>>) -> String {
    let mut parts = Vec::with_capacity(values.len());
    for val in values.0.into_iter() {
        let part = if val.is_string() {
            val.as_string()
                .and_then(|s| s.to_string().ok())
                .unwrap_or_default()
        } else {
            ctx.json_stringify(val)
                .ok()
                .flatten()
                .and_then(|s| s.to_string().ok())
                .unwrap_or_else(|| "[unstringifiable]".into())
        };
        parts.push(part);
    }
    parts.join(" ")
}

fn console_log<'js>(ctx: Ctx<'js>, values: Rest<Value<'js>>) {
    let msg = format_values(ctx, values);
    eprintln!("[JS] {msg}");
}

fn console_error<'js>(ctx: Ctx<'js>, values: Rest<Value<'js>>) {
    let msg = format_values(ctx, values);
    eprintln!("[JS ERROR] {msg}");
}

fn console_warn<'js>(ctx: Ctx<'js>, values: Rest<Value<'js>>) {
    let msg = format_values(ctx, values);
    eprintln!("[JS WARN] {msg}");
}

pub fn register_console(ctx: &Ctx<'_>) -> anyhow::Result<()> {
    let console = Object::new(ctx.clone())?;

    console.set(
        "log",
        rquickjs::Function::new(ctx.clone(), console_log)?.with_name("log")?,
    )?;
    console.set(
        "error",
        rquickjs::Function::new(ctx.clone(), console_error)?.with_name("error")?,
    )?;
    console.set(
        "warn",
        rquickjs::Function::new(ctx.clone(), console_warn)?.with_name("warn")?,
    )?;

    ctx.globals().set("console", console)?;
    Ok(())
}
