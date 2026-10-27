# Verification

## Automated coverage
The Rust suite covers image, audio/video and document conversion, output verification, collisions including publication races, preservation of invalid inputs, six-format menus, animation geometry, native typography/alpha, native icon resources, tray-menu mute state, and the real Windows message loop.

Notification tests verify that the XML contains an explicit appLogoOverride image, escapes file names and uses silent notification audio. File-URI tests cover spaces, reserved characters and Unicode installation paths.

## Build and manual checks
Run Build.ps1, or cargo test --locked and cargo build --release --locked. The interactive desktop test is intentionally ignored inside the build sandbox. Native render snapshots are written to target/visual-checks.

Petal.exe --test-notification sends a harmless Conversion complete sample without converting any file. Its exit code reports whether Windows accepted the branded notification. Windows notification and Do Not Disturb settings may suppress banners independently of the application.

Image and media round trips, independent PDF parsing and rendering, and DOCX/ODT interoperability have been checked during development. Windows loads the executable icon at 16, 24, 32, 48 and 256 pixels. The visible-success mute path is exercised by the native message-loop test.

The final branded Windows notification was inspected on screen: the blue logo appears in both the sender header and the app-logo area beside Conversion complete. The notification diagnostic exited successfully and the per-user Start-menu shortcut and identity registration were checked. Sound-setting persistence through the actual tray UI has not yet been checked manually.

Current suite result: 37 tests passed; two opt-in tasks are ignored by default (the interactive desktop check and README-frame export). The frame exporter has also been run explicitly and its output inspected. Build and media-install scripts pass the PowerShell syntax parser. Authored source, script and documentation files were scanned for remaining legacy names and non-English application text.

## Data handling
Use copies for exploratory conversion tests. The original is permanently removed after a verified conversion. Metadata, image profiles, document layout and media attachments are not guaranteed to survive. Existing output files are never overwritten.
