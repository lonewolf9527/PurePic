# PurePic Repository Instructions

## Git workflow

- This repository uses Git, with <code>main</code> as the primary branch.
- The entire <code>Doc/</code> directory is local design material and must never be tracked by Git.
- Keep <code>/Doc/</code> in the root <code>.gitignore</code>. Do not use force-add to bypass this rule.
- Every completed batch of source-code, test, build, or configuration changes must be committed before starting another independent code change or handing work back to the user.
- Before editing, inspect <code>git status</code>. Preserve unrelated user changes and stage only files belonging to the current batch.
- Run the relevant formatting, build, and tests before each code commit. If verification cannot run, state the reason in the commit handoff.
- Use concise, descriptive commit messages. Do not amend, squash, reset, or rewrite existing commits unless the user explicitly asks.
- Do not commit generated build outputs, local IDE settings, logs, caches, binaries, or secrets.

## Project direction

- PurePic targets Windows 11 x64.
- The implementation uses Rust, windows-rs, Win32, WIC, Direct2D, D3D11, and DXGI.
- Optimize the time to first visible image. Directory enumeration and thumbnail generation must not block the first image.
- Keep Windows 11 window behavior, Per-Monitor V2 DPI support, bounded queues, and byte-budgeted image caches.

