// SPDX-License-Identifier: AGPL-3.0-or-later

import { build } from "esbuild";
import { createHash } from "node:crypto";
import { mkdir, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const editorRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const outputDirectory = join(editorRoot, "dist");
await rm(outputDirectory, { recursive: true, force: true });
await mkdir(outputDirectory, { recursive: true });

const result = await build({
  entryPoints: [join(editorRoot, "src/main.tsx")],
  bundle: true,
  minify: true,
  format: "esm",
  target: ["chrome120", "edge120", "firefox121", "safari17"],
  outdir: join(outputDirectory, "assets"),
  entryNames: "main-[hash]",
  assetNames: "asset-[hash]",
  metafile: true,
  legalComments: "eof",
});

const outputs = Object.keys(result.metafile.outputs).map((path) =>
  relative(outputDirectory, path).replaceAll("\\", "/"),
);
const script = outputs.find((path) => path.endsWith(".js"));
const stylesheet = outputs.find((path) => path.endsWith(".css"));
if (!script || !stylesheet) {
  throw new Error(`editor build did not emit JS and CSS: ${outputs.join(", ")}`);
}

const html = `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="color-scheme" content="dark">
    <meta name="description" content="An independent workspace for reliable personal automation.">
    <title>Canopy Workbench</title>
    <link rel="stylesheet" href="/${stylesheet}">
  </head>
  <body>
    <div id="app"></div>
    <noscript>Canopy Workbench needs JavaScript in this browser to open the editor.</noscript>
    <script type="module" src="/${script}"></script>
  </body>
</html>
`;
await writeFile(join(outputDirectory, "index.html"), html);
const manualContract = JSON.parse(
  await readFile(join(editorRoot, "../contracts/manual-trigger.v1alpha1.json"), "utf8"),
);
const contractBytes = JSON.stringify(canonical(manualContract));
await writeFile(
  join(outputDirectory, "catalog.v1.json"),
  JSON.stringify({
    schema: 1,
    nodes: [{
      display_name: manualContract.extensions["canopy.workbench/display"].display_name,
      description: manualContract.extensions["canopy.workbench/display"].description,
      contract_lock: {
        api_version: manualContract.identity.api_version,
        namespace: manualContract.identity.namespace,
        name: manualContract.identity.name,
        version: manualContract.identity.version,
        digest: `sha256:${createHash("sha256").update(contractBytes).digest("hex")}`,
      },
      configuration_schema: manualContract.configuration.schema,
      editor_hints: manualContract.configuration.editor_hints,
    }],
  }),
);

const files = await listFiles(outputDirectory);
const manifest = {
  schema: 1,
  generatedBy: "editor/scripts/build.mjs",
  files: await Promise.all(
    files.map(async (path) => {
      const bytes = await readFile(path);
      return {
        path: relative(outputDirectory, path).replaceAll("\\", "/"),
        bytes: bytes.length,
        sha256: createHash("sha256").update(bytes).digest("hex"),
      };
    }),
  ),
};
await writeFile(
  join(outputDirectory, "asset-manifest.json"),
  `${JSON.stringify(manifest, null, 2)}\n`,
);

async function listFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const paths = [];
  for (const entry of entries) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) paths.push(...(await listFiles(path)));
    else if (entry.isFile()) paths.push(path);
  }
  return paths.sort();
}

function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(Object.keys(value).sort().map((key) => [key, canonical(value[key])]));
  }
  return value;
}
