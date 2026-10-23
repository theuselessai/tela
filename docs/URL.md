# URL parsing

`Tela.url.parse(input, base?)` is a synchronous, capability-free parser. It does
not fetch anything or grant network access. Arguments must be strings, bounded
to 8 KiB each. Relative URLs require an absolute base.

```js
const u = Tela.url.parse('bundle.js', 'https://example.test/app/manifest.json');
// u.href: https://example.test/app/bundle.js
```

The returned plain object has `href`, `origin`, `scheme`, `host`, `port`, `path`,
`query`, `fragment`, `username` and `password`. Optional fields are null; scheme
has no colon and query/fragment omit their leading punctuation. Origin is `null`
as a string for opaque origins. Parsing uses the runtime's URL implementation;
this is not a complete browser `URL`/`URLSearchParams` compatibility layer.

Parsing/canonicalization is not an application's trust policy. Apps must still
validate schemes, credentials, path, query/fragment and exact expected origins
for their own protocols. Failures use generic errors rather than echoing input,
which may contain a private invitation or token.
