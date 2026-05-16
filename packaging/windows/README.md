# Windows Packaging Notes

Current Windows packaging is a CPack ZIP skeleton with executable icon/version resources.

Required before public release:

1. Build with Qt 6 MSVC and `CROSSSCP_BUILD_GUI=ON`.
2. Run `windeployqt` for Qt runtime deployment.
3. Choose installer format: WiX MSI, MSIX, or NSIS.
4. Configure Authenticode signing.
5. Validate Windows Credential Manager / DPAPI integration once implemented.
6. Confirm high-DPI and screen-reader behavior.

The product icon source is:

```text
apps/crossscp-gui/resources/icons/crossscp.ico
```
