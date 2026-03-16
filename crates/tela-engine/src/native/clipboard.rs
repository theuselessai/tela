use rquickjs::Ctx;

const CLIPBOARD_BOOTSTRAP: &str = r#"
globalThis.clipboard = {
    write: function(text) {
        __tela_clipboard_write__(String(text));
    },
    read: function() {
        return __tela_clipboard_read__();
    },
};
"#;

pub fn register_clipboard(ctx: &Ctx<'_>) -> anyhow::Result<()> {
    let write = |text: String| {
        set_clipboard(&text);
    };

    let read = || -> Option<String> { get_clipboard() };

    ctx.globals().set(
        "__tela_clipboard_write__",
        rquickjs::Function::new(ctx.clone(), write)?,
    )?;
    ctx.globals().set(
        "__tela_clipboard_read__",
        rquickjs::Function::new(ctx.clone(), read)?,
    )?;

    ctx.eval::<(), _>(CLIPBOARD_BOOTSTRAP)
        .map_err(|e| anyhow::anyhow!("failed to register clipboard bootstrap: {e}"))?;

    Ok(())
}

fn set_clipboard(text: &str) {
    #[cfg(target_os = "linux")]
    {
        use std::io::Write;
        if let Ok(mut child) = std::process::Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(ref mut stdin) = child.stdin {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
        }
    }
    #[cfg(target_os = "macos")]
    {
        use std::io::Write;
        if let Ok(mut child) = std::process::Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(ref mut stdin) = child.stdin {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
        }
    }
}

fn get_clipboard() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xclip")
            .args(["-selection", "clipboard", "-o"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("pbpaste")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}
