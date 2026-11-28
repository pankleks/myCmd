# Changelog

## [0.2.20] - 2026-11-28

### Fixes

- A broken `config.json` no longer wipes all settings: it is kept as `config.json.corrupt`, the app says so at startup, and settings are written safely to disk.
- Windows: the command line and Ctrl+T run in the panel folder instead of `C:\Windows`; network (UNC) folders are refused with a clear message.
- Copying to USB sticks (FAT/exFAT) and network drives no longer fails when the target cannot keep file attributes or permissions.
- Rename and move work on NFS, sshfs and SMB mounts.
- Large searches no longer freeze the window; content search stops reading a file at the first match.
- Extracting archives no longer fills `/tmp` and writes the data twice.
- SFTP: servers known in `~/.ssh/known_hosts` by another key type are no longer blocked as "key changed"; reconnecting cannot freeze other SFTP actions; uploaded folders keep their permissions instead of becoming private; password connections reopen as password.
- Moving to the Trash, renaming and cross-drive moves no longer walk whole folder trees first; the progress bar is correct.
- A stalled network drive no longer freezes the window.
- macOS update command: the old app is removed only after the new copy succeeded.

### Interface

- The delete dialog names the items it deletes and says Recycle Bin on Windows, Trash elsewhere.
- Dialog buttons, fields and lists are clearly visible; one focus ring and one button style everywhere; Settings matches the other dialogs.
- F-keys, arrows, Tab and Esc keep working while typing a command.
- The conflict dialog: Enter renames, Overwrite is marked dangerous, and "apply to all" says what it does.
- A failed rename or new folder reopens its dialog with the typed name.
- Rename selects the name without its extension.
- F3 on a folder has a Close button; F3 works from the SFTP context menu.
- Empty folders and searches say so; search progress stays visible in the other panel.
- Hidden files are dimmed; pinned folders can be unpinned from the dropdown; saved SFTP connections can be forgotten.
- Function keys with Ctrl/Alt no longer run the plain action; PageUp/PageDown respect row spacing; the cursor stays in place after a delete.
- Markdown previews cannot add links or imitate app dialogs.
- Consistent wording ("folder", "pinned folder") and an accurate shortcuts list.

## [0.2.19] - 2026-11-27

- F3 View on an image: Up/Down (also Left/Right, Home/End) shows the previous or next image in the folder, and the panel cursor follows. Press F3 again for a thumbnail grid of all images in the folder; Enter opens the selected one.
- Settings → Updates shows the installed version and asks GitHub for a newer release. **View…** shows the release notes and a ready-to-copy update command for this installation (DEB, AppImage, Windows installer, macOS). Nothing is downloaded or installed automatically.

## [0.2.17] - 2026-11-22

- Remember the sort order per folder, including after a restart.
- Fix the file row spacing setting resetting to 0 after a restart.

## [0.2.16] - 2026-11-22

- Add a configurable file row spacing (0–20 px) in Settings → Appearance.
- Tell the user when ZIP compression skips symbolic links.
- Render Markdown previews in a time-limited worker so hostile files cannot freeze the app.

## [0.2.15] - 2026-11-03

- Add offline HEIC and HEIF image previews in F3 View and the preview pane.
- Keep file extensions when auto-renaming conflicting copies (`report (1).pdf`), locally and over SFTP.
- SFTP: remote folders containing unusable file names (non-Unicode or containing `\`) stay browsable; copy and delete name those files instead of failing silently.
- SFTP: refreshing a connection during a transfer reports "busy" instead of freezing the panel; reconnecting cannot replace a session mid-transfer.
- SFTP: a slow server no longer delays connecting to other servers.
- ZIP/7z extraction is no longer blocked by unsafe member names outside the selection.

## [0.2.14] - 2026-11-02

- Fix a Windows-only Clippy warning that blocked the 0.2.13 release.
- Include the SVG previews, SFTP previews, and remote favorites from 0.2.13.

## [0.2.13] - 2026-11-02

Not published because Windows Clippy failed.

- Add isolated SVG image previews.
- Enable F3 View and the preview pane for SFTP files, including text, Markdown, images, PDFs, and directory summaries, with existing preview size limits.
- Allow pinning SFTP folders as favorites and reconnecting saved connections when opening them.

## [0.2.12] - 2026-10-31

- Add custom names for pinned directories, with inline renaming and search by name or path.
- Add Alt+F5 ZIP compression to the opposite panel, with editable archive names, progress, cancellation, and overwrite confirmation. Symbolic links are skipped.
- Improve Markdown preview width and Mermaid diagram label readability.
- Fix ZIP test path comparisons on Windows and macOS, which blocked the 0.2.11 release.

## [0.2.11] - 2026-10-31

Not published because cross-platform ZIP tests failed.

- Add custom names for pinned directories, with inline renaming and search by name or path.
- Add Alt+F5 ZIP compression to the opposite panel, with editable archive names, progress, cancellation, and overwrite confirmation. Symbolic links are skipped.
- Improve Markdown preview width and Mermaid diagram label readability.

## [0.2.10] - 2026-10-25

- Render fenced Mermaid diagrams in Markdown previews, with isolated SVG images that fit the available width and readable source fallbacks for invalid diagrams.
- Update compatible npm dependencies and override Monaco's DOMPurify dependency with the patched version, clearing all reported npm audit vulnerabilities.

## [0.2.9] - 2026-10-18

- Fix case-only file and folder renames on Windows, such as `pgdev` to `pgDEV`, while preserving overwrite protection. Case-only renames on case-insensitive macOS filesystems remain unsupported.
- Fit PDF previews to the preview panel width by default.
- Sort pinned directories alphabetically in the location dropdown.
- Remove unused code and share cancellation and text-decoding logic.

## [0.2.8] - 2026-10-16

- Fix location dropdown entries not responding to clicks in WebKit-based builds by deciding whether focus left the picker from the focus target instead of the WebKit-reported active element.

## [0.2.7] - 2026-10-13

- Fix F5/F6 destinations containing only a filename to use the source file's directory instead of the application's working directory.
- Give F2 renames the same conflict resolution options as F5/F6: Skip, Rename, Overwrite, and Cancel.
- Fix location dropdown keyboard navigation after mouse input, including Linux/WebKit focus handling and visible selection.
- Use the Settings checkbox styling throughout the app.

## [0.2.6] - 2026-10-09

- Add highlighted search and match navigation in rendered Markdown previews.
- Enable search for code, text, JSON, and Markdown source views.
- Add optional recursive text-content search to the Alt+F7 dialog, supporting UTF-8 and UTF-16 files while skipping binary files.
- Allow F3 and the preview pane to view results while a search is still running.
- Standardize UI icons on Iconify with bundled Lucide icons.
