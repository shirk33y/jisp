#!/usr/bin/env -S denon run -A --import-map import_map.json --unstable

import * as esbuild from "https://deno.land/x/esbuild@v0.14.1/mod.js";
// const ts = 'let test: boolean = true'
// const ts = 'import * as react from "https://esm.sh/react?dev";'
// const result = await esbuild.transform(ts, { loader: 'ts' })
// console.log('result:', result)
// esbuild.stop()

// const { build } = require('esbuild')

// let envPlugin = plugin => {
//   plugin.setName('env-plugin')
//   plugin.addResolver({ filter: /^env$/ }, args => {
//     return { path: 'env', namespace: 'env-plugin' }
//   })
//   plugin.addLoader({ filter: /^env$/, namespace: 'env-plugin' }, args => {
//     return { contents: JSON.stringify(process.env), loader: 'json' }
//   })
// }

const result = await esbuild
  .build({
    entryPoints: ["entry.js"],
    bundle: true,
    outfile: "out.js",
    // plugins: [
    //   envPlugin,
    // ],
  })
  .catch(() => Deno.exit(1));

console.log("result:", result);
esbuild.stop();
