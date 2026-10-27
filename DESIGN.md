# Design

## Surface
Windows 11 native utility, Operate mode. Six rounded petals surround the central Windows menu button. No separate lower button, numeric badges or replacement footer.

## Rendering and typography
GDI+ antialiasing on a premultiplied ARGB layered window. Transparent outer area, thin outlines and a soft downward shadow. System Segoe UI Bold at weight 700. Format labels: 21 logical units; center label: 17.5; filename: 12. Physical positions and font sizes are rounded to whole pixels. Native GDI ClearType draws opaque text on the final DIB; GDI+ Bold grayscale text handles translucent entrance frames. Original alpha is preserved.

The design canvas is 400 by 400 units, displayed at 75% (300 by 300 pixels at 100% Windows scaling). Geometry and hit targets follow the same scale and DPI.

## Theme and hover
Follow Windows AppsUseLightTheme and the system accent color. Dark surfaces: #292929/#242424; light surfaces: #FAFAFA/#F3F3F3. At rest, text follows the light/dark theme. Hover fills the entire petal with the exact Windows accent; the label becomes black. The center uses the same full accent background and black icon, title and filename. Preserve accent contrast for loading and success graphics.

Each hovered petal moves outward by 10 logical units over 200 ms, with an interruptible cubic transition. Keep its resting hit area active to prevent pointer flicker. Gaps remain a constant 8 logical units at every radius.

## Entrance and feedback
The center is available immediately. Petals appear in order with a 60 ms offset and a 300 ms transition each, taking 600 ms overall. Respect the Windows reduced-motion setting.

Choosing a format hides the petals. A 60-unit disk holds a 44-unit spinner, then a check drawn over 240 ms. Working feedback lasts at least 450 ms; successful feedback remains for 1200 ms. No simulated percentage. The Paid sound plays once with the check if the menu is visible and sound is enabled. Mute sound is saved per user. Outside clicks and Escape hide the UI without cancelling an active conversion.

A native high-resolution timer drives moving frames and stops while idle or hidden. Conversions run off the UI thread.

## Controls
Shift + right-click opens Petal by default; unmodified right-click preserves the Windows menu. Settings can capture any non-modifier key or mouse button with optional Ctrl/Shift/Alt/Win modifiers. Keyboard shortcuts target the file under the pointer while Explorer or the desktop is foreground. M opens that menu from the center. Escape closes; Tab navigates; Enter/Space activates; 1-6 invoke the corresponding format. Numeric shortcuts remain functional without visible badges.

## Windows notifications and identity
Use the blue ribbon mark from assets/petal.svg. The executable embeds a nine-size ICO via build.rs. The Windows notification identity is Petal.Converter, registered per user with a branded Start-menu shortcut and icon metadata. Modern notifications include the logo explicitly using appLogoOverride. Successful notifications say Conversion complete followed by the output filename. Notification audio is always silent; the existing check sound is controlled separately. Classic custom-icon balloons are retained only as an API-failure fallback.

## Settings
Native modeless Windows 11 panel, same blue mark, Segoe UI hierarchy, system theme and accent. Three groups: shortcut capture, launch at sign-in, completion audio. Rounded neutral surfaces, accent hover and toggles, clear native keyboard focus. Changes save immediately; Escape cancels capture. Custom WAV selection uses the Windows file picker and falls back to the embedded sound if the chosen file becomes unavailable.
