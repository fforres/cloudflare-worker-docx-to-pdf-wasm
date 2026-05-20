# opt-2 worker — fonts at runtime

For local dev we import the four required fonts as bundled `Data` assets so
no Cloudflare account is needed. In production you would store the same TTFs
in an R2 bucket and fetch them lazily on the first request.

## Dev (this folder)

```bash
npm install
npx wrangler dev --port 8789 --ip 127.0.0.1
# in another shell
curl --data-binary @../../../fixtures/tier1/test.docx \
     -H "content-type: application/octet-stream" \
     http://127.0.0.1:8789/convert -o out.pdf
```

The `[[rules]] type = "Data"` block in `wrangler.toml` makes `import x from "./y.ttf"`
yield an `ArrayBuffer` so we can encode it into the mini-format buffer the WASM
expects.

## Production swap-in (R2)

Step 1 — replace the static font imports in `src/worker.js` with R2 lookups:

```js
// wrangler.toml
// [[r2_buckets]]
// binding = "FONTS"
// bucket_name = "wasm-docx-fonts"

let FONTS_BUFFER = null;

async function getFontsBuffer(env) {
  if (FONTS_BUFFER) return FONTS_BUFFER;
  const names = [
    ["Carlito", "Carlito-Regular.ttf"],
    ["Carlito", "Carlito-Bold.ttf"],
    ["Liberation Serif", "LiberationSerif-Regular.ttf"],
    ["Liberation Serif", "LiberationSerif-Bold.ttf"],
  ];
  const fetched = await Promise.all(
    names.map(async ([family, key]) => {
      const obj = await env.FONTS.get(key);
      if (!obj) throw new Error(`font missing in R2: ${key}`);
      return [family, await obj.arrayBuffer()];
    }),
  );
  FONTS_BUFFER = buildFontsBuffer(fetched);
  return FONTS_BUFFER;
}

export default {
  async fetch(req, env) {
    const fontsBuffer = await getFontsBuffer(env);
    // ... rest of handler ...
  },
};
```

Step 2 — upload the 4 TTFs to R2 once:

```bash
npx wrangler r2 bucket create wasm-docx-fonts
for f in Carlito-Regular.ttf Carlito-Bold.ttf \
         LiberationSerif-Regular.ttf LiberationSerif-Bold.ttf; do
  npx wrangler r2 object put "wasm-docx-fonts/$f" --file "src/fonts/$f"
done
```

R2 reads are cached by the Worker runtime so warm requests stay fast.
The WASM still ships at ~0.65 MiB gzipped (no fonts bundled in the binary).
