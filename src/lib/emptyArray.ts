/**
 * Stable empty-array reference for Zustand selectors. `state.foo[key] ?? []`
 * allocates a new array every render, which breaks `useSyncExternalStore`'s
 * reference-equality check and causes an infinite "Maximum update depth
 * exceeded" render loop — use this instead wherever a selector needs a
 * default for a missing key.
 */
export const EMPTY_ARRAY: readonly never[] = [];
