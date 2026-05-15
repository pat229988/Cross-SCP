# macOS Packaging Notes

Current macOS packaging is a CPack DragNDrop skeleton.

Required before public release:

1. Build with Qt 6 and `CROSSSCP_BUILD_GUI=ON`.
2. Run `macdeployqt` on the generated `.app` bundle.
3. Add original `.icns` generation from provided PNG/SVG assets.
4. Configure Developer ID signing.
5. Notarize and staple the DMG/app.
6. Verify accessibility metadata and high-DPI rendering.

The bundle identifier is currently:

```text
org.crossscp.CrossSCP
```
