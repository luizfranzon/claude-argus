import { create } from "zustand";

export interface FeatureGroup {
  id: string;
  workspaceId: string;
  name: string;
  color: string;
}

/** Built-in Session Filter values alongside real Feature Group ids. */
export const ALL_FILTER = "all";
export const UNGROUPED_FILTER = "ungrouped";

let nextGroupId = 1;

interface FeatureGroupStoreState {
  groups: Record<string, FeatureGroup>;
  groupsByWorkspace: Record<string, string[]>;
  /** sessionId -> groupId, or absent/UNGROUPED_FILTER for ungrouped. */
  sessionGroup: Record<string, string>;
  /** workspaceId -> ALL_FILTER | UNGROUPED_FILTER | groupId, defaults to "all". */
  sessionFilter: Record<string, string>;
  expanded: Record<string, boolean>;
  pendingCreateFeatureGroupWorkspaceId: string | null;

  openCreateFeatureGroup: (workspaceId: string) => void;
  cancelCreateFeatureGroup: () => void;
  createFeatureGroup: (name: string, color: string) => void;
  deleteFeatureGroup: (id: string) => void;
  renameFeatureGroup: (id: string, name: string) => void;
  setFeatureGroupColor: (id: string, color: string) => void;
  assignSessionToGroup: (sessionId: string, groupId: string) => void;
  removeSessionFromGroups: (sessionId: string) => void;
  setSessionFilter: (workspaceId: string, filter: string) => void;
  toggleGroupExpanded: (groupId: string) => void;
  removeGroupsForWorkspace: (workspaceId: string) => void;
}

export function groupFor(state: Pick<FeatureGroupStoreState, "sessionGroup">, sessionId: string): string {
  return state.sessionGroup[sessionId] ?? UNGROUPED_FILTER;
}

export const useFeatureGroupStore = create<FeatureGroupStoreState>((set, get) => ({
  groups: {},
  groupsByWorkspace: {},
  sessionGroup: {},
  sessionFilter: {},
  expanded: {},
  pendingCreateFeatureGroupWorkspaceId: null,

  openCreateFeatureGroup: (workspaceId: string) => set({ pendingCreateFeatureGroupWorkspaceId: workspaceId }),
  cancelCreateFeatureGroup: () => set({ pendingCreateFeatureGroupWorkspaceId: null }),

  createFeatureGroup: (name: string, color: string) => {
    const workspaceId = get().pendingCreateFeatureGroupWorkspaceId;
    if (!workspaceId) return;
    const id = `group-${nextGroupId++}`;
    set((state) => ({
      groups: { ...state.groups, [id]: { id, workspaceId, name, color } },
      groupsByWorkspace: {
        ...state.groupsByWorkspace,
        [workspaceId]: [...(state.groupsByWorkspace[workspaceId] ?? []), id],
      },
      expanded: { ...state.expanded, [id]: true },
      pendingCreateFeatureGroupWorkspaceId: null,
    }));
  },

  deleteFeatureGroup: (id: string) => {
    set((state) => {
      const group = state.groups[id];
      if (!group) return {};
      const { [id]: _removed, ...groups } = state.groups;
      const { [id]: _removedExpanded, ...expanded } = state.expanded;
      const groupsByWorkspace = {
        ...state.groupsByWorkspace,
        [group.workspaceId]: (state.groupsByWorkspace[group.workspaceId] ?? []).filter((gid) => gid !== id),
      };
      // Sessions in the deleted group fall back to "ungrouped" rather than
      // being closed — a Feature Group is only an organizational bucket.
      const sessionGroup = { ...state.sessionGroup };
      for (const [sessionId, groupId] of Object.entries(sessionGroup)) {
        if (groupId === id) delete sessionGroup[sessionId];
      }
      const sessionFilter = { ...state.sessionFilter };
      if (sessionFilter[group.workspaceId] === id) {
        sessionFilter[group.workspaceId] = ALL_FILTER;
      }
      return { groups, groupsByWorkspace, expanded, sessionGroup, sessionFilter };
    });
  },

  renameFeatureGroup: (id: string, name: string) =>
    set((state) => {
      const group = state.groups[id];
      if (!group) return {};
      return { groups: { ...state.groups, [id]: { ...group, name } } };
    }),

  setFeatureGroupColor: (id: string, color: string) =>
    set((state) => {
      const group = state.groups[id];
      if (!group) return {};
      return { groups: { ...state.groups, [id]: { ...group, color } } };
    }),

  assignSessionToGroup: (sessionId: string, groupId: string) =>
    set((state) => {
      if (groupId === UNGROUPED_FILTER) {
        const { [sessionId]: _removed, ...sessionGroup } = state.sessionGroup;
        return { sessionGroup };
      }
      return { sessionGroup: { ...state.sessionGroup, [sessionId]: groupId } };
    }),

  removeSessionFromGroups: (sessionId: string) =>
    set((state) => {
      const { [sessionId]: _removed, ...sessionGroup } = state.sessionGroup;
      return { sessionGroup };
    }),

  setSessionFilter: (workspaceId: string, filter: string) =>
    set((state) => ({ sessionFilter: { ...state.sessionFilter, [workspaceId]: filter } })),

  toggleGroupExpanded: (groupId: string) =>
    set((state) => ({ expanded: { ...state.expanded, [groupId]: !state.expanded[groupId] } })),

  removeGroupsForWorkspace: (workspaceId: string) =>
    set((state) => {
      const ids = state.groupsByWorkspace[workspaceId] ?? [];
      const groups = { ...state.groups };
      const expanded = { ...state.expanded };
      for (const id of ids) {
        delete groups[id];
        delete expanded[id];
      }
      const { [workspaceId]: _removedGroups, ...groupsByWorkspace } = state.groupsByWorkspace;
      const { [workspaceId]: _removedFilter, ...sessionFilter } = state.sessionFilter;
      return { groups, expanded, groupsByWorkspace, sessionFilter };
    }),
}));
