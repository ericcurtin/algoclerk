# AlgoClerk for macOS

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
macos/package-macos.sh aarch64-apple-darwin     # -> dist/AlgoClerk-<version>-macos-aarch64.zip
```

The app is not signed by default; macOS Gatekeeper will ask you to confirm the first start
(right-click, Open). To sign and notarise release builds set `ALGOCLERK_SIGN_IDENTITY` and use
`xcrun notarytool` (see `docs/BUILDING.md`). Rendering uses wgpu through Metal.
