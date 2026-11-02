# AlgoClerk for iOS

The game is the Rust static library `libalgoclerk_app.a` (crate `algoclerk-app`, entry point
`algoclerk_ios_main`). A tiny Objective-C `main.m` calls it; winit (inside the library) runs
`UIApplicationMain` and wgpu renders through Metal. The Xcode project is generated from
`project.yml` with [XcodeGen](https://github.com/yonaskolb/XcodeGen), so no generated project files
are committed.

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
brew install xcodegen
ios/build-ios.sh simulator     # unsigned simulator build
ios/build-ios.sh archive       # device archive; needs ALGOCLERK_TEAM_ID (Apple developer team)
```

Signing: device builds need an Apple Developer account; the release workflow only produces the
simulator build unless the secrets listed in `docs/BUILDING.md` are configured.

The iOS path has **not** been verified on real hardware or in the simulator by the maintainers who
wrote it (the project was developed on Linux); the CI job `ios-check` type-checks the crate for the
iOS targets, and `release.yml` builds the simulator app on a macOS runner. Please report problems.
