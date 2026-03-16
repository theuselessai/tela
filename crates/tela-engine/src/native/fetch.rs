use rquickjs::Ctx;
use tokio::sync::mpsc;

const FETCH_BOOTSTRAP: &str = r#"
globalThis.__tela_fetch_resolvers__ = {};
globalThis.__tela_next_fetch_id__ = 1;

globalThis.__tela_resolve_fetch__ = function(id, resultJson) {
    var entry = globalThis.__tela_fetch_resolvers__[id];
    if (entry) {
        delete globalThis.__tela_fetch_resolvers__[id];
        try {
            var result = JSON.parse(resultJson);
            if (result.error) {
                entry.reject(new Error(result.error));
            } else {
                entry.resolve({
                    ok: result.status >= 200 && result.status < 300,
                    status: result.status,
                    statusText: result.statusText || "",
                    _body: result.body || "",
                    json: function() { return JSON.parse(this._body); },
                    text: function() { return this._body; },
                });
            }
        } catch (e) {
            entry.reject(e);
        }
    }
};

globalThis.fetch = function(url, opts) {
    opts = opts || {};
    var id = globalThis.__tela_next_fetch_id__++;
    var p = new Promise(function(resolve, reject) {
        globalThis.__tela_fetch_resolvers__[id] = { resolve: resolve, reject: reject };
    });
    __tela_native_fetch__(
        id,
        String(url),
        String(opts.method || "GET"),
        opts.body != null ? String(opts.body) : "",
        opts.headers ? JSON.stringify(opts.headers) : "{}"
    );
    return p;
};
"#;

pub fn register_fetch(
    ctx: &Ctx<'_>,
    action_tx: mpsc::UnboundedSender<serde_json::Value>,
    handle: tokio::runtime::Handle,
) -> anyhow::Result<()> {
    let tx = action_tx.clone();
    let rt_handle = handle.clone();

    let native_fetch = move |id: u64, url: String, method: String, body: String, headers_json: String| {
        let tx = tx.clone();
        rt_handle.spawn(async move {
            let client = reqwest::Client::new();
            let mut req = match method.to_uppercase().as_str() {
                "POST" => client.post(&url),
                "PUT" => client.put(&url),
                "DELETE" => client.delete(&url),
                "PATCH" => client.patch(&url),
                _ => client.get(&url),
            };

            if let Ok(headers) =
                serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&headers_json)
            {
                for (key, val) in headers {
                    if let Some(v) = val.as_str() {
                        req = req.header(key.as_str(), v);
                    }
                }
            }

            if !body.is_empty() {
                req = req.body(body);
            }

            let result_json = match req.send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let status_text = resp
                        .status()
                        .canonical_reason()
                        .unwrap_or("")
                        .to_string();
                    let body = resp.text().await.unwrap_or_default();
                    serde_json::json!({
                        "status": status,
                        "statusText": status_text,
                        "body": body,
                    })
                }
                Err(e) => {
                    serde_json::json!({ "error": e.to_string() })
                }
            };

            let _ = tx.send(serde_json::json!({
                "type": "__tela_fetch__",
                "id": id,
                "result": result_json.to_string(),
            }));
        });
    };

    ctx.globals().set(
        "__tela_native_fetch__",
        rquickjs::Function::new(ctx.clone(), native_fetch)?,
    )?;

    ctx.eval::<(), _>(FETCH_BOOTSTRAP)
        .map_err(|e| anyhow::anyhow!("failed to register fetch bootstrap: {e}"))?;

    Ok(())
}
