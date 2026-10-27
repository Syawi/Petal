# README media

The screenshots and GIF use Petal's native Win32/GDI+ renderer with the synthetic filename sample.jpg. They contain no user's desktop or personal files. The GIF previews rendering and transitions; it is not a recording of a conversion on disk.

Recreate them on Windows from the repository root:

```powershell
cargo test --locked transparent_native_render_at_multiple_dpi
cargo test --locked export_documentation_frames -- --ignored
python -m pip install Pillow
python tools/render_readme.py
```

Frames are written to target/ and are not tracked. Only the optimized README media in docs/media/ is committed. Windows system Segoe UI supplies the rasterized presentation text; no font file is redistributed.
