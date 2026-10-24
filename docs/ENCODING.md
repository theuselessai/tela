# UTF-8 text codecs

Tela supplies `TextEncoder` and a UTF-8-only `TextDecoder` without a permission.

```js
const bytes = new TextEncoder().encode('hello 猫'); // fresh Uint8Array
const text = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
```

Encoding replaces lone UTF-16 surrogates with U+FFFD. Decoding accepts an
ArrayBuffer or view and honors its offset/length. The default decoder replaces
invalid UTF-8; `fatal:true` rejects it. Leading BOM handling follows `ignoreBOM`
(default false). Inputs/outputs are bounded to 4 MiB; encoding also bounds the
input UTF-16 length before native conversion.

This is an intentionally limited surface, not a complete Encoding Standard
implementation: no other encodings, streaming decode or `encodeInto` yet.
Unsupported encodings and `stream:true` throw instead of silently guessing.
