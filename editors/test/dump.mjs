// editors/test/dump.mjs
//
// Print every token of a snippet or a file with the scopes the grammar put on
// it. This is the tool to reach for when a rule does not fire and it is not
// obvious why — TextMate picks the *leftmost* match and breaks a tie by the
// order of the patterns, and reading the token stream is the only reliable way
// to see which of the two decided.
//
//     node dump.mjs 'let x = f"{a:.1}"'
//     node dump.mjs @../../examples/1brc.nika

import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import oniguruma from "vscode-oniguruma";
import vsctm from "vscode-textmate";

const here = dirname(fileURLToPath(import.meta.url));
const grammarPath = resolve(here, "..", "vscode", "syntaxes", "nikaia.tmLanguage.json");

await oniguruma.loadWASM(
  readFileSync(join(here, "node_modules", "vscode-oniguruma", "release", "onig.wasm")),
);

const registry = new vsctm.Registry({
  onigLib: Promise.resolve({
    createOnigScanner: (s) => new oniguruma.OnigScanner(s),
    createOnigString: (s) => new oniguruma.OnigString(s),
  }),
  loadGrammar: async (scopeName) =>
    scopeName === "source.nika"
      ? vsctm.parseRawGrammar(readFileSync(grammarPath, "utf8"), grammarPath)
      : null,
});

const grammar = await registry.loadGrammar("source.nika");
const arg = process.argv[2] ?? "";
const src = arg.startsWith("@") ? readFileSync(arg.slice(1), "utf8") : arg;

let state = vsctm.INITIAL;
for (const line of src.split("\n")) {
  const result = grammar.tokenizeLine(line, state);
  for (const t of result.tokens) {
    const text = line.slice(t.startIndex, t.endIndex);
    if (text.trim().length === 0) continue;
    console.log(JSON.stringify(text).padEnd(28), t.scopes.join(" "));
  }
  state = result.ruleStack;
}
console.log(`-- ends at stack depth ${state.depth} (root is ${vsctm.INITIAL.depth})`);
