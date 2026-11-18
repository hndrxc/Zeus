# DEAD LETTER: The Undertow Post

*No address beyond the tide.* Play a courier delivering mail through a drowned sorting office run by clockwork clerks. DEAD LETTER is a single-player isometric action roguelike written in Rust. All three chambers share one submerged diorama. The waves, endorsements and boss patterns change as the delivery progresses.

## Play on Windows

Extract `dead-letter-windows-x64.zip` and launch `dead-letter.exe`. If you have the source checkout, use the build instructions below. The Windows package targets Windows 11 x64 with Direct3D 11.1. The executable embeds the runtime artwork, so players can run it without a separate asset folder, Rust installation or Blender installation.

| Input | Action |
| --- | --- |
| Enter, Space or left click | Start from the title screen |
| WASD or arrow keys | Move relative to the screen |
| Mouse | Aim at a point on the arena floor |
| Left mouse button or J | Strike. Hold to chain three swings. A swing can cut hostile projectiles in front of the courier. |
| Right mouse button or K | Cast a ranged seal. Hold to repeat as the cooldown expires. |
| Space during combat | Dash with brief invulnerability |
| 1, 2, 3 or click an endorsement | Choose an upgrade between chambers |
| Esc | Pause or resume combat. Losing window focus also pauses combat. |
| Enter or R | Start a fresh run after victory or defeat |
| Tab or click Skip drill | Skip the opening drill |
| T on the title or results screen | Start a run that opens with the drill |

For keyboard-only play, J and K aim in the movement direction, or along the courier’s last facing when standing still. Move the mouse to return to pointer aiming.

## A delivery from start to finish

1. **Sorting Floor.** The first delivery of a session opens with a short drill: strike, cast, then dash, each prompted at the bottom of the screen. Waves hold until the drill ends, and finishing or skipping it leaves the seeded run unchanged. Then clear two waves. Bailiffs wind up before lunging, giving you time to sidestep or dash. Censors introduce ranged fire in the second wave.
2. **Dead Letter Vault.** Clear three waves of mixed clerks. After each of the first two chambers, pick one of three endorsements. The choice applies for the run, and entering the next chamber restores 25 HP up to your maximum.
3. **Postmaster’s Office.** Face the Postmaster. At half health the boss briefly becomes invulnerable and begins its second shift. Watch the floor markings for charge lanes and projectile patterns.

## Build and test

From the repository root on Windows, use a stable Rust MSVC toolchain (`x86_64-pc-windows-msvc`), Visual Studio C++ Build Tools and a Windows SDK that includes `fxc.exe`. If GPUI cannot find the shader compiler, add the SDK directory containing `fxc.exe` to `PATH` or set `GPUI_FXC_PATH` to the executable.

```powershell
cargo build --release --locked
.\target\release\dead-letter.exe
```

For the edit-and-run loop, use `cargo run --locked`. To run the simulation tests with the release profile, use `cargo test --release --locked`. The tests exercise seeded replay, combat timing, projectile interactions, arena bounds and run progression. For a crash during startup or play, the panic hook writes the latest panic to `$env:TEMP\dead-letter-crash.log` on Windows.

## CI and releases

Pull requests and pushes to `main` run formatting checks, simulation tests and a release build on Windows. Download the `windows-x64` artifact from the [workflow run](https://github.com/hndrxc/Zeus/actions/workflows/windows.yml) to playtest its packaged executable.

To publish, update the package version in Cargo.toml and Cargo.lock, merge the tested commit, and push a tag matching that version (for example, `v1.0.0`). The tag run rebuilds and tests the game, verifies the archive, then publishes the ZIP and its SHA-256 checksum to [GitHub Releases](https://github.com/hndrxc/Zeus/releases). A prerelease version such as `1.0.0-rc.1` produces a GitHub prerelease.

## Where the code lives

| File | Responsibility |
| --- | --- |
| [src/engine.rs](src/engine.rs) | Advances the game one 60 Hz tick at a time. Owns player and enemy state, collisions, projectiles, waves, endorsements, and simulation tests. |
| [src/main.rs](src/main.rs) | Hosts the GPUI window. Translates key and mouse events into game inputs, drives the fixed-step clock, and draws the full-screen HUD and menus. |
| [src/scene.rs](src/scene.rs) | Projects world positions onto the arena, maps cursor positions back to the ground, selects sprite frames, and layers actors, attack warnings and effects. |
| [assets/](assets/) | Contains the baked arena, directional animation frames, character stills and editable Blender character scenes. |
| [tools/](tools/) | Contains the procedural Blender bake driver and actor-specific rigging and animation code. |

## Rebuild the artwork

Ordinary builds use the supplied PNGs. To change generated art, edit the procedural models or animations and run Blender in background mode from the repository root. The driver constructs the scenes and writes the resulting images and character `.blend` files. A normal character bake can therefore overwrite hand edits to its saved scene.

```powershell
# Rebuild just the courier with the default Cycles renderer
blender -b -P tools/bake_assets.py -- --only courier

# Rebuild the arena and all four characters
blender -b -P tools/bake_assets.py
```

The available jobs are `arena`, `courier`, `bailiff`, `censor` and `postmaster`. The arena PNG is 2700 × 1800, fitted to a 900 × 600 design space. Each 768 × 768 character frame represents a 256 × 256 sprite in that space. Each character has four orthographic views. Rebuild the executable after a bake to pick up changed PNGs.

For implementation conventions and checks to run when changing combat or art, see [AGENTS.md](AGENTS.md).
