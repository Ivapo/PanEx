import type { FileEntry } from "./types.ts";

export interface FsBackend {
  readDir(path: string): Promise<FileEntry[]>;
  getHomeDir(): Promise<string>;
  getParentDir(path: string): Promise<string>;
  openEntry(path: string): Promise<void>;
  renameEntry(path: string, newName: string): Promise<void>;
  deleteEntry(path: string, permanent?: boolean): Promise<void>;
  copyEntry(source: string, destDir: string): Promise<string>;
  moveEntry(source: string, destDir: string): Promise<string>;
  getDirSize(path: string): Promise<number>;
  createFile(dir: string, name: string): Promise<void>;
  createFolder(dir: string, name: string): Promise<void>;
  openInTerminal(path: string): Promise<void>;
}

function createTauriFs(): FsBackend {
  // Lazy import so @tauri-apps/api is only loaded in Tauri context
  let invokeRef: typeof import("@tauri-apps/api/core")["invoke"] | null = null;

  async function getInvoke() {
    if (!invokeRef) {
      const mod = await import("@tauri-apps/api/core");
      invokeRef = mod.invoke;
    }
    return invokeRef;
  }

  return {
    async readDir(path: string): Promise<FileEntry[]> {
      const invoke = await getInvoke();
      return invoke<FileEntry[]>("read_dir", { path });
    },
    async getHomeDir(): Promise<string> {
      const invoke = await getInvoke();
      return invoke<string>("get_home_dir");
    },
    async getParentDir(path: string): Promise<string> {
      const invoke = await getInvoke();
      return invoke<string>("get_parent_dir", { path });
    },
    async openEntry(path: string): Promise<void> {
      const invoke = await getInvoke();
      await invoke("open_entry", { path });
    },
    async renameEntry(path: string, newName: string): Promise<void> {
      const invoke = await getInvoke();
      await invoke("rename_entry", { path, newName });
    },
    async deleteEntry(path: string, permanent?: boolean): Promise<void> {
      const invoke = await getInvoke();
      await invoke("delete_entry", { path, permanent: permanent ?? false });
    },
    async copyEntry(source: string, destDir: string): Promise<string> {
      const invoke = await getInvoke();
      return invoke<string>("copy_entry", { source, destDir });
    },
    async moveEntry(source: string, destDir: string): Promise<string> {
      const invoke = await getInvoke();
      return invoke<string>("move_entry", { source, destDir });
    },
    async getDirSize(path: string): Promise<number> {
      const invoke = await getInvoke();
      return invoke<number>("calculate_dir_size", { path });
    },
    async createFile(dir: string, name: string): Promise<void> {
      const invoke = await getInvoke();
      await invoke("create_file", { dir, name });
    },
    async createFolder(dir: string, name: string): Promise<void> {
      const invoke = await getInvoke();
      await invoke("create_folder", { dir, name });
    },
    async openInTerminal(path: string): Promise<void> {
      const invoke = await getInvoke();
      await invoke("open_in_terminal", { path });
    },
  };
}

function createBrowserFs(): FsBackend {
  // Cache of path -> FileSystemDirectoryHandle
  const handleCache = new Map<string, FileSystemDirectoryHandle>();
  let rootHandle: FileSystemDirectoryHandle | null = null;
  let rootName = "";

  async function pickRoot(): Promise<FileSystemDirectoryHandle> {
    const handle = await window.showDirectoryPicker({ mode: "readwrite" });
    rootHandle = handle;
    rootName = handle.name;
    const rootPath = "/" + rootName;
    handleCache.set(rootPath, handle);
    return handle;
  }

  async function ensureRoot(): Promise<FileSystemDirectoryHandle> {
    if (rootHandle) return rootHandle;
    return pickRoot();
  }

  function getRootPath(): string {
    return "/" + rootName;
  }

  // Resolve a virtual path like "/rootName/sub/dir" to its directory handle
  async function resolveDir(path: string): Promise<FileSystemDirectoryHandle> {
    const cached = handleCache.get(path);
    if (cached) return cached;

    const root = await ensureRoot();
    const rootPath = getRootPath();

    if (path === rootPath) return root;

    // Strip root prefix: "/rootName/sub/dir" -> "sub/dir"
    const relative = path.slice(rootPath.length + 1);
    if (!relative) return root;

    const segments = relative.split("/").filter(Boolean);
    let current: FileSystemDirectoryHandle = root;
    let currentPath = rootPath;

    for (const segment of segments) {
      current = await current.getDirectoryHandle(segment);
      currentPath += "/" + segment;
      handleCache.set(currentPath, current);
    }

    return current;
  }

  // Get the parent handle and child name from a full path
  function splitPath(path: string): { parentPath: string; name: string } {
    const lastSlash = path.lastIndexOf("/");
    if (lastSlash <= 0) return { parentPath: "/", name: path.slice(1) };
    return {
      parentPath: path.slice(0, lastSlash),
      name: path.slice(lastSlash + 1),
    };
  }

  return {
    async readDir(path: string): Promise<FileEntry[]> {
      const dirHandle = await resolveDir(path);
      const entries: FileEntry[] = [];

      for await (const [name, handle] of dirHandle.entries()) {
        if (handle.kind === "file") {
          try {
            const file = await (handle as FileSystemFileHandle).getFile();
            entries.push({
              name,
              path: path + "/" + name,
              is_dir: false,
              size: file.size,
              modified: file.lastModified,
            });
          } catch {
            entries.push({
              name,
              path: path + "/" + name,
              is_dir: false,
              size: 0,
              modified: 0,
            });
          }
        } else {
          entries.push({
            name,
            path: path + "/" + name,
            is_dir: true,
            size: 0,
            modified: 0,
          });
          // Cache the directory handle
          handleCache.set(path + "/" + name, handle as FileSystemDirectoryHandle);
        }
      }

      // Sort: dirs first, then alphabetical
      entries.sort((a, b) => {
        if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
        return a.name.localeCompare(b.name);
      });

      return entries;
    },

    async getHomeDir(): Promise<string> {
      const root = await ensureRoot();
      return "/" + root.name;
    },

    async getParentDir(path: string): Promise<string> {
      const rootPath = getRootPath();
      if (path === rootPath || path.length <= rootPath.length) {
        // Already at root — can't go higher, prompt for new root
        return rootPath;
      }
      const { parentPath } = splitPath(path);
      return parentPath;
    },

    async openEntry(path: string): Promise<void> {
      // In browser mode, download the file
      const { parentPath, name } = splitPath(path);
      const parentHandle = await resolveDir(parentPath);

      try {
        const fileHandle = await parentHandle.getFileHandle(name);
        const file = await fileHandle.getFile();
        const url = URL.createObjectURL(file);
        window.open(url, "_blank");
        // Clean up after a delay
        setTimeout(() => URL.revokeObjectURL(url), 60000);
      } catch {
        // Might be a directory — no-op
      }
    },

    async renameEntry(path: string, newName: string): Promise<void> {
      const { parentPath, name } = splitPath(path);
      const parentHandle = await resolveDir(parentPath);

      // Try the move() method first (Chrome 86+)
      try {
        const handle = await parentHandle.getFileHandle(name).catch(() =>
          parentHandle.getDirectoryHandle(name)
        );
        if ("move" in handle && typeof (handle as any).move === "function") {
          await (handle as any).move(newName);
          // Update cache if it was a directory
          const oldPath = path;
          const newPath = parentPath + "/" + newName;
          const cachedHandle = handleCache.get(oldPath);
          if (cachedHandle) {
            handleCache.delete(oldPath);
            handleCache.set(newPath, cachedHandle);
          }
          return;
        }
      } catch {
        // fall through to manual copy
      }

      // Fallback: copy + delete for files
      try {
        const fileHandle = await parentHandle.getFileHandle(name);
        const file = await fileHandle.getFile();
        const newFileHandle = await parentHandle.getFileHandle(newName, { create: true });
        const writable = await newFileHandle.createWritable();
        await writable.write(file);
        await writable.close();
        await parentHandle.removeEntry(name);
      } catch {
        throw new Error(`Cannot rename "${name}" to "${newName}"`);
      }
    },

    async deleteEntry(path: string, _permanent?: boolean): Promise<void> {
      const { parentPath, name } = splitPath(path);
      const parentHandle = await resolveDir(parentPath);
      await parentHandle.removeEntry(name, { recursive: true });
      // Clean up handle cache
      for (const key of handleCache.keys()) {
        if (key === path || key.startsWith(path + "/")) {
          handleCache.delete(key);
        }
      }
    },

    async copyEntry(source: string, destDir: string): Promise<string> {
      const { parentPath: srcParentPath, name: srcName } = splitPath(source);
      const srcParent = await resolveDir(srcParentPath);
      const destParent = await resolveDir(destDir);
      const destPath = destDir + "/" + srcName;

      // Check if source is a file
      try {
        const srcFileHandle = await srcParent.getFileHandle(srcName);
        const file = await srcFileHandle.getFile();
        const destFileHandle = await destParent.getFileHandle(srcName, { create: true });
        const writable = await destFileHandle.createWritable();
        await writable.write(file);
        await writable.close();
        return destPath;
      } catch {
        // Source is a directory — recursive copy
        await copyDirRecursive(srcParent, srcName, destParent);
        return destPath;
      }
    },

    async moveEntry(source: string, destDir: string): Promise<string> {
      const { name: srcName } = splitPath(source);
      const destPath = destDir + "/" + srcName;

      // Copy then delete
      await this.copyEntry(source, destDir);
      await this.deleteEntry(source);
      return destPath;
    },

    async createFile(dir: string, name: string): Promise<void> {
      const dirHandle = await resolveDir(dir);
      await dirHandle.getFileHandle(name, { create: true });
    },

    async createFolder(dir: string, name: string): Promise<void> {
      const dirHandle = await resolveDir(dir);
      await dirHandle.getDirectoryHandle(name, { create: true });
    },

    async openInTerminal(_path: string): Promise<void> {
      alert("Open in Terminal is not available in browser mode.");
    },

    async getDirSize(path: string): Promise<number> {
      async function walkHandle(handle: FileSystemDirectoryHandle): Promise<number> {
        let total = 0;
        for await (const [, child] of handle.entries()) {
          if (child.kind === "file") {
            try {
              const file = await (child as FileSystemFileHandle).getFile();
              total += file.size;
            } catch { /* skip */ }
          } else {
            total += await walkHandle(child as FileSystemDirectoryHandle);
          }
        }
        return total;
      }
      const dirHandle = await resolveDir(path);
      return walkHandle(dirHandle);
    },
  };
}

async function copyDirRecursive(
  srcParent: FileSystemDirectoryHandle,
  name: string,
  destParent: FileSystemDirectoryHandle
): Promise<void> {
  const srcDir = await srcParent.getDirectoryHandle(name);
  const destDir = await destParent.getDirectoryHandle(name, { create: true });

  for await (const [childName, childHandle] of srcDir.entries()) {
    if (childHandle.kind === "file") {
      const file = await (childHandle as FileSystemFileHandle).getFile();
      const destFile = await destDir.getFileHandle(childName, { create: true });
      const writable = await destFile.createWritable();
      await writable.write(file);
      await writable.close();
    } else {
      await copyDirRecursive(srcDir, childName, destDir);
    }
  }
}

function isTauri(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

export function isBrowser(): boolean {
  return !isTauri();
}

export const fs: FsBackend = isTauri() ? createTauriFs() : createBrowserFs();
