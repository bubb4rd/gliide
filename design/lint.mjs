#!/usr/bin/env node
// Design-system lint: keeps every screen on the tokens and the better-ui rules.
// Usage: node design/lint.mjs [paths…]   (default: design/ and app/src/ when present)
// Exit code 1 on any finding. No dependencies.

import { readFileSync, readdirSync, statSync, existsSync } from "node:fs";
import { join, relative, basename, extname } from "node:path";

const ROOT = new URL("..", import.meta.url).pathname;
const TOKENS = join(ROOT, "design/tokens.css");
const EXTS = new Set([".css", ".svelte", ".html"]);
const SKIP_DIRS = new Set(["node_modules", "dist", "build", "target", ".svelte-kit", "fonts"]);

const findings = [];
const report = (file, line, rule, text) =>
  findings.push(`${relative(ROOT, file)}:${line}  ${rule}  ${text.trim().slice(0, 120)}`);

function walk(path, out) {
  const st = statSync(path);
  if (st.isDirectory()) {
    if (SKIP_DIRS.has(basename(path))) return;
    for (const name of readdirSync(path)) walk(join(path, name), out);
  } else if (EXTS.has(extname(path))) out.push(path);
}

// CSS sources inside a file, with the line each starts on.
function cssChunks(file, src) {
  if (extname(file) === ".css") return [{ css: src, line: 1 }];
  const chunks = [];
  for (const m of src.matchAll(/<style[^>]*>([\s\S]*?)<\/style>/g)) {
    chunks.push({ css: m[1], line: src.slice(0, m.index + m[0].indexOf(m[1])).split("\n").length });
  }
  for (const m of src.matchAll(/\sstyle="([^"]*)"/g)) {
    chunks.push({ css: `x{${m[1]}}`, line: src.slice(0, m.index).split("\n").length, inline: true });
  }
  return chunks;
}

// Walks rules: calls visit(context[], declarationText, lineNo) for every declaration.
function walkCss(css, startLine, visit) {
  const clean = css.replace(/\/\*[\s\S]*?\*\//g, (c) => c.replace(/[^\n]/g, " "));
  const stack = [];
  let buf = "";
  let line = startLine;
  let bufLine = startLine;
  for (const ch of clean) {
    if (ch === "{") {
      stack.push(buf.trim());
      buf = "";
    } else if (ch === "}") {
      if (buf.trim()) visit(stack, buf, bufLine);
      buf = "";
      stack.pop();
    } else if (ch === ";") {
      visit(stack, buf, bufLine);
      buf = "";
    } else {
      if (!buf.trim() && ch.trim()) bufLine = line;
      buf += ch;
    }
    if (ch === "\n") line++;
  }
}

const RAW_COLOR = /#[0-9a-fA-F]{3,8}\b|\b(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch|color)\((?!\s*from\s+var\()/;
const RAW_FONT = /^font-family\s*:(?!\s*(?:var\(|inherit|initial))/;
const TRANSITION_ALL = /^transition(?:-property)?\s*:\s*(?:all\b|[^;]*\ball\s+\d)/;

function lintFile(file) {
  const src = readFileSync(file, "utf8");
  const isTokens = file === TOKENS;

  // Icon SVGs must take color from CSS.
  if (extname(file) !== ".css") {
    src.split("\n").forEach((l, i) => {
      if (/<(?:path|circle|rect|line|polyline|polygon|svg|g)\b[^>]*\b(?:fill|stroke)="#/.test(l))
        report(file, i + 1, "icon-color", l);
    });
  }

  for (const { css, line } of cssChunks(file, src)) {
    walkCss(css, line, (ctx, decl, n) => {
      const d = decl.trim();
      if (!d.includes(":")) return;
      const selector = ctx[ctx.length - 1] ?? "";
      const atRules = ctx.filter((c) => c.startsWith("@"));
      const isProp = d.startsWith("--");

      if (!isTokens) {
        if (RAW_COLOR.test(d) && !isProp) report(file, n, "raw-color (use a tokens.css variable)", d);
        if (isProp && RAW_COLOR.test(d)) report(file, n, "color token outside tokens.css", d);
        if (RAW_FONT.test(d)) report(file, n, "raw-font (use --font-ui / --font-display / --font-mono)", d);
        if (/^(?:box-shadow)\s*:/.test(d) && !/var\(|none|inset 0 0 0 1px var\(/.test(d))
          report(file, n, "raw-shadow (use --shadow-*)", d);
        if (/^(?:animation-timing-function|transition-timing-function)\s*:\s*cubic-bezier/.test(d))
          report(file, n, "raw-easing (use --ease-*)", d);
      }
      if (TRANSITION_ALL.test(d)) report(file, n, "transition-all (name the properties)", d);
      if (/^will-change\s*:\s*all/.test(d)) report(file, n, "will-change-all", d);

      const scale = d.match(/^scale\s*:\s*(0?\.\d+)/) ?? d.match(/scale\((0?\.\d+)/);
      if (scale && /:active/.test(selector) && parseFloat(scale[1]) < 0.96)
        report(file, n, "press-scale below 0.96", `${selector} { ${d} }`);
      if (/^scale\s*:/.test(d) && /:active/.test(selector) && !/:not\(:disabled\)|:enabled/.test(selector))
        report(file, n, "press-scale without :disabled exclusion", `${selector} { ${d} }`);
      if (/^scale\s*:/.test(d) && /:active/.test(selector) &&
          !atRules.some((a) => /prefers-reduced-motion:\s*no-preference/.test(a)))
        report(file, n, "press-scale outside @media (prefers-reduced-motion: no-preference)", `${selector} { ${d} }`);

      if (/:hover/.test(selector) && !atRules.some((a) => /\(hover:\s*hover\)/.test(a)))
        report(file, n, "hover outside @media (hover: hover)", selector);

      if (/^(?:translate|filter|scale)\s*:/.test(d) || /^animation\s*:/.test(d)) {
        const animatesMotion = /^animation\s*:/.test(d) && !/\bnone\b|\bfade\b/.test(d);
        if (animatesMotion && !atRules.some((a) => /prefers-reduced-motion/.test(a)))
          report(file, n, "motion without prefers-reduced-motion gate", d);
      }
    });
  }
}

// Every token the dark theme overrides must exist in the light :root (catches typos that silently fall back).
function lintDarkTokens() {
  const src = readFileSync(TOKENS, "utf8");
  const light = new Set();
  const dark = [];
  walkCss(src, 1, (ctx, decl, n) => {
    const prop = decl.trim().split(":")[0].trim();
    if (!prop.startsWith("--")) return;
    const sel = ctx[ctx.length - 1] ?? "";
    if (sel === ":root") light.add(prop);
    else if (/data-theme="dark"/.test(sel)) dark.push([prop, n]);
  });
  for (const [prop, n] of dark)
    if (!light.has(prop)) findings.push(`design/tokens.css:${n}  dark-token-unknown  ${prop} is not defined in :root`);
}

const targets = process.argv.slice(2).length
  ? process.argv.slice(2).map((p) => join(process.cwd(), p))
  : [join(ROOT, "design"), join(ROOT, "app/src")].filter(existsSync);
const files = [];
for (const t of targets) walk(t, files);
files.forEach(lintFile);
lintDarkTokens();

if (findings.length) {
  console.log(findings.join("\n"));
  console.log(`\n${findings.length} finding(s) in ${files.length} file(s).`);
  process.exit(1);
}
console.log(`design lint: ${files.length} file(s), no findings.`);
