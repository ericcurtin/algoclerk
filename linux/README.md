# AlgoClerk for Linux

```sh
linux/package-linux.sh            # -> dist/algoclerk-<version>-linux-<arch>.tar.gz
cargo run --release -p algoclerk-app --bin algoclerk
```

The window code loads Wayland, X11 and Vulkan/OpenGL libraries at run time, so building needs no
`-devel` packages (except ALSA headers for the optional `audio` feature). At run time you need a
Wayland or X11 session and a Vulkan or OpenGL driver (Mesa is fine). `algoclerk --screenshot ...`
renders without any of that. The tarball contains a `.desktop` file, an icon and AppStream metadata
for packagers.
