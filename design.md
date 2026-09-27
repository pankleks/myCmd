# Dual-Panel File Manager — MVP Design

## 1. Purpose

The goal is to build a lightweight, cross-platform, dual-panel file manager inspired by Total Commander.

The application should prioritize:

- fast keyboard-driven navigation,
- a simple two-panel layout,
- predictable file operations,
- minimal UI complexity,
- native filesystem access,
- support for Windows, Linux, and macOS.

The MVP is intentionally limited to core local filesystem operations.

---

## 2. Technology Stack

### Desktop shell

- **Tauri 2**

Tauri provides the native application shell, packaging, IPC between frontend and backend, window management, and cross-platform distribution.

### Frontend

- **Svelte 5**
- **TypeScript**
- **Vite**

Svelte is responsible only for UI state, rendering, dialogs, keyboard interaction, and communication with the Tauri backend.

SvelteKit is not required for the MVP.

### Backend

- **Rust**
- `std::fs`, `std::path`
- `tokio` where asynchronous/background filesystem work is needed

All real filesystem operations must be performed by Rust.

---

## 3. High-Level Architecture

```text
┌────────────────────────────────────────────────────────────┐
│                    Svelte 5 Frontend                       │
│                                                            │
│  ┌────────────────────┐    ┌────────────────────┐          │
│  │     Left Panel     │    │    Right Panel     │          │
│  │                    │    │                    │          │
│  │ Path Bar           │    │ Path Bar           │          │
│  │ File List          │    │ File List          │          │
│  │ Status Bar         │    │ Status Bar         │          │
│  └────────────────────┘    └────────────────────┘          │
│                                                            │
│  F2 Rename | F5 Copy | F6 Move | F7 MkDir | F8 Delete     │
└─────────────────────────────┬──────────────────────────────┘
                              │
                        Tauri invoke/events
                              │
                              ▼
┌────────────────────────────────────────────────────────────┐
│                       Rust Backend                         │
│                                                            │
│  Directory listing                                        │
│  Copy                                                     │
│  Move                                                     │
│  Rename                                                   │
│  Create directory                                         │
│  Delete                                                   │
│  File operation progress                                  │
│  Conflict detection                                       │
│                                                            │
└─────────────────────────────┬──────────────────────────────┘
                              │
                              ▼
                      Native filesystem
```

The frontend must not directly implement filesystem logic.

---

## 4. MVP Scope

The MVP must support:

- two independent filesystem panels,
- navigation through directories,
- parent directory navigation,
- opening directories,
- opening files using the operating system default application,
- switching between active panels,
- selecting one or multiple files/directories,
- copy,
- move,
- rename,
- delete,
- recursive directory operations,
- create new directory,
- sorting,
- optional display of hidden files,
- basic file conflict handling,
- keyboard-first navigation,
- basic operation progress,
- Windows, Linux, and macOS.

### Explicitly out of scope for MVP

Do not implement:

- FTP,
- SFTP,
- ZIP archive browsing,
- tabs,
- bookmarks,
- favorites,
- directory trees,
- file preview,
- internal text editor,
- plugins,
- cloud storage,
- directory synchronization,
- directory comparison,
- integrated terminal,
- checksum tools,
- advanced search,
- network discovery,
- undo history.

These may be added in later versions.

---

## 5. Main Window

The main application window should be simple and dense.

```text
┌─────────────────────────────────────────────────────────────┐
│ C:\Projects                         D:\Backup               │
├─────────────────────────────┬───────────────────────────────┤
│ Name          Size   Date   │ Name           Size    Date   │
│ ─────────────────────────── │ ───────────────────────────── │
│ [..]                        │ [..]                          │
│ 📁 src                      │ 📁 Backup                     │
│ 📁 docs                     │ 📁 Photos                     │
│ package.json   3 KB         │ archive.zip    840 MB         │
│ README.md      8 KB         │                               │
│                             │                               │
│                             │                               │
├─────────────────────────────┴───────────────────────────────┤
│ F2 Rename │ F5 Copy │ F6 Move │ F7 MkDir │ F8 Delete      │
└─────────────────────────────────────────────────────────────┘
```

There should be no permanent sidebar in the MVP.

Both panels should occupy most of the application window.

---

## 6. Panel Design

Each panel is an independent reusable component.

```text
FilePanel
├── PathBar
├── FileList
└── PanelStatus
```

Each panel stores:

- current directory,
- current file list,
- cursor position,
- selected items,
- sorting configuration,
- hidden-files visibility,
- loading/error state.

---

## 7. Path Bar

Each panel has a path bar at the top.

Example on Windows:

```text
[C: ▼]  C:\Users\User\Projects\
```

Example on Linux:

```text
[/ ▼]  /home/user/projects
```

Example on macOS:

```text
[/ ▼]  /Users/user/projects
```

Requirements:

- current path is visible,
- path is editable,
- pressing Enter loads the typed path,
- invalid paths show a non-blocking error,
- clicking a root selector allows switching to another root/mount.

### Windows roots

The application should detect available drives:

```text
C:
D:
E:
```

UNC paths must be supported by the backend:

```text
\\server\share
```

### Linux roots

Typical roots and mounts:

```text
/
~
/mnt/...
/media/...
```

### macOS roots

Typical locations:

```text
/
~
/Volumes/...
```

The backend should provide filesystem roots/mount points in a platform-specific implementation.

---

## 8. File List

Minimal columns:

- Name
- Extension
- Size
- Modified

Directories should normally appear before files.

The synthetic parent entry:

```text
[..]
```

must appear at the top of every directory except filesystem root.

### Sorting

The MVP must support sorting by:

- name,
- extension,
- size,
- modified time.

Each column can toggle ascending/descending order.

Directory-first sorting should remain enabled regardless of selected sort column.

---

## 9. File Entry Model

Frontend representation:

```ts
export interface FileEntry {
    name: string;
    path: string;

    type: "file" | "directory" | "symlink";

    extension?: string;

    size: number;

    modified?: number;
    created?: number;

    hidden: boolean;

    readonly?: boolean;
}
```

The backend should use native Rust path types internally.

Do not treat frontend string paths as the authoritative path representation inside the backend.

Use `PathBuf` internally.

---

## 10. Panel State

Example state model:

```ts
export interface PanelState {
    path: string;

    entries: FileEntry[];

    cursor: number;

    selected: Set<string>;

    sort: {
        column: "name" | "extension" | "size" | "modified";
        direction: "asc" | "desc";
    };

    showHidden: boolean;

    loading: boolean;

    error?: string;
}
```

Global application state:

```ts
export interface CommanderState {
    activePanel: "left" | "right";

    left: PanelState;
    right: PanelState;
}
```

No external state framework is required for the MVP.

Use Svelte 5 state/runes.

---

## 11. Active Panel

Exactly one panel is active at a time.

The active panel should be visually distinguishable using a subtle border/background accent.

The active panel determines:

- which item receives keyboard cursor movement,
- where new directories are created,
- which items are sources for copy/move/delete,
- which panel receives F2/F7/F8 actions.

The inactive panel is usually the destination for F5/F6 operations.

---

## 12. Keyboard Navigation

The application must be fully usable without a mouse.

### Navigation

```text
Up / Down     Move cursor
Enter         Open directory or file
Backspace     Go to parent directory
Tab           Switch active panel
Home          First item
End           Last item
Page Up       Move one page up
Page Down     Move one page down
```

### Selection

```text
Space         Toggle current item selection
Insert        Toggle current item selection
Ctrl+A        Select all
Esc           Clear selection / close dialog
```

### File operations

```text
F2            Rename
F5            Copy
F6            Move
F7            Create directory
F8            Delete
Delete        Delete
```

Possible later additions:

```text
F3            View
F4            Edit
```

These are not required for the MVP.

---

## 13. Mouse Interaction

The mouse should supplement, not replace, keyboard navigation.

Required:

- single click selects/cursors an item,
- Ctrl+click toggles selection,
- Shift+click selects a range,
- double click opens a directory or file,
- clicking another panel activates it,
- column headers change sorting.

Context menus are optional and not required for MVP.

---

## 14. File Selection

Selection is separate from keyboard cursor.

Example:

```text
> file1.txt          current cursor
* file2.txt          selected
* folder1            selected
```

Recommended frontend representation:

```ts
selected: Set<string>
```

File operations should use:

1. selected items, if at least one item is selected,
2. otherwise the item under the cursor.

This matches common dual-panel file manager behavior.

---

## 15. Copy

Shortcut:

```text
F5
```

Source:

- selected items from active panel.

Destination:

- current directory of inactive panel.

Before starting the operation, show a small confirmation dialog.

Example:

```text
┌────────────────────────────────────────┐
│ Copy                                   │
│                                        │
│ 4 selected items                       │
│                                        │
│ Destination:                           │
│ D:\Backup                              │
│                                        │
│                  Cancel   Copy         │
└────────────────────────────────────────┘
```

The destination path should be editable.

Copy must support:

- files,
- directories,
- mixed selections,
- recursive directory copy.

---

## 16. Move

Shortcut:

```text
F6
```

The default destination is the inactive panel's current directory.

Example:

```text
┌────────────────────────────────────────┐
│ Move                                   │
│                                        │
│ 3 selected items                       │
│                                        │
│ Destination:                           │
│ D:\Archive                             │
│                                        │
│                  Cancel   Move         │
└────────────────────────────────────────┘
```

Move should use native filesystem rename/move when possible.

If the source and destination are on different filesystems, the backend may fall back to:

```text
copy
+
delete source
```

---

## 17. Rename

Shortcut:

```text
F2
```

Only one item is renamed at a time in MVP.

Example:

```text
┌────────────────────────────────────────┐
│ Rename                                 │
│                                        │
│ old-file-name.txt                      │
│ [new-file-name.txt________________]    │
│                                        │
│                Cancel   Rename         │
└────────────────────────────────────────┘
```

Rename should preserve the current directory.

After rename:

- refresh the active panel,
- position cursor on the renamed item.

---

## 18. Create Directory

Shortcut:

```text
F7
```

Example:

```text
┌────────────────────────────────────────┐
│ New directory                          │
│                                        │
│ Name                                   │
│ [new-folder________________________]    │
│                                        │
│                Cancel   Create         │
└────────────────────────────────────────┘
```

After creation:

- refresh the panel,
- move cursor to the new directory.

---

## 19. Delete

Shortcut:

```text
F8
```

Also support:

```text
Delete
```

MVP behavior:

- delete selected files/directories,
- directories are deleted recursively,
- always show confirmation.

Example:

```text
┌────────────────────────────────────────┐
│ Delete                                 │
│                                        │
│ Delete 12 items?                       │
│                                        │
│ 8 files                                │
│ 4 directories                          │
│                                        │
│ Directories will be deleted            │
│ recursively.                           │
│                                        │
│                Cancel   Delete         │
└────────────────────────────────────────┘
```

Trash/recycle-bin support may be added later.

For the first MVP, permanent delete is acceptable if the confirmation dialog is explicit.

---

## 20. File Conflicts

Copy and move operations must handle destination conflicts.

Required options:

- Skip
- Rename
- Overwrite
- Cancel

Example:

```text
┌──────────────────────────────────────────────┐
│ File already exists                         │
│                                              │
│ file.txt                                     │
│                                              │
│ Source       4.2 MB   2026-09-27 12:32      │
│ Destination  3.8 MB   2026-09-25 08:17      │
│                                              │
│ □ Apply to all                               │
│                                              │
│ Skip   Rename   Overwrite   Cancel           │
└──────────────────────────────────────────────┘
```

Conflict handling must be implemented in the backend operation layer rather than duplicated in UI logic.

---

## 21. File Operations

File operations should not be implemented as direct button-to-filesystem calls.

Use a common operation abstraction.

```ts
export type FileOperation =
    | CopyOperation
    | MoveOperation
    | DeleteOperation
    | RenameOperation
    | CreateDirectoryOperation;
```

Example:

```ts
export interface CopyOperation {
    type: "copy";

    sources: string[];

    destination: string;
}
```

This makes future support easier for:

- progress,
- cancel,
- operation queue,
- retries,
- conflict handling,
- operation history.

---

## 22. Operation Progress

Large file operations must not block the UI.

Example status:

```text
Copying 4 files... 37%
```

The backend should assign an operation ID.

Example:

```ts
export interface FileOperationProgress {
    operationId: string;

    state:
        | "queued"
        | "running"
        | "completed"
        | "failed"
        | "cancelled";

    currentItem?: string;

    processedBytes: number;
    totalBytes?: number;

    processedItems: number;
    totalItems?: number;

    error?: string;
}
```

The frontend receives progress events from Tauri.

A full operation queue UI is not required for MVP.

A simple bottom status/progress area is sufficient.

---

## 23. Backend API

Suggested frontend-facing API:

```ts
export interface FileSystemApi {
    list(path: string): Promise<FileEntry[]>;

    roots(): Promise<FileSystemRoot[]>;

    copy(
        sources: string[],
        destination: string
    ): Promise<FileOperationHandle>;

    move(
        sources: string[],
        destination: string
    ): Promise<FileOperationHandle>;

    rename(
        path: string,
        newName: string
    ): Promise<void>;

    remove(
        paths: string[]
    ): Promise<FileOperationHandle>;

    createDirectory(
        parent: string,
        name: string
    ): Promise<void>;

    openFile(
        path: string
    ): Promise<void>;
}
```

Additional models:

```ts
export interface FileSystemRoot {
    name: string;
    path: string;
    type: "drive" | "mount" | "home" | "root";
}

export interface FileOperationHandle {
    operationId: string;
}
```

---

## 24. Tauri Commands

Suggested commands:

```text
list_directory
list_roots
copy_files
move_files
rename_file
delete_files
create_directory
open_file
cancel_operation
```

Example:

```rust
#[tauri::command]
async fn list_directory(
    path: String
) -> Result<Vec<FileEntryDto>, String> {
    filesystem::list(path.into())
        .await
        .map_err(|e| e.to_string())
}
```

Tauri command functions should only expose the backend API.

They should not contain filesystem business logic.

---

## 25. Rust Backend Structure

Suggested structure:

```text
src-tauri/
└── src/
    ├── main.rs
    │
    ├── commands/
    │   ├── mod.rs
    │   └── filesystem.rs
    │
    ├── filesystem/
    │   ├── mod.rs
    │   ├── list.rs
    │   ├── roots.rs
    │   ├── copy.rs
    │   ├── move.rs
    │   ├── rename.rs
    │   ├── delete.rs
    │   ├── mkdir.rs
    │   └── open.rs
    │
    ├── operations/
    │   ├── mod.rs
    │   ├── manager.rs
    │   ├── progress.rs
    │   └── conflict.rs
    │
    ├── platform/
    │   ├── mod.rs
    │   ├── windows.rs
    │   ├── linux.rs
    │   └── macos.rs
    │
    └── error.rs
```

---

## 26. Frontend Project Structure

Suggested frontend structure:

```text
src/
├── App.svelte
│
├── components/
│   ├── Commander.svelte
│   │
│   ├── FilePanel/
│   │   ├── FilePanel.svelte
│   │   ├── PathBar.svelte
│   │   ├── FileList.svelte
│   │   ├── FileRow.svelte
│   │   └── PanelStatus.svelte
│   │
│   ├── dialogs/
│   │   ├── CopyDialog.svelte
│   │   ├── MoveDialog.svelte
│   │   ├── RenameDialog.svelte
│   │   ├── DeleteDialog.svelte
│   │   ├── NewDirectoryDialog.svelte
│   │   └── ConflictDialog.svelte
│   │
│   ├── FunctionBar.svelte
│   └── OperationStatus.svelte
│
├── filesystem/
│   ├── api.ts
│   └── types.ts
│
├── operations/
│   ├── operations.ts
│   └── types.ts
│
├── keyboard/
│   └── shortcuts.ts
│
├── state/
│   └── commander.svelte.ts
│
└── utils/
    ├── formatBytes.ts
    └── formatDate.ts
```

---

## 27. Example Frontend Filesystem API

```ts
import { invoke } from "@tauri-apps/api/core";
import type {
    FileEntry,
    FileOperationHandle,
    FileSystemRoot
} from "./types";

export function listDirectory(path: string) {
    return invoke<FileEntry[]>("list_directory", { path });
}

export function listRoots() {
    return invoke<FileSystemRoot[]>("list_roots");
}

export function copyFiles(
    sources: string[],
    destination: string
) {
    return invoke<FileOperationHandle>("copy_files", {
        sources,
        destination
    });
}

export function moveFiles(
    sources: string[],
    destination: string
) {
    return invoke<FileOperationHandle>("move_files", {
        sources,
        destination
    });
}

export function renameFile(
    path: string,
    newName: string
) {
    return invoke<void>("rename_file", {
        path,
        newName
    });
}

export function deleteFiles(paths: string[]) {
    return invoke<FileOperationHandle>("delete_files", {
        paths
    });
}

export function createDirectory(
    parent: string,
    name: string
) {
    return invoke<void>("create_directory", {
        parent,
        name
    });
}
```

---

## 28. Svelte State

Example using Svelte 5 runes:

```ts
import type { PanelState } from "../filesystem/types";

function createPanelState(): PanelState {
    return {
        path: "",
        entries: [],
        cursor: 0,
        selected: new Set<string>(),
        sort: {
            column: "name",
            direction: "asc"
        },
        showHidden: false,
        loading: false
    };
}

export const commander = $state({
    activePanel: "left" as "left" | "right",

    left: createPanelState(),

    right: createPanelState()
});
```

Avoid adding Redux-like libraries unless the application later becomes substantially more complex.

---

## 29. Platform-Specific Requirements

### Windows

Must account for:

- drive letters,
- UNC paths,
- NTFS permissions,
- read-only files,
- hidden/system files,
- path normalization,
- long paths.

### Linux

Must account for:

- Unix permissions,
- symbolic links,
- mount points,
- case-sensitive filenames,
- removable media.

### macOS

Must account for:

- Unix permissions,
- symbolic links,
- `/Volumes`,
- case-sensitive or case-insensitive filesystems depending on volume,
- application sandbox implications if distribution requirements change later.

---

## 30. Symbolic Links

The MVP must identify symbolic links correctly.

Default rules:

- list symlinks as a separate `type`,
- opening a symlink follows the operating system behavior,
- copying a symlink should copy the link itself unless platform behavior requires otherwise,
- recursive directory traversal must avoid accidental symlink loops.

Detailed symlink policies can be refined during implementation.

---

## 31. Errors

Filesystem errors should not crash the application.

Typical errors:

- permission denied,
- destination exists,
- source disappeared,
- invalid path,
- disk full,
- read-only filesystem,
- device disconnected,
- sharing violation,
- path too long.

Backend errors should be mapped to structured frontend errors.

Example:

```ts
export interface FileSystemError {
    code:
        | "not_found"
        | "permission_denied"
        | "already_exists"
        | "disk_full"
        | "read_only"
        | "invalid_path"
        | "io_error";

    message: string;

    path?: string;
}
```

---

## 32. Opening Files

Pressing Enter on a file or double-clicking it should open the file using the default operating system application.

This should be delegated to the backend/platform layer.

Examples:

```text
.pdf     → default PDF viewer
.png     → default image viewer
.txt     → default text editor
```

No internal viewer is required in MVP.

---

## 33. Hidden Files

The MVP should provide a simple toggle:

```text
Show hidden files
```

Possible shortcut:

```text
Ctrl+H
```

Exact hidden-file detection is platform-specific and belongs in the backend.

---

## 34. Startup Behavior

At startup:

1. determine user's home directory,
2. load home directory in left panel,
3. load home directory in right panel,
4. activate left panel.

Future versions may restore the last opened paths.

Persisting previous session state is optional for MVP.

---

## 35. UI Principles

The application should feel like a utility, not a content application.

Prefer:

- dense layout,
- small spacing,
- clear typography,
- minimal animation,
- strong keyboard focus indication,
- subtle active-panel highlight,
- simple native-looking dialogs,
- fast response.

Avoid:

- oversized buttons,
- cards,
- large empty margins,
- mobile-style navigation,
- permanent sidebars,
- decorative animations.

---

## 36. Window Sizing

Suggested minimum size:

```text
900 × 500
```

Suggested default size:

```text
1200 × 700
```

Each panel should receive approximately 50% of available width.

A draggable divider between panels can be added either in MVP or immediately after MVP.

---

## 37. Performance Goals

The UI should remain responsive while:

- browsing directories with thousands of items,
- copying large files,
- recursively copying directories,
- deleting large directory trees.

Directory listing should not block the UI thread.

Rendering should use a strategy that remains responsive for large lists.

Virtualized lists are optional initially but should be considered if directories with tens of thousands of files cause problems.

---

## 38. Refresh Behavior

Panels should refresh automatically after operations affecting their current directories.

Examples:

- copy into inactive panel → refresh inactive panel,
- rename → refresh active panel,
- delete → refresh active panel,
- mkdir → refresh active panel,
- move → refresh both panels when necessary.

Manual refresh shortcut can be added:

```text
Ctrl+R
```

or:

```text
F5
```

However, because F5 is used for copy, `Ctrl+R` is preferred.

---

## 39. MVP Acceptance Criteria

The MVP is considered complete when all of the following work on Windows, Linux, and macOS:

### Navigation

- [ ] application starts successfully,
- [ ] both panels show a directory,
- [ ] user can enter a directory,
- [ ] user can navigate to parent,
- [ ] user can type a path,
- [ ] user can switch active panel using Tab,
- [ ] user can open a file using the default application.

### Selection

- [ ] keyboard cursor works,
- [ ] mouse selection works,
- [ ] multi-selection works,
- [ ] Ctrl+A works,
- [ ] Space/Insert toggles current item.

### Operations

- [ ] F2 renames a file,
- [ ] F2 renames a directory,
- [ ] F5 copies files,
- [ ] F5 recursively copies directories,
- [ ] F6 moves files,
- [ ] F6 moves directories,
- [ ] F7 creates directories,
- [ ] F8/Delete deletes files,
- [ ] F8/Delete recursively deletes directories.

### Safety

- [ ] delete requires confirmation,
- [ ] copy/move conflicts are handled,
- [ ] filesystem errors are shown to the user,
- [ ] application does not freeze during large operations.

### Platform

- [ ] Windows drives work,
- [ ] Linux roots/mounts work,
- [ ] macOS volumes work,
- [ ] path handling works correctly on each platform.

---

## 40. Suggested Implementation Order

### Phase 1 — Shell and navigation

1. Create Tauri + Svelte project.
2. Build two-panel layout.
3. Implement `list_directory`.
4. Add directory navigation.
5. Add active-panel switching.
6. Add keyboard cursor.

### Phase 2 — Selection

1. Single selection.
2. Multi-selection.
3. Ctrl+A.
4. Space/Insert selection.
5. Mouse selection.

### Phase 3 — Basic operations

1. Create directory.
2. Rename.
3. Delete.
4. Copy.
5. Move.

### Phase 4 — Reliability

1. Recursive operations.
2. Conflict handling.
3. Structured errors.
4. Progress reporting.
5. Background operations.

### Phase 5 — Cross-platform cleanup

1. Windows drive handling.
2. UNC paths.
3. Linux mount points.
4. macOS volumes.
5. symlink behavior.
6. permission errors.

---

## 41. Design Decisions

### Why Tauri

The application requires:

- native filesystem access,
- cross-platform packaging,
- small runtime overhead,
- native desktop integration.

Tauri provides a good separation between UI and native filesystem logic.

### Why Svelte

The UI is:

- stateful,
- component-based,
- relatively small,
- dominated by lists, selection, dialogs, and keyboard events.

Svelte provides a simple reactive model without the extra boilerplate of larger frontend frameworks.

### Why Rust owns filesystem logic

Filesystem behavior is the core of the application.

Keeping it in Rust provides:

- native path handling,
- explicit platform-specific code,
- efficient filesystem operations,
- easier asynchronous operations,
- clean frontend/backend separation.

The frontend should not become responsible for filesystem semantics.

---

## 42. Future Extensions

The architecture should allow later addition of:

```text
F3 View
F4 Edit
tabs
bookmarks
history
search
directory compare
sync
ZIP/archive browsing
SFTP
FTP
network shares
operation queue
pause/resume
undo
checksums
file preview
integrated terminal
plugins
```

None of these should complicate the MVP implementation.

---

## 43. Final MVP Definition

The first releasable version should feel like this:

> A fast, lightweight, two-panel local filesystem manager that can be fully operated with the keyboard and performs the essential file operations reliably on Windows, Linux, and macOS.

The defining features are:

```text
two panels
+
fast navigation
+
multi-selection
+
copy
+
move
+
rename
+
mkdir
+
delete
+
keyboard shortcuts
```

Everything else is secondary.
