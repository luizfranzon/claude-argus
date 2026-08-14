import * as monaco from "monaco-editor";

import { languageForPath } from "./languageForPath";
import * as tauriApi from "./tauri";

/**
 * One `ITextModel` per open file path, shared across whichever Monaco editor
 * instance is currently showing it. Lives outside React/Zustand state
 * deliberately — models aren't serializable/comparable the way plain state
 * needs to be, so the store only tracks lightweight tab metadata (dirty,
 * conflict) and defers to this module for the actual buffer.
 */
const models = new Map<string, monaco.editor.ITextModel>();
const diskContent = new Map<string, string>();

export async function getOrLoadModel(path: string): Promise<monaco.editor.ITextModel> {
  const existing = models.get(path);
  if (existing) return existing;
  const content = await tauriApi.readFile(path);
  diskContent.set(path, content);
  const model = monaco.editor.createModel(content, languageForPath(path), monaco.Uri.file(path));
  models.set(path, model);
  return model;
}

export function disposeModel(path: string) {
  models.get(path)?.dispose();
  models.delete(path);
  diskContent.delete(path);
}

export function isDirty(path: string): boolean {
  const model = models.get(path);
  const baseline = diskContent.get(path);
  if (!model || baseline === undefined) return false;
  return model.getValue() !== baseline;
}

export async function saveModel(path: string): Promise<void> {
  const model = models.get(path);
  if (!model) return;
  const content = model.getValue();
  await tauriApi.writeFile(path, content);
  diskContent.set(path, content);
}

export type ExternalChangeResult = "unchanged" | "reloaded" | "conflict";

/**
 * Compares the model against what's actually on disk right now. A clean
 * model reloads silently; a dirty one is left untouched and reported as a
 * conflict for the UI to resolve — see the External edit conflict entry in
 * CONTEXT.md.
 */
export async function checkExternalChange(path: string): Promise<ExternalChangeResult> {
  const model = models.get(path);
  if (!model) return "unchanged";
  const before = diskContent.get(path);
  let onDisk: string;
  try {
    onDisk = await tauriApi.readFile(path);
  } catch {
    return "unchanged";
  }
  if (onDisk === before) return "unchanged";

  if (!isDirty(path)) {
    model.setValue(onDisk);
    diskContent.set(path, onDisk);
    return "reloaded";
  }
  return "conflict";
}

export function conflictResolveKeepMine(path: string) {
  const model = models.get(path);
  if (model) diskContent.set(path, model.getValue());
}

export async function conflictResolveReload(path: string) {
  const model = models.get(path);
  if (!model) return;
  const onDisk = await tauriApi.readFile(path);
  model.setValue(onDisk);
  diskContent.set(path, onDisk);
}
