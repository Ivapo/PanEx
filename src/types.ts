export interface FileEntry {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
  modified: number;
}

export interface PaneState {
  id: string;
  currentPath: string;
  entries: FileEntry[];
  selectedPaths: Set<string>;
  lastClickedPath: string | null;
  expandedPaths: Set<string>;
  childrenCache: Map<string, FileEntry[]>;
  focusIndex: number;
  searchQuery: string;
}

export type SplitDirection = "horizontal" | "vertical";
// "vertical" = left|right children, "horizontal" = top/bottom children

export interface LayoutLeaf {
  type: "leaf";
  paneId: string;
}

export interface LayoutSplit {
  type: "split";
  direction: SplitDirection;
  first: LayoutNode;
  second: LayoutNode;
  ratio: number;
}

export type LayoutNode = LayoutLeaf | LayoutSplit;

export type SortField = 'name' | 'size' | 'modified' | 'type';
export type SortDirection = 'asc' | 'desc';
