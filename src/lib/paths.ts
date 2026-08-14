function separatorFor(path: string): string {
  return path.includes("\\") && !path.includes("/") ? "\\" : "/";
}

export function joinPath(dir: string, name: string): string {
  const sep = separatorFor(dir);
  return dir.endsWith(sep) ? `${dir}${name}` : `${dir}${sep}${name}`;
}

export function dirname(path: string): string {
  const sep = separatorFor(path);
  const idx = path.lastIndexOf(sep);
  return idx <= 0 ? path : path.slice(0, idx);
}

export function basename(path: string): string {
  const normalized = path.replace(/\\/g, "/").replace(/\/+$/, "");
  const parts = normalized.split("/");
  return parts[parts.length - 1] || path;
}

/** Path relative to `root`, using forward slashes regardless of platform. */
export function relativeTo(root: string, path: string): string {
  const normalizedRoot = root.replace(/\\/g, "/").replace(/\/+$/, "");
  const normalizedPath = path.replace(/\\/g, "/");
  if (normalizedPath === normalizedRoot) return "";
  if (normalizedPath.startsWith(normalizedRoot + "/")) {
    return normalizedPath.slice(normalizedRoot.length + 1);
  }
  return normalizedPath;
}
