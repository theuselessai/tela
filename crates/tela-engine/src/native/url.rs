//! Generic URL parsing without filesystem or network access.
use rquickjs::{Ctx, Exception, Function};
fn parse(ctx: Ctx<'_>, input: String, base: Option<String>) -> rquickjs::Result<String> {
    if input.len() > 8192 || base.as_ref().is_some_and(|s| s.len() > 8192) {
        return Err(Exception::throw_range(&ctx, "URL exceeds limit"));
    }
    let bad = || Exception::throw_type(&ctx, "invalid URL");
    let base = base
        .map(|s| reqwest::Url::parse(&s))
        .transpose()
        .map_err(|_| bad())?;
    let url = reqwest::Url::options()
        .base_url(base.as_ref())
        .parse(&input)
        .map_err(|_| bad())?;
    let result = serde_json::json!({
        "href":url.as_str(),"origin":url.origin().ascii_serialization(),
        "scheme":url.scheme(),"host":url.host_str(),"port":url.port(),
        "path":url.path(),"query":url.query(),"fragment":url.fragment(),
        "username":url.username(),"password":url.password()
    });
    serde_json::to_string(&result).map_err(|_| bad())
}
pub(crate) fn register_url(ctx: &Ctx<'_>) -> anyhow::Result<()> {
    ctx.globals()
        .set("__tela_url_parse__", Function::new(ctx.clone(), parse)?)?;
    ctx.eval::<(),_>(r#"
        Tela.url={parse:function(input,base){
            if(typeof input!=='string'||(base!==undefined&&typeof base!=='string'))throw new TypeError('URL strings required');
            return JSON.parse(__tela_url_parse__(input,base===undefined?null:base));
        }};
    "#)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_and_resolves_without_silent_security_policy() {
        let rt = rquickjs::Runtime::new().unwrap();
        let context = rquickjs::Context::full(&rt).unwrap();
        context.with(|ctx|{
            ctx.eval::<(),_>("globalThis.Tela={}").unwrap();register_url(&ctx).unwrap();
            assert!(ctx.eval::<bool,_>(r#"
                var a=Tela.url.parse('https://example.test/a#private');
                var b=Tela.url.parse('next.js','https://example.test/dir/manifest.json');
                a.scheme==='https'&&a.fragment==='private'&&a.username===''&&a.password===null&&b.href==='https://example.test/dir/next.js';
            "#).unwrap());
            assert!(ctx.eval::<bool,_>("try{Tela.url.parse('not a URL with private data');false}catch(e){!e.message.includes('private data')}").unwrap());
        });
    }
}
