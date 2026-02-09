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
  selectedIndex: number;
}
