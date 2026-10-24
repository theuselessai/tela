//! Bounded UTF-8 codecs for JS apps. No filesystem/network capability.
use rquickjs::{Ctx, Exception, Function, TypedArray};
const MAX_BYTES: usize = 4 * 1024 * 1024;

fn encode<'js>(ctx: Ctx<'js>, text: String) -> rquickjs::Result<TypedArray<'js, u8>> {
    if text.len() > MAX_BYTES {
        return Err(Exception::throw_range(&ctx, "UTF-8 input exceeds limit"));
    }
    // QuickJS owns the allocation; safe even if a host detaches the buffer.
    TypedArray::new_copy(ctx, text.as_bytes())
}
fn decode(ctx: Ctx<'_>, input: TypedArray<'_, u8>, fatal: bool) -> rquickjs::Result<String> {
    let bytes = input
        .as_bytes()
        .ok_or_else(|| Exception::throw_type(&ctx, "detached byte array"))?;
    if bytes.len() > MAX_BYTES {
        return Err(Exception::throw_range(&ctx, "UTF-8 input exceeds limit"));
    }
    let text = if fatal {
        std::str::from_utf8(bytes)
            .map_err(|_| Exception::throw_type(&ctx, "invalid UTF-8"))?
            .to_owned()
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    };
    if text.len() > MAX_BYTES {
        return Err(Exception::throw_range(&ctx, "UTF-8 output exceeds limit"));
    }
    Ok(text)
}
pub(crate) fn register_encoding(ctx: &Ctx<'_>) -> anyhow::Result<()> {
    ctx.globals()
        .set("__tela_utf8_encode__", Function::new(ctx.clone(), encode)?)?;
    ctx.globals()
        .set("__tela_utf8_decode__", Function::new(ctx.clone(), decode)?)?;
    ctx.eval::<(), _>(BOOTSTRAP)?;
    Ok(())
}
const BOOTSTRAP: &str = r#"
(function(){
    function wellFormed(value) {
        var s=String(value), out=[];
        if(s.length>4194304)throw new RangeError('UTF-8 input exceeds limit');
        if(!/[\ud800-\udfff]/.test(s))return s;
        for(var i=0;i<s.length;i++) {
            var c=s.charCodeAt(i);
            if(c>=0xd800 && c<=0xdbff) {
                var d=s.charCodeAt(i+1);
                if(d>=0xdc00 && d<=0xdfff){out.push(s[i]+s[++i]);}
                else out.push('\ufffd');
            } else if(c>=0xdc00 && c<=0xdfff)out.push('\ufffd');
            else out.push(s[i]);
        }
        return out.join('');
    }
    globalThis.TextEncoder=class TextEncoder {
        get encoding(){return 'utf-8';}
        encode(input=''){return __tela_utf8_encode__(wellFormed(input));}
    };
    globalThis.TextDecoder=class TextDecoder {
        constructor(label='utf-8',options={}) {
            label=String(label).trim().toLowerCase();
            if(!['utf-8','utf8','unicode-1-1-utf-8'].includes(label))throw new RangeError('only UTF-8 decoding is supported');
            this._fatal=Boolean(options.fatal);this._ignoreBOM=Boolean(options.ignoreBOM);
        }
        get encoding(){return 'utf-8';}
        get fatal(){return this._fatal;}
        get ignoreBOM(){return this._ignoreBOM;}
        decode(input=new Uint8Array(0),options={}) {
            if(options.stream)throw new TypeError('streaming UTF-8 decoding is not supported');
            var bytes;
            if(input instanceof ArrayBuffer)bytes=new Uint8Array(input);
            else if(ArrayBuffer.isView(input))bytes=new Uint8Array(input.buffer,input.byteOffset,input.byteLength);
            else throw new TypeError('expected an ArrayBuffer or byte view');
            var value=__tela_utf8_decode__(bytes,this._fatal);
            return !this._ignoreBOM && value.charCodeAt(0)===0xfeff?value.slice(1):value;
        }
    };
})();
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use rquickjs::{Context, Runtime};
    #[test]
    fn utf8_views_surrogates_and_fatal_errors() {
        let rt = Runtime::new().unwrap();
        let context = Context::full(&rt).unwrap();
        context.with(|ctx| {
            register_encoding(&ctx).unwrap();
            let encoded = encode(ctx.clone(), "detach fixture".into()).unwrap();
            let mut backing = encoded.arraybuffer().unwrap();
            backing.detach();
            assert!(encoded.as_bytes().is_none());
            assert!(ctx.eval::<bool,_>(r#"
                var enc=new TextEncoder(),dec=new TextDecoder('utf8',{fatal:true});
                var data=enc.encode('a猫🙂z');
                dec.decode(data.subarray(1,data.length-1))==='猫🙂' &&
                new TextDecoder().decode(enc.encode('\ud800x\udc00'))==='\ufffdx\ufffd' &&
                new TextDecoder().decode(new Uint8Array([239,187,191,97]))==='a' &&
                new TextDecoder('utf-8',{ignoreBOM:true}).decode(new Uint8Array([239,187,191,97]))==='\ufeffa' &&
                new TextDecoder().decode(new Uint8Array([255]))==='\ufffd';
            "#).unwrap());
            for source in ["new TextDecoder('utf8',{fatal:true}).decode(new Uint8Array([255]))", "new TextDecoder('latin1')", "new TextDecoder().decode('text')", "new TextDecoder().decode(new Uint8Array(),{stream:true})"] {
                assert!(ctx.eval::<(),_>(source).is_err());let _=ctx.catch();
            }
        });
    }
}
