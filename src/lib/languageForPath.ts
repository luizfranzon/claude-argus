const EXTENSION_LANGUAGE: Record<string, string> = {
  ts: "typescript",
  tsx: "typescript",
  js: "javascript",
  jsx: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  json: "json",
  jsonc: "jsonc",
  html: "html",
  css: "css",
  scss: "scss",
  less: "less",
  md: "markdown",
  mdx: "markdown",
  rs: "rust",
  py: "python",
  go: "go",
  rb: "ruby",
  java: "java",
  c: "c",
  h: "c",
  cpp: "cpp",
  hpp: "cpp",
  cs: "csharp",
  php: "php",
  sh: "shell",
  bash: "shell",
  ps1: "powershell",
  sql: "sql",
  yaml: "yaml",
  yml: "yaml",
  toml: "toml",
  xml: "xml",
  svg: "xml",
  dockerfile: "dockerfile",
  txt: "plaintext",
};

export function languageForPath(path: string): string {
  const normalized = path.replace(/\\/g, "/");
  const name = normalized.slice(normalized.lastIndexOf("/") + 1).toLowerCase();
  if (name === "dockerfile") return "dockerfile";
  const ext = name.includes(".") ? name.slice(name.lastIndexOf(".") + 1) : "";
  return EXTENSION_LANGUAGE[ext] ?? "plaintext";
}
