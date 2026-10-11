# Test saves

The saves the tests run the editor on. They are also the evidence behind [docs/FORMAT.md](../../docs/FORMAT.md), and all of them are the author's own Crash Bandicoot 3: Warped saves: one Switch playthrough, one new PC game, and the in-game tests of the [live test log](../../docs/FORMAT.md#10-live-test-log), where the test names used below (T1, R8…) come from.

| Folder | Files | What they are |
|---|---|---|
| `game/` | 21 | Written by the game. `initial_game.sav` and `0001.sav`–`0019.sav`: one Switch playthrough, saved in the hub after each step (the [save log](../../docs/FORMAT.md#11-save-log)). `ALCHEMYXSAVE_8.sav`: a new PC game. The editor also bundles `initial_game.sav` and `ALCHEMYXSAVE_8.sav` as the new-game saves of the conversion. |
| `edited/` | 13 | Made by the editor and loaded in game, kept as files because no recipe rebuilds them exactly (below). |
| `after_edit/` | 17 | Written by the game after it loaded an edited save: what an in-game test played and saved. |

The other saves the editor made for the in-game tests are rebuilt by the tests instead of being kept (below).

## Kept editor-made saves

| File | Test | Why it is kept |
|---|---|---|
| `step2.sav` | step2 | Its summary stores `farthest_progression_index` 18: the editor didn't raise that value yet when the file was made. The recipe `live/step2` writes 19. |
| `t1_relics.sav` | T1 | `farthest_progression_index` 18; the recipe writes 19 |
| `t2_gems.sav` | T2 | `farthest_progression_index` 18; the recipe writes 28 |
| `t3_removal.sav` | T3 | `farthest_progression_index` 18; the recipe writes 19 |
| `t4_crystals.sav` | T4 | `farthest_progression_index` 18; the recipe writes 30 |
| `t5_secret_room.sav` | T5 | `farthest_progression_index` 18; the recipe writes 24 |
| `t6_full108.sav` | T6 | `farthest_progression_index` 18 and `story_completed` 0; the recipe writes 30 and 1 |
| `t7_bonus_gem.sav` | T7 | `farthest_progression_index` 18 and `story_completed` 0; the recipe writes 30 and 1 |
| `l1_switch_to_pc.sav` | R8, conversion to PC | Made by the conversion, which no recipe can express |
| `l2_pc_to_switch.sav` | R8, conversion to Switch | Made by the conversion |
| `l3_reset_level.sav` | R8, level reset | Made with the level reset, which no recipe can express |
| `l4_story_up_to_3.sav` | R8, "story up to warp room 3" preset | Made by a preset |
| `l5_relics_matching.sav` | R8, "all relics gold, matching times" preset | Made by a preset |

No recipe covers the conversion, the level reset or the presets, so for `l1`–`l5` the files are the only record of the bytes that were tested. `crates/nsane/examples/make_live_round8.rs` builds them again with the editor.

The recipes of `step2` and T1–T7 are kept in `tests/golden/scenarios.json` all the same: their output differs from the file in the summary values above only (`stale_summary_keys`).

## Rebuilt saves

The tests rebuild these saves from their recipe: the scenario `live/<name>` of [tests/golden/scenarios.json](../golden/scenarios.json), an input save and the edits to apply to it. An input can itself be a rebuilt save (`live/step1`). The SHA-256 of each output is in [tests/golden/manifest.json](../golden/manifest.json), and it is the hash of the file that was loaded in game, given below. `crates/nsane/tests/support/corpus.rs` rebuilds a save and checks it against the manifest, and `crates/nsane/tests/saves.rs` checks this table.

The files were deleted from `tests/saves/` but stay in the repository history.

| File | Test | Rebuilt from | SHA-256 of the file tested in game |
|---|---|---|---|
| `lives42.sav` | lives42 | `live/lives42` | `8c35ad315cfe6e32b4b8a205be312a4ce61e003ae1136b6ee80378ef928e5011` |
| `step1.sav` | step2 (its first half: Deep Trouble's edits) | `live/step1` | `d74e209ac6fd0574b952a362295c6a26dc215dc92d5617ae3931a9c0d1a9e422` |
| `t4b_room4.sav` | T4b | `live/t4b_room4` | `7b43869da2eaba065186bc265960b9c8f89546d60a8c89aad035acfd734d622d` |
| `t4c_room5.sav` | T4c | `live/t4c_room5` | `1ccaedcf4c55ef7e83fe06db5daf9dd0bb187e135f504197fe82ec6fe91fc899` |
| `t4d_rooms1to4.sav` | T4d | `live/t4d_rooms1to4` | `6f965c415a2609222da55051a0af9fe1a4b3258d9f28de50d8bf2597aaa22af4` |
| `t4e_rooms1to4_intro.sav` | T4e | `live/t4e_rooms1to4_intro` | `52e887c279c0ddf5fdfd4a41ba5c1b723723cd48ee7065aaf3d6e9417b4eeb18` |
| `t4f_mixed20.sav` | T4f | `live/t4f_mixed20` | `24518ff2f37a8aa7fe520742b861e29e330e622eb0e4d3dae52e7ca5acd2dbd1` |
| `t4g_room3_short.sav` | T4g | `live/t4g_room3_short` | `0afd2c85689ecfc7cf5377d0ec076153d0e3ba689cb9db5dfe5b25bef606402f` |
| `t4h_total22.sav` | T4h | `live/t4h_total22` | `fb6ef15dccd37bf29bb90ae0dd35ac725b8e30d302729bcbd39c7de5fe3cb056` |
| `t8_full108_scenes.sav` | T8 | `live/t8_full108_scenes` | `6b6bee94b213ce52ad311de5aade102972bd84ccfd21446ee78b6460694d74a0` |
| `r7_base.sav` | R7 | `live/r7_base` | `f0b38e5a5859b62fe4514564b60a7b481123998b575e957a10e052140fe34676` |
| `r7_racegems.sav` | R7 (prepared, not played) | `live/r7_racegems` | `bdad0cf9024cd3fc47ecc30c1fe9aaae3b7d152145e8695f9826de074073d910` |
| `pc1_lives42.sav` | PC1 | `live/pc1_lives42` | `94521bbc9ea36f2829dda6eb1925921a926bbbd3b9e6a78f589f56d62acb10f3` |
| `pc2_full108.sav` | PC2 | `live/pc2_full108` | `2e354b0196c11ecd63e316d95c850393f019d302da0bc1d7c18fff3fbbc26234` |
| `l6_relic_panels_pc.sav` | R8, relic times on the time panels (PC) | `live/l6_relic_panels_pc` | `b60f642e22f4c2a958ea4bea969df215c2acaca84edd40d5f9307f881c61a865` |
| `l7_bonus_gem_30.sav` | R8, bonus gem with 30 gold relics | `live/l7_bonus_gem_30` | `d5672d253e1b3f5c57d4ef0ebb6c98ed5e6c94b031d0284f60ac6072ffe16646` |
| `l8_lives_255.sav` | R8, 255 lives | `live/l8_lives_255` | `ea2aea7336ccc5afab281954f251a406177ada99d3bad02110c96c1f5d6f89d2` |
| `r9_lives_98.sav` | R9, 98 lives | `live/r9_lives_98` | `09acda733814ae83e402f15e8c1c9eb3c569fb60516bbcdb0f7f7dc6261935c2` |
| `r9_lives_99.sav` | R9, 99 lives | `live/r9_lives_99` | `a7840e0ce7338a4491fe48b55c98525a658e1d0e11fe3318912a748f36ee91f8` |
| `r9_lives_100.sav` | R9, 100 lives | `live/r9_lives_100` | `d611848c005fe4c355ae8f202baad9c306340031917f56354e5554bb060b42c1` |
| `r9_lives_127.sav` | R9, 127 lives | `live/r9_lives_127` | `6b69ef424631243d1cea90daf5c014ffef7bdbd5a1904fa3ba576c3556f24a96` |
| `r9_lives_128.sav` | R9, 128 lives | `live/r9_lives_128` | `bff5590324037cf36750d294bb2515e26b73fe7ba117e3276017ad3ab19ee8db` |
| `r9_lives_200.sav` | R9, 200 lives | `live/r9_lives_200` | `b564aa054282f8fa18b9cc5b5ffafa19933cc6151148f04ca18f3f50188eebb8` |
| `r9_lives_253.sav` | R9, 253 lives | `live/r9_lives_253` | `f08ab1edaf45e3bcbbd446a09ff30eab2ebd729efc172c2d5658a66f3ab5e58f` |
| `t7_after_picking_47th_gem.sav` | T7 (see below) | `edited/t7_bonus_gem.sav` | `c80903e353f6aeefa783a57a659ec03a8af1a0455020cabdc8cc536c4ee0f7b8` |

`t7_after_picking_47th_gem.sav` was meant to be the game's save after the bonus gem was picked up in T7, but the wrong file was copied back: it is byte for byte `edited/t7_bonus_gem.sav`. `after_edit/t7_after_beating_cortex.sav` shows the gem taken.
