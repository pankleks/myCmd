# Changelog

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
