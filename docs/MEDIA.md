# README media

Every image and the GIF are drawn by Petal's own Win32/GDI+ renderer, over a synthetic wallpaper of soft colour glows that is generated in code. They never contain a real desktop or personal files. The demo previews rendering and transitions; it is not a recording of a conversion on disk.

Recreate them on Windows from the repository root:

```powershell
cargo test --locked export_readme_media -- --ignored
```

This writes straight into `docs/media/`:

| File | Content |
| --- | --- |
| `hero.png` | The liquid glass wheel with one lit petal, beside the pitch |
| `demo.gif` | Bubble reveal, glowing hover, conversion spinner and check |
| `themes.png` | Liquid glass and Classic, in dark and light |
| `quick-settings.png` | The glass quick panel from the tray, dark and light |

The GIF stores only the rectangle that changes in each frame, with unchanged pixels left transparent, which keeps it small. Windows system Segoe UI supplies the text; no font file is redistributed.

`settings.png` is a capture of the real Settings window. Start `Petal.exe --settings` with default settings, then capture only that window, for example with `PrintWindow`, so nothing behind it is included.
