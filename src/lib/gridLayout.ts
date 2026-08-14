/**
 * A tmux-style binary split tree describing how a Workspace's visible
 * Sessions tile the Grid. A leaf holds one Session; a split divides its
 * space between two children along `direction`, at `fraction` (the `a`
 * child's share, 0..1) — freely resizable by dragging the handle between
 * them, unlike the earlier fixed-cell dashboard layout.
 */

export interface LeafNode {
  type: "leaf";
  sessionId: string;
}

export interface SplitNode {
  type: "split";
  id: string;
  direction: "row" | "column";
  fraction: number;
  a: LayoutNode;
  b: LayoutNode;
}

export type LayoutNode = LeafNode | SplitNode;

let nextSplitId = 1;

export function leaf(sessionId: string): LeafNode {
  return { type: "leaf", sessionId };
}

export function collectLeafIds(node: LayoutNode, out: string[] = []): string[] {
  if (node.type === "leaf") {
    out.push(node.sessionId);
  } else {
    collectLeafIds(node.a, out);
    collectLeafIds(node.b, out);
  }
  return out;
}

export function sameIdSet(a: string[], b: string[]): boolean {
  if (a.length !== b.length) return false;
  const set = new Set(a);
  return b.every((id) => set.has(id));
}

/** A balanced binary split tree over `ids`, alternating row/column direction
 * per depth so a handful of Sessions tile roughly square rather than as one
 * long strip. Returns `null` for an empty Workspace. */
export function buildBalancedLayout(ids: string[], direction: "row" | "column" = "row"): LayoutNode | null {
  if (ids.length === 0) return null;
  if (ids.length === 1) return leaf(ids[0]);
  const mid = Math.ceil(ids.length / 2);
  const nextDirection = direction === "row" ? "column" : "row";
  return {
    type: "split",
    id: `split-${nextSplitId++}`,
    direction,
    fraction: 0.5,
    a: buildBalancedLayout(ids.slice(0, mid), nextDirection) as LayoutNode,
    b: buildBalancedLayout(ids.slice(mid), nextDirection) as LayoutNode,
  };
}

export function setFraction(node: LayoutNode, splitId: string, fraction: number): LayoutNode {
  if (node.type === "leaf") return node;
  if (node.id === splitId) {
    return { ...node, fraction: Math.min(0.85, Math.max(0.15, fraction)) };
  }
  return { ...node, a: setFraction(node.a, splitId, fraction), b: setFraction(node.b, splitId, fraction) };
}

export function swapLeaves(node: LayoutNode, sessionA: string, sessionB: string): LayoutNode {
  if (node.type === "leaf") {
    if (node.sessionId === sessionA) return leaf(sessionB);
    if (node.sessionId === sessionB) return leaf(sessionA);
    return node;
  }
  return { ...node, a: swapLeaves(node.a, sessionA, sessionB), b: swapLeaves(node.b, sessionA, sessionB) };
}
