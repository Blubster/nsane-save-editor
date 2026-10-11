# Crash Bandicoot N. Sane Trilogy save format (Crash 3: Warped)

This is the format reference for the editor. The code and the other documents of this repository cite it. Every rule below names its evidence, using these sources:

- **Saves the game wrote.** `saves/0001.sav`–`0019.sav` and `saves/initial_game.sav`: one Switch playthrough, one save after each step (see the [save log](#11-save-log)). `saves/ALCHEMYXSAVE_8.sav`: a fresh PC game. The `*_after*.sav` and `r7a*.sav` files in `test_saves/`: what the game wrote after a live test.
- **Live tests.** Saves made by the editor, loaded on a Switch or a PC, played, and often saved again by the game. They are named T1–T8, R7, PC1/PC2 and R8 after the test rounds. The files are in `test_saves/`, and the results are in the [live test log](#10-live-test-log).
- **Public sources.** Community guides and wikis, cited by URL where they are used.

"All observed saves" means the 54 `.sav` files in `saves/` and `test_saves/`, outside `test_saves/round8/` and `test_saves/round9/`. Those folders hold the later test rounds R8 and R9: the saves the editor made for them and those the game wrote afterwards (`*_after.sav`), described in the [live test log](#10-live-test-log).

## Contents

1. [Platforms](#1-platforms)
2. [Container: Alchemy IGB](#2-container-alchemy-igb)
3. [Levels](#3-levels)
4. [Zone sessions](#4-zone-sessions)
5. [Variables and flags](#5-variables-and-flags)
6. [Summary table](#6-summary-table)
7. [Game rules](#7-game-rules)
8. [What an edit must write](#8-what-an-edit-must-write)
9. [Relic time thresholds](#9-relic-time-thresholds)
10. [Live test log](#10-live-test-log)
11. [Save log](#11-save-log)

## 1. Platforms

| | Switch | PC |
|---|---|---|
| File name | `MACK_<n>.sav` on the console, `<n>` being the save slot (below). The save manager used here exports it under that name; the copies in `saves/` were renamed `0001.sav`… for storage. | `ALCHEMYXSAVE_<n>.sav`, `<n>` being the save slot (below) |
| Size | Always **153,600 bytes**: the content, then zero padding (every Switch save in `saves/`) | **Exactly the content**, with no padding: 64,781 bytes for a fresh game (`saves/ALCHEMYXSAVE_8.sav`) |
| Container | Alchemy IGB, version `0xB400000C` | Same |
| Meta fields (type registry) | 821 | 860: the PC registry has extra rendering and input types |
| Game data | | Identical: same classes, summary keys, strings, handle names, zones and variables. A fresh PC save matches `saves/initial_game.sav` field for field. |
| Checksum | None | None |
| "No reference" value | `0xFFFFFFFE` in the empty references of the save data objects (`CDynamicDifficultySaveData`, `CQuestTrackSaveDataList`, `CGameVariableValueSavedStringList`); `0xFFFFFFFD` in `igObjectDirectoryIGBInfo` | The other way round |
| Bool upper bytes | Between the two fresh saves, they differ in 84 of the 116 zone sessions and in 21 of the 30 bool variables | |

- **Switch.** An edited file is written back padded to 153,600 bytes, and an edit whose content no longer fits is refused. A 108% save with every scene flag set uses 68,157 bytes before padding (`test_saves/t8_full108_scenes.sav`).
- **PC.** An edited file is written at its new length. A full 108% edit grows the fresh save from 64,781 to 68,318 bytes. PC1 (same size) and PC2 (grown) both loaded in game.
- **Converting between platforms.** The game data maps one to one by meaning (zone path, variable name, flag key, summary key), not by object id. The editor copies it into a save of the target platform (a "template") and keeps the template's own container: type registry, ids, "no reference" values, bool upper bytes and padding. Both fresh saves convert to each other byte for byte. Converted saves load in game in both directions with the same progress, and the game's next save keeps all of it (R8: Switch → PC and PC → Switch).
- **All three games are in one file.** Every save holds 116 zone sessions (`crash1/…`, `crash2/…`, `crash3/…`, `whiterooms/…`) and the variables of the three games. Only Crash 3 is decoded here.
- **Save slots.** Each game has 4 save slots, and each slot is one file numbered `<n>` = 0 to 11, the same on both platforms: 0–3 for Crash Bandicoot, 4–7 for Crash 2 and 8–11 for Crash 3 (`MACK_8.sav`… `MACK_11.sav`, `ALCHEMYXSAVE_8.sav`… `ALCHEMYXSAVE_11.sav`). Observed on the PC and on the Switch by looking at the files each slot writes. `saves/ALCHEMYXSAVE_8.sav` is the first Crash 3 slot. What the game does with a Crash 3 save placed in another game's slot has not been tested. On the PC used here, the save files are in `%APPDATA%\Game`.

## 2. Container: Alchemy IGB

All integers are little-endian `u32` unless noted. Every section carries its own size. The editor rebuilds every observed save byte for byte.

| Section | Layout |
|---|---|
| Header (offset 0) | 12 × `u32`: `(entries size, count)`, `(meta objects size, count)`, `(objects size, count)`, `(memory size, count)`, `(meta fields size, count)`, magic `0xFADA`, version. The entries and memory pairs change when a hash table grows: 31 entries and memory `0x63c` in `saves/0019.sav`, 32 entries and `0x77c` in `test_saves/t5_secret_room.sav`. **Recompute them on write.** |
| Summary (offset `0x30`) | `u32 3`, then 3 size-prefixed arrays: entries `(type, key offset, value offset)` (type 0 = int, 1 = float, 2 = string), the key strings, the values. See [§6](#6-summary-table). |
| String table | `u32 size` (including itself), `u32 count`, then per string `u32 length` (including the NUL) and the latin-1 bytes. Length 0 is the null string, always at index 0. The other strings are **kept sorted by bytes**. String fields, string memory blocks and name elements store indices into this table, so inserting a string means renumbering every reference after it. |
| Meta fields | count × `(u32 name length, u32, u32)`, then the type names (`igIntMetaField`…). Kept as is. |
| Meta objects | 36 × `(u32 name length, u32, u32, i32 field count, i32 parent, i32)`, then for each: the class name (padded to 2 bytes) and its fields `(u16 meta field index, u16 slot, u16 size)`. **Look classes and types up by name**, never by index: the indices differ between Switch and PC. |
| Handle names | `u32 size, u32 1, u32 n`, n × hash, n NUL-terminated names. Hash = FNV-1a-32 of the **lowercased** name. It resolves the handle that names each game variable. |
| Memory pools | `u32 size, u32 n`, then the names. Kept as is. |
| Pool index list | `u32 size`, then ints. Kept as is. |
| Directory | `entries count` records `(u32 class, u32 record size, values)`. An object entry is `(name, class index, pool)`; a memory entry is `(name, element count, element type, active, alignment, pool)`. Memory blocks with the same count and type share one entry, so resizing a block can add an entry, and an entry no block uses any more is dropped. |
| Object index | `u32 size, u32 count`, count × `u16` directory index, then padding. The position in this list is the **id** that references use; objects and memory blocks share the id space. Ids differ from one save to the next, so they are never hard-coded. |
| `u32` | Unknown, kept as is (285 in `saves/initial_game.sav`, 14 in `saves/ALCHEMYXSAVE_8.sav`). |
| Objects | For each object id in order: `u32 class index, u32 size`, then the fields. String, object reference, memory reference, int, enum, uint and float are 4 bytes; a handle is 8 bytes `(name hash, namespace hash)`; a vector is 8 bytes; a bool is 4 bytes, where only the low byte is the value and the upper bytes hold arbitrary data that is **written back unchanged**. Every observed save has 288 objects. |
| Memory | For each memory id in order, the raw elements: object reference, int and string are 4 bytes, bool 1 byte, name 8 bytes `(string index, hash)`. Unused slots of the int hash table hold the fill byte `0xFA`. |
| Switch only | Zero padding up to 153,600 bytes. |

### Hash tables

The two hash tables (`igStringBoolHashTable`, `igStringIntHashTable`) have the fields `(values, keys, count, bool, load factor 0.5)`: **field 0 holds the values and field 1 the keys**. Keys are string indices.

- **Slot.** A key's slot is FNV-1a-32 of the key text (**case sensitive**, unlike handles) modulo the capacity, with linear probing.
- **Growth.** Capacity starts at 64. Before an insert, if `count + 1 > capacity × 0.5`, the capacity doubles and every key is inserted again. After a growth, empty key slots are 0, empty bool values 0 and empty int values `0xFAFAFAFA`.
- **When it happens.** The bool table of a mid-game save holds about 16 keys (`saves/0019.sav`), so adding about 16 relic or cutscene flags makes it grow. T5 added 18 keys and grew the table from 64 to 128 slots; the game loaded it.

## 3. Levels

### Level table

"Index" is the progression index used by the summary ([§6](#6-summary-table)): story order, Toad Village = 1 to Dr. Neo Cortex = 30. The name column is the spelling the game stores in `<Name>_RelicCounted` flag keys (seen in the game's own saves, for example `Toad Village_RelicCounted` in `saves/0010.sav`).

| # | Name | Key | Room | Kind | Zone path | Index | Crystal | Second gem | Colored gem |
|---|---|---|---|---|---|---|---|---|---|
| 1 | Toad Village | `toadvillage` | 1 | level | `crash3/l301_toadvillage/l301_toadvillage` | 1 | yes | – | – |
| 2 | Under Pressure | `underpressure` | 1 | level | `crash3/l302_underpressure/l302_underpressure` | 2 | yes | – | – |
| 3 | Orient Express | `orientexpress` | 1 | level | `crash3/l303_orientexpress/l303_orientexpress` | 3 | yes | – | – |
| 4 | Bone Yard | `boneyard` | 1 | level | `crash3/l304_boneyard/l304_boneyard` | 4 | yes | yes | – |
| 5 | Makin' Waves | `makinwaves` | 1 | level | `crash3/l305_makinwaves/l305_makinwaves` | 5 | yes | – | – |
| – | Tiny Tiger | `tinytiger` | 1 | boss | `crash3/bosses/b301_tinytiger/b301_tinytiger` | 6 | – | – | – |
| 6 | Gee Wiz | `geewiz` | 2 | level | `crash3/l306_geewiz/l306_geewiz` | 7 | yes | – | – |
| 7 | Hang'em High | `hangemhigh` | 2 | level | `crash3/l307_hangemhigh/l307_hangemhigh` | 8 | yes | – | yellow |
| 8 | Hog Ride | `hogride` | 2 | level | `crash3/l308_hogride/l308_hogride` | 9 | yes | – | – |
| 9 | Tomb Time | `tombtime` | 2 | level | `crash3/l309_tombtime/l309_tombtime` | 10 | yes | yes | – |
| 10 | Midnight Run | `midnightrun` | 2 | level | `crash3/l310_midnightrun/l310_midnightrun` | 11 | yes | – | – |
| – | Dingodile | `dingodile` | 2 | boss | `crash3/bosses/b302_dingodile/b302_dingodile` | 12 | – | – | – |
| 11 | Dino Might! | `dinomight` | 3 | level | `crash3/l311_dinomight/l311_dinomight` | 13 | yes | yes | – |
| 12 | Deep Trouble | `deeptrouble` | 3 | level | `crash3/l312_deeptrouble/l312_deeptrouble` | 14 | yes | – | red |
| 13 | High Time | `hightime` | 3 | level | `crash3/l313_hightime/l313_hightime` | 15 | yes | – | purple |
| 14 | Road Crash | `roadcrash` | 3 | level | `crash3/l314_roadcrash/l314_roadcrash` | 16 | yes | – | – |
| 15 | Double Header | `doubleheader` | 3 | level | `crash3/l315_doubleheader/l315_doubleheader` | 17 | yes | – | – |
| – | Dr. N. Tropy | `ntropy` | 3 | boss | `crash3/bosses/b303_ntropy/b303_ntropy` | 18 | – | – | – |
| 16 | Sphynxinator | `sphynxinator` | 4 | level | `crash3/l316_sphynxinator/l316_sphynxinator` | 19 | yes | yes | – |
| 17 | Bye Bye Blimps | `byebyeblimps` | 4 | level | `crash3/l317_byebyeblimps/l317_byebyeblimps` | 20 | yes | – | – |
| 18 | Tell No Tales | `tellnotales` | 4 | level | `crash3/l318_tellnotales/l318_tellnotales` | 21 | yes | – | – |
| 19 | Future Frenzy | `futurefrenzy` | 4 | level | `crash3/l319_futurefrenzy/l319_futurefrenzy` | 22 | yes | yes | – |
| 20 | Tomb Wader | `tombwader` | 4 | level | `crash3/l320_tombwader/l320_tombwader` | 23 | yes | – | blue |
| – | N. Gin | `ngin` | 4 | boss | `crash3/bosses/b304_ngin/b304_ngin` | 24 | – | – | – |
| 21 | Gone Tomorrow | `gonetomorrow` | 5 | level | `crash3/l321_gonetomorrow/l321_gonetomorrow` | 25 | yes | yes | – |
| 22 | Orange Asphalt | `orangeasphalt` | 5 | level | `crash3/l322_orangeasphalt/l322_orangeasphalt` | 26 | yes | – | – |
| 23 | Flaming Passion | `flamingpassion` | 5 | level | `crash3/l323_flamingpassion/l323_flamingpassion` | 27 | yes | – | green |
| 24 | Mad Bombers | `madbombers` | 5 | level | `crash3/l324_madbombers/l324_madbombers` | 28 | yes | – | – |
| 25 | Bug Lite | `buglite` | 5 | level | `crash3/l325_buglite/l325_buglite` | 29 | yes | yes | – |
| – | Dr. Neo Cortex | `neocortex` | 5 | boss | `crash3/bosses/b305_neocortex/b305_neocortex` | 30 | – | – | – |
| 26 | Ski Crazed | `skicrazed` | 6 | level | `crash3/l326_skicrazed/l326_skicrazed` | – | – | – | – |
| 27 | Hang'em High (second entrance) | `27` → `hangemhigh` | 6 | – | shares level 7's zone | – | – | – | – |
| 28 | Area 51? | `area51` | 6 | level | `crash3/l328_area51/l328_area51` | – | – | yes | – |
| 29 | Future Frenzy (second entrance) | `29` → `futurefrenzy` | 6 | – | shares level 19's zone | – | – | – | – |
| 30 | Rings of Power | `ringsofpower` | 6 | level | `crash3/l330_ringsofpower/l330_ringsofpower` | – | – | yes | – |
| – | Hot Coco | `hotcoco` | 6 | secret | `crash3/l331_hotcoco/l331_hotcoco` | – | – | – | – |
| – | Eggipus Rex | `eggipusrex` | 6 | secret | `crash3/l332_eggipusrex/l332_eggipusrex` | – | – | – | – |
| – | Future Tense | `futuretense` | 6 | secret | `crash3/l333_futuretense/l333_futuretense` | – | – | yes | – |

- Every level and secret level (31 in all) has a clear gem and a relic. Only levels 1–25 have a crystal.
- In the Kind column, the numbered secret-room levels 26, 28 and 30 are `level`, while the unnumbered Hot Coco, Eggipus Rex and Future Tense are `secret`. "Room 6" is the secret warp room.
- The progression indices are those the game writes in `previous_level_progression_index` and `farthest_progression_index` (for example 25 after beating N. Gin in `test_saves/t4d_after.sav`, 30 after Cortex in `test_saves/t7_after_beating_cortex.sav`).
- The second gem and colored gem columns follow the guides cited below, and were checked in game where the notes say so.

### Colored gems and their paths

| Gem | Found in | Zone bit | Variable | Opens a path in |
|---|---|---|---|---|
| Red | Deep Trouble: the "!" crate at the end, then a crate earlier in the level [PowerPyx gems] | `GEM_RED` | `Gem_Red` | **Bone Yard** (step2, T2), Bug Lite |
| Yellow | Hang'em High, through its second entrance, level 27 (10 relics) [PowerPyx gems]; picked up in T5 | `GEM_YELLOW` | `Gem_Yellow` | **Dino Might!** (T2), whose path leads to **Eggipus Rex** (T2, [PowerPyx Eggipus Rex]); Bug Lite |
| Purple | High Time, death route [PowerPyx gems] | `GEM_PURPLE` | `Gem_Purple` | **Tomb Time** (T2), Bug Lite |
| Blue | Tomb Wader, death route [PowerPyx gems] | `GEM_BLUE` | `Gem_Blue` | **Sphynxinator** (T2), Bug Lite |
| Green | Flaming Passion, death route [PowerPyx gems] | `GEM_GREEN` | `Gem_Green` | Gone Tomorrow [PowerPyx gems], Bug Lite |

- **A colored gem is its zone bit plus its `Gem_<Color>` variable.** Setting both is enough for its platforms to work (step2, T2). The game writes exactly these two when the gem is picked up for real: T5's yellow gem, in `test_saves/t5_after.sav`.
- Bug Lite's multicolored path needs all five gems [PowerPyx gems]. It was visible in T6.
- A `Gem_Orange` variable exists in every save. It is 0 in all observed saves, and the editor never writes it.

### Second gems

Ten levels have a second clear gem [PowerPyx gems]. The gem for breaking every box is always `GEM_CLEAR`; the other one is `GEM_SECOND`. In `saves/0007.sav`–`0019.sav` the game wrote Bone Yard with `GEM_CLEAR | CRYSTAL` (`0x0202`) after every box was broken, while its red-gem path was still out of reach.

| Level | How the second gem is earned |
|---|---|
| Bone Yard | Red gem path [PowerPyx gems] |
| Tomb Time | Purple gem path (platforms checked in T2) [PowerPyx gems] |
| Dino Might! | Yellow gem path [PowerPyx gems] |
| Sphynxinator | Blue gem path [PowerPyx gems] |
| Future Frenzy | Second entrance, level 29 (20 relics) [PowerPyx gems] |
| Gone Tomorrow | Green gem path [PowerPyx gems] |
| Bug Lite | Multicolored path, needs all five colored gems [PowerPyx gems] |
| Area 51? | Winning the race [PowerPyx gems] |
| Rings of Power | Winning the race [PowerPyx gems] |
| Future Tense | The death route, which can't be reached without some of the super powers [Wiki Future Tense] |

The race gems of Area 51? and Rings of Power, and Future Tense's death route gem, are taken to be `GEM_SECOND`, by the rule above. This has not been checked separately in game; T6 (both gems set on every level) loaded and showed 108%.

### Bosses and abilities

| Boss | Opens | Ability (variable) | Evidence | Story scenes (cutscene step) |
|---|---|---|---|---|
| Tiny Tiger | warp room 2 | Super Charged Body Slam (`Upgrade_SuperChargedBodySlam`) | set by the game in `saves/0009.sav` | `C3_Tiny_Intro`, `C3_Tiny_PreBossFight`, `C3_Tiny_PostBossFight` |
| Dingodile | warp room 3 | Double Jump (`Upgrade_DoubleJump`) | set by the game in `saves/0017.sav` | `C3_Hub2_Intro`, `C3_Dingodile_Intro`, `C3_Dingodile_PreBossFight` |
| Dr. N. Tropy | warp room 4 | Death Tornado Spin (`Upgrade_DeathTornadoSpin`) | [Crashmania powers]; worked in step2 and T3 | `C3_NTropy_Hub3_Intro`, `C3_NTropy_PreFight`, `C3_NTropy_Taunt`, `C3_Cortex_Post_NTropy_Fight` |
| N. Gin | warp room 5 | Fruit Bazooka (`Upgrade_FruitBazooka`) | set by the game in `test_saves/t4d_after.sav` | `C3_NGin_Intro`, `C3_PreNGinFight`, `C3_PostNGinFight` |
| Dr. Neo Cortex | ending | Speed Shoes (`Upgrade_SpeedShoes`) | [Crashmania powers] ("Crash Dash"); worked in T3 | `C3_Cortex_Taunt_2`, `C3_PreFinalBattle` |

- **Abilities are stored only in these variables**, independently of the boss bits. In T3, Death Tornado Spin still worked with N. Tropy undefeated, Double Jump could be turned off, and the Fruit Bazooka and Speed Shoes worked when turned on early.
- **Scene steps.** A step holds the scenes that play between the previous boss's defeat and this boss's. The game's own saves show where each one is first marked: `C3_Tiny_Intro` in 0007, `C3_Tiny_PreBossFight` and `C3_Tiny_PostBossFight` in 0009, `C3_Hub2_Intro` in 0010, `C3_Dingodile_Intro` in 0015, `C3_Dingodile_PreBossFight` in 0017, `C3_NTropy_Hub3_Intro` in 0018, `C3_PreNGinFight` and `C3_PostNGinFight` in `t4d_after.sav`, `C3_PreFinalBattle` in `t6_after.sav`.
- The other names in the table, and `C3_AkuAku_Warning` below, come from a 108% Switch save published by another player ("Crash Bandicoot N Sane Trilogy 100% Save" by championgamer, April 2026), which is not included in this repository. Marking every scene as seen stopped all of them from playing (T8).

## 4. Zone sessions

`CZoneInfoSaveList` lists `CZoneInfoSave` objects. Each one holds a zone path string and a reference to that zone's `CZoneInfoUserSession`. There is one session per map, keyed by the zone path of the [level table](#level-table).

### Session fields

The names are ours. They describe what the saves show.

| # | Type | Name | Meaning | Evidence |
|---|---|---|---|---|
| 0 | uint | `unknown_0` | Always 0. Preserved. | All observed saves |
| 1 | int | `unknown_1` | Always 0. Preserved. | All observed saves |
| 2 | enum | `state` | -1 = never entered, 1 = entered. No other value is observed. | 0001–0019 (Toad Village -1 in `initial_game.sav`, 1 from 0002) |
| 3 | int | `completions` | 0 until the level is first left, then 1, whether it was finished or quit. After that, **only finishing adds 1**: quitting, breaking a crate, collecting fruit or losing a life leave it unchanged. It is **not** a play count. | 0002 (Toad Village 0 → 1, quit after one box), 0003 (1 → 2, finished), 0018, R7 (steps 1–4 unchanged, step 5 +1) |
| 4 | int | `unknown_4` | Always 0. Preserved. | All observed saves |
| 5–7 | int | `time_1`…`time_3` | Time trial leaderboard in centiseconds (6023 = 1:00.23), best first, 0 = empty slot | 0010, 0011 (1:00.23, then 0:55.44 above it) |
| 8–10 | string | `initials_1`…`initials_3` | `""` for a player's time, `"---"` for an empty slot | 0010, 0011 |
| 11 | int | `relic_tier` | 0 none, 1 sapphire, 2 gold, 3 platinum | 0010 (1), 0011 (2), T1 (3) |
| 12 | bool | `crystal` | Equal to the `CRYSTAL` bit in every observed record (6,264 sessions in the 54 saves). Finishing a level without its crystal leaves it 0. | All observed saves; 0018 (Double Header finished without the crystal) |
| 13 | uint | `collectibles` | The bit field below | |
| 14 | bool | `unknown_14` | Always 0. Preserved. | All observed saves |

### Collectible bits (field 13)

| Bit | Name | Meaning | Evidence |
|---|---|---|---|
| `0x0002` | `GEM_CLEAR` | Clear gem for breaking every box | 0004, 0005 |
| `0x0004` | `GEM_BLUE` | Blue gem (Tomb Wader) | T2 |
| `0x0008` | `GEM_GREEN` | Green gem (Flaming Passion) | T2 (shown on the save slot) |
| `0x0010` | `GEM_PURPLE` | Purple gem (High Time) | T2 |
| `0x0020` | `GEM_RED` | Red gem (Deep Trouble) | step2, T2 |
| `0x0040` | `GEM_YELLOW` | Yellow gem (Hang'em High) | T2; written by the game in `t5_after.sav` |
| `0x0100` | `GEM_SECOND` | Second clear gem: the one that does not come from boxes | Bone Yard in 0007–0019 (see [Second gems](#second-gems)); T2 |
| `0x0200` | `CRYSTAL` | Crystal (levels 1–25) | 0003 |
| `0x0800` | `BOSS_DEFEATED` | Boss defeated (boss zones) | 0009, 0017, `t4d_after.sav` |
| `0x1000` | `RELIC_PLATINUM` | Platinum relic | T1 |
| `0x2000` | `RELIC_SAPPHIRE` | Sapphire relic | 0010 |
| `0x4000` | `RELIC_GOLD` | Gold relic | 0011 |
| every other bit | – | Never observed; preserved | All observed saves: the union of every session's bits is `0x7b7e` |

- **One relic bit at a time.** A better relic replaces the lower one: 0011 has `RELIC_GOLD` and no `RELIC_SAPPHIRE`. The bit always agrees with `relic_tier` in the game's saves.
- **The game does not check times against relics.** A sapphire with a 0:55.44 time, a platinum with no time, and three times with no relic all loaded and showed as written (T1).
- **The time trial needs the crystal.** In T3, removing Toad Village's crystal also removed its time trial.

## 5. Variables and flags

### Game variables

`CGameVariableStoreSaveData` holds `CGameVariableValueSaved{Bool,Int,Float}` objects, each a `(value, handle)` pair. The handle's hash is resolved through the handle name table ([§2](#2-container-alchemy-igb)).

- **Abilities** (`Upgrade_*`) and **colored gems** (`Gem_<Color>`): 0 or 1, see [§3](#3-levels).
- **Everything else is never written by the editor.** For example, the game changes `C2_CrystalsFailCount` on each first boss win (0009, 0017, `t4d_after.sav`), and `C3_Hub_Progression` is 0 in all observed saves.

### Flag table (`igStringBoolHashTable`)

| Key | Meaning | Evidence |
|---|---|---|
| `<Name>_RelicCounted` | One per level holding a relic of any tier, with the game's spelling of the name (`Hang'em High`, `Area 51?`, `Dino Might!`, `Makin' Waves`…). Removed when the relic is removed. | Added with Toad Village's relic in 0010; T1, T3 |
| `LastGemRewarded` | The bonus gem, the 47th gem. It counts in `gem_count` and in the percentage. | Set by the game when the gem was picked up in T7 (`t7_after_beating_cortex.sav`) |
| `OneHundredCortexBeat` | Set when Cortex is beaten again at 108%, which plays the secret ending | T6 (`t6_after.sav`), T7 (`t7_after_beating_cortex.sav`) |
| `l300_hub_talkingheadcutscenes.C3_<Scene> (archetype l300_hub_talkingheadcutscenes.C3_<Scene>_entityData)` | A hub story scene already seen; the whole string is the key. **A scene marked as seen doesn't play** (T8). Skipping a story step by editing leaves its scenes to play later, which is harmless (T6). | See [Bosses and abilities](#bosses-and-abilities) |
| `HintDisplay_<n>` | In-level hints already shown. The editor never writes them. | Added by the game in `t4d_after.sav` |

The cutscene `start` step groups three scenes that belong to no boss: `C3_Cortex_Taunt_1` (seen in 0001), `C3_AkuAku_TimeTrialInfo` (seen in 0004, when the time trials were introduced), and `C3_AkuAku_Warning`, which no save the game wrote here has.

The **int hash table** (`igStringIntHashTable`) holds checkpoint and object state inside levels (`<level>.PlayerStartAll`, `Crate_Checkpoint_…`). The game added to it in R7 after a lost life and after a finish. The editor never touches it.

## 6. Summary table

The table at the top of the file holds what the save slot screen shows. The game displays the stored values and recomputes nothing on load: every count shown in the live tests matched what the editor wrote (24% in step2, 108% in T6).

| Key | Type | Meaning | Evidence |
|---|---|---|---|
| `save_version` | int | 75 | All observed saves |
| `farthest_chapter` | int | Always 0 in Crash 3 saves; left unchanged | All observed saves |
| `farthest_progression_index` | int | Furthest story point reached; rule below | 0001–0019 |
| `previous_level_progression_index` | int | Progression index of the last zone entered; -1 in a fresh save. Updated even by an immediate quit. | `initial_game.sav`, R7 step 1 |
| `previous_level_name` | string | Zone path of the last zone entered. **Absent from a fresh save.** When the game saves a file that has no last level (`previous_level_progression_index` −1) before any level is entered, it can write a few meaningless bytes here instead (5 in `l2_after.sav`); the game loaded fine before that save. | `initial_game.sav`, R7, R8 |
| `credits_watched` | int | Always 0 in Crash 3 saves; left unchanged. It stayed 0 after the ending (T7) and after watching the credits from the menu (R7). | All observed saves |
| `story_completed` | int | 1 once Cortex is beaten | `t7_after_beating_cortex.sav` |
| `active_progression_index` | int | 1 in 0001, 0 in every other observed save, including all saves made in the hub in R7. Left unchanged. | All observed saves |
| `percent_complete` | float | crystals + gems + relics (any tier) + bosses defeated | All observed saves |
| `life_count` | int | **The only place lives are stored.** The game reads it back on load: 42 in lives42 loaded with 42 lives, and dying gave 41 (0019). 255 loads and shows as 255, and dying gives 254; at 254, picking up a life (twice) left the counter at 254, so the game adds no life above some maximum, not yet found. | lives42, PC1, R8 |
| `key_count` | int | Always 0 in Crash 3 saves; left unchanged | All observed saves |
| `crystal_count`, `gem_count`, `sapphire_count`, `gold_count`, `platinum_count` | int | Totals | All observed saves |

**Percentage.** The maximum is 25 crystals + 47 gems + 31 relics + 5 bosses = **108%** (T6, T7). The 47 gems are 31 clear gems, 10 second gems, 5 colored gems and the bonus gem. The original game counted 105% [PowerPyx gems]; the N. Sane version adds Future Tense, with two gems and a relic.

**`farthest_progression_index` rule.**

- **Value:** the progression index of the furthest zone entered (`state` 1), plus one once its `completions` is at least 1, capped at 30. 0 if nothing was entered.
- **Accuracy:** it reproduces every save the game wrote, except 0001, where the game stored 1 for a rule value of 0. So the editor only ever raises the stored value.
- **When it changes:** the game raises it when a level is finished, and never recomputes it on load.
- **Why the editor raises it:** so that an edited save carries the value the game would have written. It does **not** open boss portals: T4b (index 24) showed no N. Gin portal, and T4c (index 30) glitched like T4 (index 18). See [§7](#7-game-rules).

## 7. Game rules

These rules come from live tests and guides. The save has no storage of its own for them.

- **Boss portals.** The portal of room N's boss appears once the **total** crystal count reaches 5 × N (Tiny Tiger 5, Dingodile 10, N. Tropy 15, N. Gin 20, Cortex 25). It doesn't matter which levels the crystals come from: T4f had 20 crystals with room 4 incomplete, and N. Gin's portal worked.
- **Portal glitch.** With **more** than 5 × N crystals while boss N is alive, his portal flickers over the room's centre level and can't be entered. Normal play can't produce this state, because room N+1 stays locked until boss N is beaten. The editor warns about it.

  | Save | Crystals per room | Total | Result |
  |---|---|---|---|
  | T4, T4c | 5/5/2/5/5 | 22 | glitch |
  | T4h | 5/5/5/5/2 | 22 | glitch |
  | T4d, T4e | 5/5/5/5/0 | 20 | portal works |
  | T4f | 5/5/5/4/1 | 20 | portal works |
  | T4g | 5/5/2/5/3 | 20 | portal works |
  | T4b | 5/5/2/5/0 | 17 | no portal |

- **Warp rooms.** Room N+1 opens when room N's boss has `BOSS_DEFEATED` (step2: N. Tropy set, room 4 open; T3: cleared, room 4 locked again).
- **Secret warp room.** Entered from the orange pad in the hub centre [PowerPyx gems]. Its levels open with the **total relic count**, any tier: 26 needs 5, 27 needs 10, 28 needs 15, 29 needs 20, 30 needs 25 [Wikipedia: the first five relics open a level of the secret warp room, and every five more open another]. In game: 5 relics opened 26 only (T1); 20 opened 26–29 with 30 locked (T5).
- **Hot Coco**: through the alien road sign in Road Crash [PowerPyx Hot Coco]; worked in T6.
- **Eggipus Rex**: through the yellow gem path in Dino Might! [PowerPyx Eggipus Rex]; reached in T2.
- **Future Tense**: from the pad above the hub centre, playable at any point in the game [Press Start Future Tense, Wiki Future Tense]; worked in T6.
- **Bonus gem.** Observed: with every other gem and every relic gold or better (107%), the game spawned the 47th gem in the central warp room; picking it up gave 108% and set `LastGemRewarded` (T7). Guides give the same condition: "all gems and gold / platinum relics" [PowerPyx gems]. With 30 gold relics and one sapphire (still 107%), the gem spawned too, and picking it up set `LastGemRewarded` (R8). The editor's "bonus gem due" indicator uses gold-or-better relics ≥ 30, percentage ≥ 107 and the flag not set. Whether fewer than 30 are enough was not tested.
- **Secret ending.** Beating Cortex again at 108% plays it and sets `OneHundredCortexBeat` (T6, T7).

## 8. What an edit must write

The game keeps whatever it reads. After every live test, its next save kept the edits, including the removals (T3). An edited save therefore has to be self-consistent:

| Edit | Writes |
|---|---|
| Turning anything on, or writing a non-empty times list | If the level was never entered: `state` = 1 and `completions` = 1, as the game writes after a first visit (0002, R7). Turning things off and removing a relic don't mark the level entered. |
| Crystal | `CRYSTAL` **and** field 12 `crystal` |
| Clear gem / second gem | `GEM_CLEAR` / `GEM_SECOND` |
| Colored gem | Its zone bit **and** its `Gem_<Color>` variable (`t5_after.sav`) |
| Relic | `relic_tier`, exactly one relic bit (the other two cleared), and the `<Name>_RelicCounted` flag (removed with the relic) |
| Times | Up to 3, sorted best first, in `time_1`…`time_3`; `""` initials per time and `"---"` per empty slot |
| Boss | `BOSS_DEFEATED`. The ability and the story scenes are separate edits. |
| Ability | Its `Upgrade_*` variable, 0 or 1 |
| Bonus gem | The `LastGemRewarded` flag |
| Cutscene | Insert or remove its flag key |
| Lives | `life_count` |
| On every write | Recompute `percent_complete` and the five counts; raise `farthest_progression_index` to the rule's value; `story_completed` = Cortex defeated |

Inserting a flag can insert a string, which re-sorts the string table and renumbers references. It can also grow a hash table, which resizes memory blocks and changes the directory and the header ([§2](#2-container-alchemy-igb)).

`t4d_after.sav` shows the game writing exactly the editor's edits when N. Gin is beaten for real: the boss bit, the Fruit Bazooka, `C3_PreNGinFight` and `C3_PostNGinFight`, and the progression index raised to 25. It also changed a counter variable and a hint flag, which the editor leaves alone.

## 9. Relic time thresholds

A time trial time **under** a threshold earns that relic: the game showed "less than 1:03.00" for Toad Village's sapphire and gave it for 1:00.23 (0010), then showed "less than 0:57.50" for gold and gave it for 0:55.44 (0011). Times are in centiseconds, so the rule is `time < threshold`. The bosses have no time trial.

The editor uses the values below. Each value lists the public sources that give the same value. "All" means every source that covers that tier: W, L, S and Q for every tier, H for gold and platinum, P for gold.

| Tag | Source |
|---|---|
| W | Crash Bandicoot wiki, N. Sane Trilogy page, relic table for every level: <https://crashbandicoot.fandom.com/wiki/Crash_Bandicoot_N._Sane_Trilogy> |
| L | Crash Bandicoot wiki, each level's page (its N. Sane Trilogy relic times): `https://crashbandicoot.fandom.com/wiki/<Level_name>`, for example <https://crashbandicoot.fandom.com/wiki/Toad_Village> |
| S | "Crash Bandicoot: N. Sane Trilogy – Relic times chart" by CrashRatchet008 (v1.07.2): <https://docs.google.com/spreadsheets/d/1qXiCwTyVdOsB8yBs-T9lZBTsx9iY8R_GurFtYAOjeLE>. It marks Hot Coco and Eggipus Rex as approximate. |
| Q | Steam guide "Crash Bandicoot: Warped: Quick Time Trial Reference": <https://steamcommunity.com/sharedfiles/filedetails/?id=1443157942>. It credits S, and marks Hot Coco and Eggipus Rex as approximate. |
| H | Steam guide "Crash Bandicoot N. Sane Trilogy 100% completion – short text guide" (gold and platinum only): <https://steamcommunity.com/sharedfiles/filedetails/?id=3430571654> |
| P | PowerPyx, "Crash Bandicoot 3: Warped – Time Trial Speedruns Guide (Gold Relics)" (gold only, no Future Tense): <https://www.powerpyx.com/crash-bandicoot-3-warped-time-trial-speedruns-guide-gold-relics/> |
| game | Shown in game: during the playthrough (0010, 0011), and on the PC time panels in R8 |

These sources partly copy each other (Q credits S), so agreement between them is not fully independent.

On PC, a level's time panel shows the time of the **next** relic only: the sapphire time with no relic, the gold time with sapphire, and so on. R8 put four levels one relic below the value to read. The panels showed Deep Trouble gold **1:15.12**, Double Header gold **1:21.12** and Future Frenzy platinum **1:19.62**, the values most sources give, and Sphynxinator gold 1:22.63, as every source gives. For the first three, the editor's threshold is one centisecond above the panel, with the strict rule `time < threshold`: a time of 1:15.12 or faster earns gold. A run timed to exactly that hundredth was not tried. Hot Coco and Eggipus Rex have no time panel or leaderboard in game, so their values can't be read there.

| Level | Platinum | Gold | Sapphire |
|---|---|---|---|
| Toad Village | 0:44.03 · all | 0:57.50 · all, game | 1:03.00 · all, game |
| Under Pressure | 1:03.50 · all | 1:10.90 · all | 1:39.00 · all |
| Orient Express | 0:18.10 · all | 0:27.80 · all | 0:41.00 · all |
| Bone Yard | 1:21.00 · all | 1:40.20 · all | 1:45.00 · all |
| Makin' Waves | 0:56.23 · all | 1:03.23 · all | 1:11.00 · all |
| Gee Wiz | 1:05.90 · all | 1:22.73 · all | 1:35.00 · all |
| Hang'em High | 0:43.80 · all | 0:52.63 · all | 1:24.00 · all |
| Hog Ride | 0:38.06 · all | 0:44.43 · all | 0:48.00 · all |
| Tomb Time | 0:53.90 · all | 1:10.00 · W L S H P (Q: 1:10.26) | 1:42.00 · all |
| Midnight Run | 0:18.20 · L S Q H (W: 0:18.10) | 0:38.23 · all | 0:51.00 · all |
| Dino Might! | 1:03.00 · all | 1:25.76 · all | 1:34.00 · all |
| Deep Trouble | 1:08.33 · all | 1:15.13 · L H; W S Q P: 1:15.12. Panel: 1:15.12 (R8) | 1:37.00 · all |
| High Time | 0:56.93 · all | 1:04.10 · all | 2:12.00 · all |
| Road Crash | 1:23.10 · all | 1:26.70 · all | 1:31.00 · all |
| Double Header | 0:59.40 · all | 1:21.13 · all sources: 1:21.12. Panel: 1:21.12 (R8) | 1:27.00 · all |
| Sphynxinator | 0:56.70 · all | 1:22.63 · all; game (R8) | 1:42.00 · all |
| Bye Bye Blimps | 0:31.50 · all | 0:38.40 · all | 0:49.00 · all |
| Tell No Tales | 1:15.23 · all | 1:35.63 · all | 2:12.00 · all |
| Future Frenzy | 1:19.63 · all sources: 1:19.62. Panel: 1:19.62 (R8) | 1:34.00 · all | 2:01.00 · all |
| Tomb Wader | 1:24.00 · all | 1:45.03 · all | 2:44.00 · all |
| Gone Tomorrow | 1:02.10 · all | 1:25.60 · all | 2:05.00 · all |
| Orange Asphalt | 1:23.80 · all | 1:33.30 · all | 1:38.00 · all |
| Flaming Passion | 0:59.40 · all | 1:13.10 · all | 1:43.00 · all |
| Mad Bombers | 0:58.13 · all | 1:15.23 · all | 2:18.00 · all |
| Bug Lite | 1:14.90 · all | 1:34.86 · all | 1:49.00 · all |
| Ski Crazed | 0:36.30 · all | 0:53.50 · all | 1:19.00 · all |
| Area 51? | 1:52.50 · all | 1:57.80 · all | 2:01.00 · all |
| Rings of Power | 1:01.73 · all | 1:11.46 · all | 1:30.00 · all |
| Hot Coco | 0:19.96 · W S Q; L H: 0:19.93. Not shown in game | 0:30.10 · W L S Q H (P: 0:30.08) | 1:00.00 · all |
| Eggipus Rex | 0:47.83 · W: 0:46.83; L S Q H: 0:47.74. Not shown in game | 0:53.03 · W L H: 0:52.03; S Q: 0:53.01; P: 0:50.00. Not shown in game | 0:58.00 · Q (approximate); W L: 0:57.00; S: 0:59.00. Not shown in game |
| Future Tense | 0:52.17 · all | 1:15.43 · all | 2:01.52 · all |

The isolated differences (Tomb Time gold in Q, Midnight Run platinum in W, Hot Coco gold in P) disagree with every other source and look like typing errors.

## 10. Live test log

The tests ran on a Switch (step2, T1–T8, R7, lives42, most of R8) and on PC (PC1, PC2, two R8 files). Each test save has its own life count, so the loaded file can be confirmed on the save slot.

| Test | File | Edit | Result in game |
|---|---|---|---|
| lives42 | `test_saves/lives42.sav` | 0018 with lives 5 → 42 | 42 lives; dying gave 41 (0019) |
| step2 | `test_saves/step2.sav` | Deep Trouble: crystal, clear gem, red gem, gold relic, 2 times. Rooms up to 4 unlocked, with abilities. | All shown. The red platform works in Bone Yard. N. Tropy counts as beaten, room 4 is open, the spin works. The slot shows 24%. |
| T1 | `test_saves/t1_relics.sav` | Sapphire, platinum, a downgrade from gold, a relic without times, times without a relic | All shown as edited. 5 relics: the secret room offers level 26 only. |
| T2 | `test_saves/t2_gems.sav` | Second gems, a clear gem, all colored gems, including in never-entered levels | All shown. The red, purple, yellow and blue platforms work; Eggipus Rex is reachable. |
| T3 | `test_saves/t3_removal.sav` | Crystal, gem, relic and times off; red gem off; N. Tropy off; Double Jump off; Fruit Bazooka and Speed Shoes on | All applied. Toad Village lost its time trial. Room 4 locked again. The game kept every change on its next save (`t3_after.sav.sav`). |
| T4 | `test_saves/t4_crystals.sav` | Crystals in 15–25 with N. Gin alive: 22 crystals | N. Gin's portal glitches over level 18 |
| T4b | `test_saves/t4b_room4.sav` | Room 4 complete, 17 crystals, index 24 | No N. Gin portal |
| T4c | `test_saves/t4c_room5.sav` | T4b plus room 5's crystals: 22, index 30 | Room 5 stays locked; glitch |
| T4d | `test_saves/t4d_rooms1to4.sav` | 5/5/5/5/0 crystals | N. Gin's portal works. Beating him wrote `t4d_after.sav`. |
| T4e | `test_saves/t4e_rooms1to4_intro.sav` | T4d plus `C3_NGin_Intro` seen | Same as T4d |
| T4f | `test_saves/t4f_mixed20.sav` | 5/5/5/4/1 crystals | N. Gin's portal works |
| T4g | `test_saves/t4g_room3_short.sav` | 5/5/2/5/3 crystals | Portal works |
| T4h | `test_saves/t4h_total22.sav` | 5/5/5/5/2 crystals | Glitch |
| T5 | `test_saves/t5_secret_room.sav` | 20 gold relics; the flag table grows from 64 to 128 slots | Loads. Secret levels 26–29 open, 30 locked. The yellow gem picked up for real: `t5_after.sav`. |
| T6 | `test_saves/t6_full108.sav` | Everything (108%) | Everything works. Story scenes play once. Beating Cortex again plays the secret ending (`t6_after.sav`). |
| T7 | `test_saves/t7_bonus_gem.sav` | Everything, every relic gold, no bonus gem (107%) | The 47th gem spawns in the central warp room, with fireworks; picking it up gives 108%. Beating Cortex again plays the secret ending (`t7_after_beating_cortex.sav`). |
| T8 | `test_saves/t8_full108_scenes.sav` | T6 with every story scene marked as seen | No scene plays |
| R7 | `test_saves/r7_base.sav` → `r7a1.sav`–`r7a5.sav` | Under Pressure: quit at once, break a crate, collect fruit, lose a life, finish | `completions` rises only on the finish. `previous_level_*` follow the last level entered. Credits watched from the menu: the save didn't change. |
| PC1 | `test_saves/pc/pc1_lives42.sav` | Fresh PC save, lives 4 → 42, same size | 42 lives |
| PC2 | `test_saves/pc/pc2_full108.sav` | Everything; the file grows from 64,781 to 68,318 bytes | 108%, everything open, no story scenes |
| R8 | `test_saves/round8/l1_switch_to_pc.sav` (PC) | 0019 with 61 lives, converted to PC with the fresh PC save as template | Same progress as 0019: totals, rooms, abilities, scenes. The game's save after finishing Toad Village (`l1_after.sav`) changes only Toad Village's `completions`, 5 → 6. |
| R8 | `test_saves/round8/l2_pc_to_switch.sav` | Fresh PC save made 108% (platinum, a time just under each threshold, scenes seen), 62 lives, converted to Switch with the fresh Switch save as template | 108%, everything open, no scene plays. The game's save in the hub (`l2_after.sav`) keeps all game data and adds meaningless bytes as `previous_level_name` (§6). |
| R8 | `test_saves/round8/l3_reset_level.sav` | 0019 with Under Pressure reset to never played, 63 lives | Under Pressure shows as never played: no crystal, gem, relic or time trial; totals 9 crystals, 5 gems; nothing else changed. Nothing special on the next visit. Finishing it (`l3_after.sav`) writes a normal first visit: state 1, `completions` 1, crystal. |
| R8 | `test_saves/round8/l4_story_up_to_3.sav` | Fresh Switch save with rooms 1–3's crystals, the first three bosses with their abilities and scenes, 64 lives | Rooms 1–4 open, no N. Gin portal (15 crystals), no glitch, no scene plays, the three abilities work. `l4_after.sav`: Sphynxinator finished on a first visit. |
| R8 | `test_saves/round8/l5_relics_matching.sav` | 0019 with gold on every level and a time just under gold where none earned it, 65 lives | 31 gold relics; every secret level opens. The game's save (`l5_after.sav`) changes nothing. |
| R8 | `test_saves/round8/l6_relic_panels_pc.sav` (PC) | PC2 with Deep Trouble, Double Header and Sphynxinator at sapphire and Future Frenzy at gold, 66 lives | The time panels show the next relic's time: see §9 |
| R8 | `test_saves/round8/l7_bonus_gem_30.sav` | T7 with Future Tense at sapphire: 30 gold relics and 1 sapphire, 107%, 67 lives | The 47th gem spawns; picking it up gives 108% and sets `LastGemRewarded` (`l7_after.sav`). |
| R8 | `test_saves/round8/l8_lives_255.sav` | 0019 with 255 lives | 255 shown; dying gives 254; at 254, picking up a life twice leaves 254 (`l8_after.sav`). |

`test_saves/t7_after_picking_47th_gem.sav` is byte-identical to `t7_bonus_gem.sav` (the wrong file was copied back); `t7_after_beating_cortex.sav` shows the gem taken. `test_saves/r7_racegems.sav` (both gems of Area 51? and Rings of Power removed) was prepared but not played.

## 11. Save log

One Switch playthrough, saved in the hub after each step.

| Save | What was played |
|---|---|
| `initial_game.sav` | After the opening scene; only warp room 1 open |
| `0001.sav` | Entered Toad Village (a Cortex scene played) and quit at once |
| `0002.sav` | Toad Village: one box broken, no crystal, quit |
| `0003.sav` | Toad Village: crystal, quit |
| `0004.sav` | Toad Village: every box (clear gem); a scene introduced the time trials |
| `0005.sav` | Under Pressure: crystal and clear gem |
| `0006.sav` | Orient Express: crystal |
| `0007.sav` | Bone Yard: crystal and the box gem (a Tiny Tiger scene played first) |
| `0008.sav` | Makin' Waves: crystal and clear gem; only the boss is left in room 1 |
| `0009.sav` | Tiny Tiger beaten; Super Charged Body Slam; room 2 opens |
| `0010.sav` | Toad Village time trial: 1:00.23, sapphire. The game showed the sapphire threshold as 1:03.00. |
| `0011.sav` | Toad Village time trial: 0:55.44, gold. The game showed the gold threshold as 0:57.50. |
| `0012.sav` | Gee Wiz: crystal and clear gem |
| `0013.sav` | Hang'em High: crystal and clear gem |
| `0014.sav` | Hog Ride: crystal |
| `0015.sav` | Tomb Time: crystal (a Dingodile scene played first) |
| `0016.sav` | Midnight Run: crystal; only the boss is left in room 2 |
| `0017.sav` | Dingodile beaten; Double Jump; room 3 opens |
| `0018.sav` | Double Header finished without its crystal |
| `0019.sav` | `test_saves/lives42.sav` loaded (42 lives), Toad Village entered, one life lost, quit |

## Sources

| Tag | URL |
|---|---|
| PowerPyx gems | <https://www.powerpyx.com/crash-bandicoot-3-warped-ps4-all-gem-locations-guide/> |
| PowerPyx Eggipus Rex | <https://www.powerpyx.com/crash-bandicoot-3-warped-how-to-unlock-eggipus-rex-secret-level> |
| PowerPyx Hot Coco | <https://www.powerpyx.com/crash-bandicoot-3-warped-how-to-unlock-hot-coco-secret-level/> |
| Wikipedia | <https://en.wikipedia.org/wiki/Crash_Bandicoot:_Warped> |
| Crashmania powers | <https://www.crashmania.net/en/encyclopedia/super-powers/> |
| Press Start Future Tense | <https://press-start.com.au/news/nintendo/2018/06/12/crash-bandicoot-n-sane-trilogy-is-getting-its-first-entirely-new-level/> |
| Wiki Future Tense | <https://crashbandicoot.fandom.com/wiki/Future_Tense> |
