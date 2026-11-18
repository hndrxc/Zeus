# Working on DEAD LETTER

This repository builds the `dead-letter` Rust binary. The [README](README.md) covers Windows setup, player controls and Blender commands. Start with the module that owns the behavior you need to change.

## Find the change site

| Task | Start here | Check alongside it |
| --- | --- | --- |
| Tune a weapon, enemy, wave or endorsement | `src/engine.rs` — `Game::step`, `step_bailiff`, `step_censor`, `step_boss`, `wave_kinds`, `apply_boon` | Combat tests at the end of the module, attack warnings in `src/scene.rs`, and player-facing endorsement descriptions |
| Change a key binding, focus behavior or menu | `src/main.rs` — `Shell`, `build_input`, event handlers and `impl Render for Shell` | In-game labels, README controls, mouse and keyboard-only play |
| Adjust camera fit, aim or sprite placement | `src/scene.rs` — `Fit`, `screen_to_world`, `sprite`, `render` | Board offsets from `metrics` in `src/main.rs` and the camera setup in `tools/bake_assets.py` |
| Change a model or animation | `tools/bake_assets.py` for geometry and the bake driver, `tools/courier.py` or `tools/enemies.py` for actor rigs and samples | Generated files under `assets/`, frame selection in `scene.rs`, and `Enemy::animation_frame` in `engine.rs` |
| Change a chamber background | `tools/bake_assets.py` — `arena_base`, `arena_under` and `arena_backdrop` for the shared floor, supports, water and key suns, `arena_sorting` for the Sorting Floor; `tools/arena_vault.py` and `tools/arena_office.py` for the other chambers' landmarks and lights | The three `assets/arena*.png` files, `arena_image` in `src/scene.rs` (which also preloads the next chamber's background), and readability of the HUD, bolts and attack warnings over each chamber |

## Simulation and input contracts

- Keep the simulation reproducible from a seed and a sequence of inputs. The shell obtains a clock seed and accumulates elapsed time. Each call to `Game::step` advances a fixed `DT`. Keep the shell’s `STEP` in sync with the engine if the tick rate changes.
- `Input::movement` is a world-space direction. `Input::aim` is a world-space **point**. Screen-right maps to positive X and positive Y. The shell subtracts the board origin before passing mouse coordinates to `screen_to_world`. Keep projection and its ground-plane inverse paired when changing the viewport.
- Strike and cast repeat while held. The shell latches short presses until the next simulation step, and dash fires on a fresh press. Losing focus clears both held and latched inputs and pauses live combat.
- The engine owns `Ready`, `Combat`, `Choice`, `Won` and `Lost`. Pause belongs to the shell. Use `Game::choose` for endorsement selection so upgrades, healing, chamber advancement and encounter cleanup happen together.
- Attack warnings use dimensions in `scene.rs` that mirror the engine’s strike arcs, projectile spreads and charge reach. When tuning attacks, compare the visible warning and release pose with the tick that can deal damage.

## Assets and tooling

- The renderer embeds the three chamber backgrounds (`arena.png`, `arena_vault.png`, `arena_office.png`) and the directional PNGs with `include_bytes!`. A missing frame breaks compilation. Each actor directory uses `<direction>_<two-digit frame>.png`. Directions 0 through 3 correspond to +X, +Y, −X and −Y.
- Per direction the courier has 19 frames, the Bailiff and Censor have 14 each, and the Postmaster has 34. If you change the sample list, update the corresponding embedded frame table and pose indices together. Preserve the sprite foot anchor when changing the camera or crop.
- Run a targeted bake through Blender, for example `blender -b -P tools/bake_assets.py -- --only censor`. The driver supplies shared globals to the actor and chamber landmark scripts. Blender provides `bpy`, `bmesh` and `mathutils` in its embedded interpreter. Make reproducible art changes in the generator before regenerating its outputs.
- Use Miniconda at `C:\Users\carte\miniconda3` for standalone local Python work. Reuse a dedicated project environment or create one with the latest stable Python available through Conda at creation time. If project requirements conflict with that version, explain the conflict. Keep dependencies out of `base`. Run scripts and tests through activation or `conda run -n <environment>`, and install pip-only packages with that environment’s `python -m pip`.

## Verify a change

- **Simulation.** Run `cargo test --release --locked`. For a focused check, append a test name such as `deterministic_for_same_seed_and_inputs`. Use a fixed seed and scripted inputs for regression tests.
- **Input or layout.** Run `cargo run --locked`. Try keyboard-only aiming, then mouse aiming. Resize the window and check the cursor against the floor target. Switch away during combat and verify movement and attacks release on pause.
- **Animation or telegraphs.** Rebuild after changing baked images. Watch the edited actor turn, wind up and release in game. Check its feet stay on the floor and its attack lands where the warning appeared.
- **Windows packaging.** Build with `cargo build --release --locked`. The MSVC target configuration in `.cargo/config.toml` selects the static C runtime. The resulting executable is `target/release/dead-letter.exe`; Cargo output and local art previews belong under the ignored `target/` directory.
