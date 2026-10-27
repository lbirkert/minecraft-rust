# minecraft-rs

This crate provides the application for my minecraft style game
written in rust using WGPU/WGSL for rendering.

Right now this crate contains most of the code of the application.
In the future we might want to out-source a namespace to its own
crate. (e.g. primitives -> minecraft-core)

This might make sense if we consider implementing our own server/
if the project grows too large.

## Contents

```
```txt
src/assets -> assets loader
src/physics -> provides game physics
src/chunk -> chunk loader/generator and renderer
src/primitives -> contains core concepts (world, game, etc.)
src/shaders -> WGSL shader code
src/main.rs -> entrypoint of application
src/config.rs -> constants of the app
```

