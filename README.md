# HKXC Annotation Editor

A desktop GUI application for editing Havok HKX animation annotations for Skyrim and other Bethesda games.

## Features

- **Drag & Drop** - Drop HKX files or folders directly onto the window
- **Multi-file Editing** - Open and edit multiple animations simultaneously
- **Keyboard Shortcuts** - Ctrl+S to save, Ctrl+Shift+S to save all
- **Format Support** - 64-bit (SE/AE) and 32-bit (LE) formats
- **Auto Cleanup** - Annotation files are automatically cleaned up when tabs are closed

## Requirements

- **Windows 10+** with WebView2 (pre-installed on Windows 11)
- **hkxc-anno-cli.exe** - Must be in the same folder as the application

## Quick Start

1. **Place files together:**
   ```
   YourFolder/
   ├── hkxc-anno-gui.exe
   ├── hkxc-anno-cli.exe
   └── hkxc.exe
   ```

2. **Launch the app:**
   - Double-click `hkxc-anno-gui.exe`

3. **Open files:**
   - Click "Open File(s)" or "Open Folder"
   - Or drag & drop HKX files/folders onto the window

4. **Edit annotations:**
   - Modify annotation timing and events in the editor
   - Modified tabs show a dot (•) indicator

5. **Save changes:**
   - Press `Ctrl+S` to save the current file
   - Press `Ctrl+Shift+S` to save all modified files
   - Select format: 64-bit (SE/AE) or 32-bit (LE)

## Workflow

```
1. Dump → Opens HKX files and extracts annotations
2. Edit → Modify annotations in the Monaco editor
3. Save → Updates the original HKX files with your changes
4. Close → Annotation files are automatically cleaned up
```

## File Structure

When you dump files, annotation `.txt` files are created next to your HKX files:

```
Your_Mod/
├── animations/
│   ├── attack.hkx         ← Original animation
│   ├── attack.txt         ← Annotation file (temporary)
│   ├── block.hkx
│   └── block.txt          ← Annotation file (temporary)
```

**Note:** Annotation files are temporary and are deleted when you close tabs or the app. Your changes are saved to the `.hkx` files when you press Ctrl+S.

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+S` | Save current file |
| `Ctrl+Shift+S` | Save all modified files |
| `Ctrl+F` | Find in file |
| `Ctrl+H` | Find & replace |
