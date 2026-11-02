# AlgoClerk for Windows

```powershell
rustup target add x86_64-pc-windows-msvc
windows/package-windows.ps1     # -> dist/AlgoClerk-<version>-windows-x86_64.zip
```

The binary uses the GUI subsystem in release builds (no console window). Rendering uses wgpu
(Direct3D 12 or Vulkan). The executable is not code-signed by default; SmartScreen may warn on the
first start. Cross-compiling from Linux with `cargo xwin` or `x86_64-pc-windows-gnu` is described in
`docs/BUILDING.md`.
