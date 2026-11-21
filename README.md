# DEAD LETTER: The Undertow Post

*No address beyond the tide.* Play a courier delivering mail through a drowned sorting office run by clockwork clerks. DEAD LETTER is a single-player isometric action roguelike written in Rust. Before each of the first two chambers, the courier walks a hall of the drowned building that scrolls beyond a single screen, and each of the three chambers has its own submerged arena art. The waves, endorsements and boss patterns change as the delivery progresses.

## Play on Windows

Extract `dead-letter-windows-x64.zip` and launch `dead-letter.exe`. If you have the source checkout, use the build instructions below. The Windows package targets Windows 11 x64 with Direct3D 11.1. The executable embeds the runtime artwork, so players can run it without a separate asset folder, Rust installation or Blender installation.

| Input | Action |
| --- | --- |
| Enter, Space or left click | Start from the title screen |
| WASD or arrow keys | Move relative to the screen |
| Mouse | Aim at a point on the floor |
| Left mouse button or J | Strike. Hold to chain a slash, a backhand and a finishing chop. A swing can cut hostile projectiles in front of the courier. |
| Right mouse button or K | Cast a ranged seal. Hold to repeat as the cooldown expires. |
| Space in a hall or during combat | Dash with brief invulnerability |
| 1, 2, 3 or click an endorsement | Choose an upgrade between chambers |
| Esc | Pause or resume play. Losing window focus also pauses. |
| O or click Settings on the title or pause menu | Open Settings |
| Enter, R or click Fresh run | Start a fresh run after victory or defeat |
| P or click Retry same seed | Retry the finished delivery after victory or defeat |
| Tab or click Skip drill | Skip the opening drill |
| T on the title or results screen | Start a run that opens with the drill |

For keyboard-only play, J and K aim in the movement direction, or along the courier’s last facing when standing still. Move the mouse to return to pointer aiming.

Settings lets you hide the in-arena aim marker without changing how aiming works, or hide the title and pause control hints. Changes save immediately for the current user and survive game restarts. **Reset Defaults** turns both options back on. Missing, damaged or older settings files start the game with defaults.

The file is `%APPDATA%\dead-letter\settings.json` on Windows, `~/Library/Application Support/dead-letter/settings.json` on macOS, and `$XDG_CONFIG_HOME/dead-letter/settings.json` (or `~/.config/dead-letter/settings.json`) on Linux.

The results screen shows the full delivery seed. A same-seed retry returns to the Sorting Floor hall entrance, skipping the drill, and repeats the waves and endorsement offers when you use the same inputs and make the same choices. The way you walk a hall never changes the seeded waves or offers.

## A delivery from start to finish

Before the first two fights, follow the route card to the delivery door. Each hall has a landmark and an optional loop that rejoins the main route. You can walk and dash in the halls, but strikes and casts only work in combat. The Postmaster’s Office has no separate hall.

1. **Sorting Floor.** Find the sorting hall’s delivery door to enter the chamber. On the first delivery of a session, a short drill prompts you to strike, cast and dash before the waves begin. Finishing or skipping it leaves the seeded run unchanged. Clear two waves: two Bailiffs open with lunges you can sidestep or dash through, then Censors introduce ranged fire. Later non-boss waves vary the mix of Bailiffs and Censors by delivery seed.
2. **Dead Letter Vault.** Walk through the archive stacks to reach the Vault chamber. Clear three waves of mixed clerks. The second and third Vault waves each bring a Ward Clerk, a support enemy that shields the nearest nearby Bailiff or Censor; that recipient takes 60% less damage. A diamond marks the Ward Clerk, a shield marks the protected recipient, and a lavender tether shows the active connection. Defeat the Ward Clerk or separate the two to drop the protection. Support placement follows the delivery seed. After each of the first two chambers, pick one of three endorsements. The choice applies for the run; the repair before the next encounter restores 25 HP up to your maximum.
3. **Postmaster’s Office.** Look for the postmaster’s desk and crest behind the floor. Face the Postmaster. At half health the boss briefly becomes invulnerable and begins its second shift. Watch the floor markings for charge lanes and projectile patterns.

Each endorsement card lists the stats it changes as current → resulting values, so you can compare the real effect of each pick. Move speed and strike reach also show the percentage change. The Pressure Valve card shows your hull on entering the next chamber, after the 25 HP repair, without and with the valve. Pressing 1, 2 or 3 or clicking a card applies exactly what the card shows.

## Build and test

From the repository root on Windows, use a stable Rust MSVC toolchain (`x86_64-pc-windows-msvc`), Visual Studio C++ Build Tools and a Windows SDK that includes `fxc.exe`. If GPUI cannot find the shader compiler, add the SDK directory containing `fxc.exe` to `PATH` or set `GPUI_FXC_PATH` to the executable.

```powershell
cargo build --release --locked
.\target\release\dead-letter.exe
```

For the edit-and-run loop, use `cargo run --locked`. To run the simulation tests with the release profile, use `cargo test --release --locked`. The tests exercise seeded replay, combat timing, projectile interactions, Ward Clerk protection, arena bounds, hall routes and collisions, and run progression. For a crash during startup or play, the panic hook writes the latest panic to `$env:TEMP\dead-letter-crash.log` on Windows.

## CI and releases

Pull requests and pushes to `main` run formatting checks, simulation tests and a release build on Windows. Download the `windows-x64` artifact from the [workflow run](https://github.com/hndrxc/Zeus/actions/workflows/windows.yml) to playtest its packaged executable.

To publish, update the package version in Cargo.toml and Cargo.lock, merge the tested commit, and push a tag matching that version (for example, `v1.0.0`). The tag run rebuilds and tests the game, verifies the archive, then publishes the ZIP and its SHA-256 checksum to [GitHub Releases](https://github.com/hndrxc/Zeus/releases). A prerelease version such as `1.0.0-rc.1` produces a GitHub prerelease.

## Where the code lives

| File | Responsibility |
| --- | --- |
| [src/engine.rs](src/engine.rs) | Advances the game one 60 Hz tick at a time. Owns player and enemy state, collisions, projectiles, waves, endorsements, hall walking and its camera, and simulation tests. |
| [src/floor.rs](src/floor.rs) | Loads the hall layouts and resolves the courier's movement against their walls, counters, racks and railings. |
| [src/main.rs](src/main.rs) | Hosts the GPUI window. Translates key and mouse events into game inputs, drives the fixed-step clock, and draws the full-screen HUD and menus. |
| [src/settings.rs](src/settings.rs) | Loads and saves versioned per-user preferences. |
| [src/scene.rs](src/scene.rs) | Projects world positions through the camera onto the arena or hall, maps cursor positions back to the ground, selects sprite frames, and layers actors, scenery the courier stands behind, attack warnings and effects. |
| [assets/](assets/) | Contains the baked chamber backgrounds, the hall layouts and baked hall art in `assets/floors/`, directional animation frames, character stills and editable Blender character scenes. |
| [tools/](tools/) | Contains the procedural Blender bake driver, the hall art scripts, the Dead Letter Vault and Postmaster’s Office landmark scripts, actor-specific rigging and animation code, and the floor-plan generator. |
| [docs/floors/](docs/floors/) | Annotated plans for the [Sorting Floor hall](docs/floors/sorting_hall_plan.png) and [Vault Stacks](docs/floors/vault_stacks_plan.png). |

## Rebuild the artwork

Ordinary builds use the supplied PNGs. To change generated art, edit the procedural models or animations and run Blender in background mode from the repository root. The bakes were verified with Blender 5.2. A character bake also writes its `.blend` file, so it can overwrite hand edits to that scene.

```powershell
# Rebuild just the courier with the default Cycles renderer
blender -b -P tools/bake_assets.py -- --only courier

# Rebuild just the Ward Clerk support enemy
blender -b -P tools/bake_assets.py -- --only ward_clerk

# Rebuild the three arena PNGs, both halls and all five characters
blender -b -P tools/bake_assets.py

# Rebuild both halls without rebaking chambers or characters
blender -b -P tools/bake_assets.py -- --only sorting_hall vault_stacks
```

Final bakes default to Cycles at 64 samples. Blender renders on the first available GPU backend (OptiX, CUDA, HIP, oneAPI or Metal) and falls back to the CPU if none is available. Cycles denoises on the CPU to reduce variation between runs; rebakes are not guaranteed byte-identical. Use the regular bake command for GPU output; there is no separate `--dev` or `--gpu-preview` path.

The available jobs are `arena`, `sorting_hall`, `vault_stacks`, `courier`, `bailiff`, `censor`, `ward_clerk` and `postmaster`. To rebuild only the chamber art, run `blender -b -P tools/bake_assets.py -- --only arena`. It writes `assets/arena.png` (Sorting Floor), `assets/arena_vault.png` (Dead Letter Vault) and `assets/arena_office.png` (Postmaster’s Office). All three are 2700 × 1800 and share one registration to the 900 × 600 design space. Each 768 × 768 character frame represents a 256 × 256 sprite in that space. Each character has eight orthographic views. Directions 0 through 3 face +X, +Y, −X and −Y. Directions 4 through 7 face the diagonals at 45°, 135°, 225° and 315° from +X. The courier uses 27 frames per view, with the backhand at 19–22 and the finishing chop at 23–26. Rebuild the executable after a bake to pick up changed PNGs.

### Change a hall

1. Edit `assets/floors/<id>.floor` for collision, door thresholds, routes or camera framing. Edit `tools/floor_sorting.py` or `tools/floor_vault.py` for art. Keep visible block footprints and heights aligned with the layout, tag scenery that dresses a block with its `block` index, and avoid overlapping faces on the same plane.
2. Run `blender -b -P tools/bake_assets.py -- --only <id>`. It checks camera registration, then writes the background, `<id>_occ.png` atlas, and `<id>.occ` atlas table into `assets/floors/`. The backgrounds are 4160 × 2840 for `sorting_hall` and 4160 × 2880 for `vault_stacks`. `--graybox` substitutes layout boxes and **writes to the same output paths**; run again without it before committing the art.
3. Run `python tools/floor_plan.py` from a Python environment with Pillow to regenerate the plans under `docs/floors/`. Each plan marks the main route, loop, side threshold, and camera frames at arrival, junction and return.
4. Rebuild the game to embed the new images. Walk the main route and loop at the 820 × 560 minimum window size and again at a larger size. Try keyboard-only and pointer aiming; check that the courier’s feet, the next turn and the floor target remain visible and aligned while scrolling.
