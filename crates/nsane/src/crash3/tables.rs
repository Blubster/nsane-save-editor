//! Static Crash 3 tables: levels, bosses, story order, secret warp room,
//! relic times, abilities and hub story scenes, plus the session layout.
//! Everything here follows `docs/FORMAT.md` (§3–§5, §9).

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Level,
    Boss,
    /// Hot Coco, Eggipus Rex and Future Tense: no number, reached by other means.
    Secret,
}

impl Kind {
    pub fn key(self) -> &'static str {
        match self {
            Kind::Level => "level",
            Kind::Boss => "boss",
            Kind::Secret => "secret",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GemColor {
    Red,
    Yellow,
    Purple,
    Blue,
    Green,
}

impl GemColor {
    pub const ALL: [GemColor; 5] = [
        GemColor::Red,
        GemColor::Yellow,
        GemColor::Purple,
        GemColor::Blue,
        GemColor::Green,
    ];

    pub fn key(self) -> &'static str {
        match self {
            GemColor::Red => "red",
            GemColor::Yellow => "yellow",
            GemColor::Purple => "purple",
            GemColor::Blue => "blue",
            GemColor::Green => "green",
        }
    }

    pub fn from_key(key: &str) -> Option<GemColor> {
        GemColor::ALL.into_iter().find(|c| c.key() == key)
    }

    /// The zone bit of the level the gem is found in.
    pub fn bit(self) -> u32 {
        match self {
            GemColor::Red => bit::GEM_RED,
            GemColor::Yellow => bit::GEM_YELLOW,
            GemColor::Purple => bit::GEM_PURPLE,
            GemColor::Blue => bit::GEM_BLUE,
            GemColor::Green => bit::GEM_GREEN,
        }
    }

    /// The game variable that makes the gem's platforms appear.
    pub fn variable(self) -> &'static str {
        match self {
            GemColor::Red => "Gem_Red",
            GemColor::Yellow => "Gem_Yellow",
            GemColor::Purple => "Gem_Purple",
            GemColor::Blue => "Gem_Blue",
            GemColor::Green => "Gem_Green",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Relic {
    Sapphire = 1,
    Gold = 2,
    Platinum = 3,
}

impl Relic {
    pub const ALL: [Relic; 3] = [Relic::Sapphire, Relic::Gold, Relic::Platinum];

    /// The value of the `relic_tier` session field.
    pub fn tier(self) -> i32 {
        self as i32
    }

    /// `None` for 0 (no relic) and for any value the game never writes.
    pub fn from_tier(tier: i32) -> Option<Relic> {
        Relic::ALL.into_iter().find(|r| r.tier() == tier)
    }

    pub fn key(self) -> &'static str {
        match self {
            Relic::Sapphire => "sapphire",
            Relic::Gold => "gold",
            Relic::Platinum => "platinum",
        }
    }

    pub fn from_key(key: &str) -> Option<Relic> {
        Relic::ALL.into_iter().find(|r| r.key() == key)
    }

    pub fn name(self) -> &'static str {
        match self {
            Relic::Sapphire => "Sapphire",
            Relic::Gold => "Gold",
            Relic::Platinum => "Platinum",
        }
    }

    pub fn bit(self) -> u32 {
        match self {
            Relic::Sapphire => bit::RELIC_SAPPHIRE,
            Relic::Gold => bit::RELIC_GOLD,
            Relic::Platinum => bit::RELIC_PLATINUM,
        }
    }
}

/// The five super powers. They are stored only in their variables, apart from
/// the boss bits (§3, "Bosses and abilities").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Ability {
    BodySlam,
    DoubleJump,
    TornadoSpin,
    Bazooka,
    SpeedShoes,
}

pub const ABILITIES: [Ability; 5] = [
    Ability::BodySlam,
    Ability::DoubleJump,
    Ability::TornadoSpin,
    Ability::Bazooka,
    Ability::SpeedShoes,
];

impl Ability {
    pub fn key(self) -> &'static str {
        match self {
            Ability::BodySlam => "bodyslam",
            Ability::DoubleJump => "doublejump",
            Ability::TornadoSpin => "tornadospin",
            Ability::Bazooka => "bazooka",
            Ability::SpeedShoes => "speedshoes",
        }
    }

    pub fn from_key(key: &str) -> Option<Ability> {
        ABILITIES.into_iter().find(|a| a.key() == key)
    }

    pub fn name(self) -> &'static str {
        match self {
            Ability::BodySlam => "Super Charged Body Slam",
            Ability::DoubleJump => "Double Jump",
            Ability::TornadoSpin => "Death Tornado Spin",
            Ability::Bazooka => "Fruit Bazooka",
            Ability::SpeedShoes => "Speed Shoes",
        }
    }

    pub fn variable(self) -> &'static str {
        match self {
            Ability::BodySlam => "Upgrade_SuperChargedBodySlam",
            Ability::DoubleJump => "Upgrade_DoubleJump",
            Ability::TornadoSpin => "Upgrade_DeathTornadoSpin",
            Ability::Bazooka => "Upgrade_FruitBazooka",
            Ability::SpeedShoes => "Upgrade_SpeedShoes",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Level {
    /// `None` for bosses and the three unnumbered secret levels.
    pub number: Option<u8>,
    /// Short id, used in edit ops.
    pub key: &'static str,
    /// The game's spelling, as stored in `<Name>_RelicCounted` flag keys.
    pub name: &'static str,
    /// Zone path of the level's session.
    pub zone: &'static str,
    /// 1–5, or 6 for the secret warp room.
    pub warp_room: u8,
    pub kind: Kind,
    pub second_gem: bool,
    pub colored_gem: Option<GemColor>,
}

impl Level {
    /// Only levels 1–25 have a crystal.
    pub fn has_crystal(&self) -> bool {
        self.number.is_some_and(|n| n <= 25)
    }

    /// Every level but the bosses has a clear gem, a time trial and a relic.
    pub fn has_relic(&self) -> bool {
        self.kind != Kind::Boss
    }
}

const fn level(
    number: Option<u8>,
    key: &'static str,
    name: &'static str,
    zone: &'static str,
    warp_room: u8,
) -> Level {
    Level {
        number,
        key,
        name,
        zone,
        warp_room,
        kind: Kind::Level,
        second_gem: false,
        colored_gem: None,
    }
}

const fn n(number: u8) -> Option<u8> {
    Some(number)
}

const fn boss(key: &'static str, name: &'static str, zone: &'static str, warp_room: u8) -> Level {
    Level {
        kind: Kind::Boss,
        ..level(None, key, name, zone, warp_room)
    }
}

const fn secret(key: &'static str, name: &'static str, zone: &'static str) -> Level {
    Level {
        kind: Kind::Secret,
        ..level(None, key, name, zone, 6)
    }
}

const fn second(lv: Level) -> Level {
    Level {
        second_gem: true,
        ..lv
    }
}

const fn colored(lv: Level, color: GemColor) -> Level {
    Level {
        colored_gem: Some(color),
        ..lv
    }
}

/// Every level, boss and secret level, in warp room order (§3, "Level table").
/// Levels 27 and 29 are second entrances into levels 7 and 19 and share their
/// sessions, so they have no entry here (see [`SECRET_ROOM`]).
#[rustfmt::skip]
pub static LEVELS: [Level; 36] = [
    level(n(1), "toadvillage", "Toad Village", "crash3/l301_toadvillage/l301_toadvillage", 1),
    level(n(2), "underpressure", "Under Pressure", "crash3/l302_underpressure/l302_underpressure", 1),
    level(n(3), "orientexpress", "Orient Express", "crash3/l303_orientexpress/l303_orientexpress", 1),
    second(level(n(4), "boneyard", "Bone Yard", "crash3/l304_boneyard/l304_boneyard", 1)),
    level(n(5), "makinwaves", "Makin' Waves", "crash3/l305_makinwaves/l305_makinwaves", 1),
    boss("tinytiger", "Tiny Tiger", "crash3/bosses/b301_tinytiger/b301_tinytiger", 1),
    level(n(6), "geewiz", "Gee Wiz", "crash3/l306_geewiz/l306_geewiz", 2),
    colored(level(n(7), "hangemhigh", "Hang'em High", "crash3/l307_hangemhigh/l307_hangemhigh", 2), GemColor::Yellow),
    level(n(8), "hogride", "Hog Ride", "crash3/l308_hogride/l308_hogride", 2),
    second(level(n(9), "tombtime", "Tomb Time", "crash3/l309_tombtime/l309_tombtime", 2)),
    level(n(10), "midnightrun", "Midnight Run", "crash3/l310_midnightrun/l310_midnightrun", 2),
    boss("dingodile", "Dingodile", "crash3/bosses/b302_dingodile/b302_dingodile", 2),
    second(level(n(11), "dinomight", "Dino Might!", "crash3/l311_dinomight/l311_dinomight", 3)),
    colored(level(n(12), "deeptrouble", "Deep Trouble", "crash3/l312_deeptrouble/l312_deeptrouble", 3), GemColor::Red),
    colored(level(n(13), "hightime", "High Time", "crash3/l313_hightime/l313_hightime", 3), GemColor::Purple),
    level(n(14), "roadcrash", "Road Crash", "crash3/l314_roadcrash/l314_roadcrash", 3),
    level(n(15), "doubleheader", "Double Header", "crash3/l315_doubleheader/l315_doubleheader", 3),
    boss("ntropy", "Dr. N. Tropy", "crash3/bosses/b303_ntropy/b303_ntropy", 3),
    second(level(n(16), "sphynxinator", "Sphynxinator", "crash3/l316_sphynxinator/l316_sphynxinator", 4)),
    level(n(17), "byebyeblimps", "Bye Bye Blimps", "crash3/l317_byebyeblimps/l317_byebyeblimps", 4),
    level(n(18), "tellnotales", "Tell No Tales", "crash3/l318_tellnotales/l318_tellnotales", 4),
    second(level(n(19), "futurefrenzy", "Future Frenzy", "crash3/l319_futurefrenzy/l319_futurefrenzy", 4)),
    colored(level(n(20), "tombwader", "Tomb Wader", "crash3/l320_tombwader/l320_tombwader", 4), GemColor::Blue),
    boss("ngin", "N. Gin", "crash3/bosses/b304_ngin/b304_ngin", 4),
    second(level(n(21), "gonetomorrow", "Gone Tomorrow", "crash3/l321_gonetomorrow/l321_gonetomorrow", 5)),
    level(n(22), "orangeasphalt", "Orange Asphalt", "crash3/l322_orangeasphalt/l322_orangeasphalt", 5),
    colored(level(n(23), "flamingpassion", "Flaming Passion", "crash3/l323_flamingpassion/l323_flamingpassion", 5), GemColor::Green),
    level(n(24), "madbombers", "Mad Bombers", "crash3/l324_madbombers/l324_madbombers", 5),
    second(level(n(25), "buglite", "Bug Lite", "crash3/l325_buglite/l325_buglite", 5)),
    boss("neocortex", "Dr. Neo Cortex", "crash3/bosses/b305_neocortex/b305_neocortex", 5),
    level(n(26), "skicrazed", "Ski Crazed", "crash3/l326_skicrazed/l326_skicrazed", 6),
    second(level(n(28), "area51", "Area 51?", "crash3/l328_area51/l328_area51", 6)),
    second(level(n(30), "ringsofpower", "Rings of Power", "crash3/l330_ringsofpower/l330_ringsofpower", 6)),
    secret("hotcoco", "Hot Coco", "crash3/l331_hotcoco/l331_hotcoco"),
    secret("eggipusrex", "Eggipus Rex", "crash3/l332_eggipusrex/l332_eggipusrex"),
    second(secret("futuretense", "Future Tense", "crash3/l333_futuretense/l333_futuretense")),
];

/// A level, boss or secret level: an index into [`LEVELS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LevelId(u8);

const fn str_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// The id of a level key, checked at compile time.
const fn id(key: &str) -> LevelId {
    let mut i = 0;
    while i < LEVELS.len() {
        if str_eq(LEVELS[i].key, key) {
            return LevelId(i as u8);
        }
        i += 1;
    }
    panic!("unknown level key");
}

impl LevelId {
    pub fn new(index: usize) -> Option<LevelId> {
        (index < LEVELS.len()).then_some(LevelId(index as u8))
    }

    pub fn index(self) -> usize {
        usize::from(self.0)
    }

    pub fn level(self) -> &'static Level {
        &LEVELS[self.index()]
    }

    pub fn key(self) -> &'static str {
        self.level().key
    }

    pub fn name(self) -> &'static str {
        self.level().name
    }

    pub fn from_key(key: &str) -> Option<LevelId> {
        LevelId::all().find(|id| id.key() == key)
    }

    /// The level whose session has this zone path.
    pub fn from_zone(zone: &str) -> Option<LevelId> {
        LevelId::all().find(|id| id.level().zone == zone)
    }

    /// Every id, in [`LEVELS`] order.
    pub fn all() -> impl Iterator<Item = LevelId> {
        (0..LEVELS.len() as u8).map(LevelId)
    }

    /// The story order index of the summary (§6): Toad Village = 1 … Dr. Neo
    /// Cortex = 30. `None` for the secret warp room's levels.
    pub fn progression_index(self) -> Option<usize> {
        PROGRESSION.iter().position(|&p| p == self).map(|i| i + 1)
    }
}

pub const BOSSES: [LevelId; 5] = [
    id("tinytiger"),
    id("dingodile"),
    id("ntropy"),
    id("ngin"),
    id("neocortex"),
];

pub const CORTEX: LevelId = id("neocortex");

/// Story order used by the summary's progression indices (§3): levels 1–25 and
/// the bosses, which are the first 30 entries of [`LEVELS`].
pub const PROGRESSION: [LevelId; 30] = {
    let mut ids = [LevelId(0); 30];
    let mut i = 0;
    while i < ids.len() {
        ids[i] = LevelId(i as u8);
        i += 1;
    }
    ids
};

/// One level of the secret warp room. Its levels open with the total relic
/// count, any tier (§7); nothing else is stored for them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecretRoomLevel {
    pub number: u8,
    pub relics: u8,
    pub level: LevelId,
}

pub const SECRET_ROOM: [SecretRoomLevel; 5] = [
    SecretRoomLevel {
        number: 26,
        relics: 5,
        level: id("skicrazed"),
    },
    SecretRoomLevel {
        number: 27,
        relics: 10,
        level: id("hangemhigh"),
    },
    SecretRoomLevel {
        number: 28,
        relics: 15,
        level: id("area51"),
    },
    SecretRoomLevel {
        number: 29,
        relics: 20,
        level: id("futurefrenzy"),
    },
    SecretRoomLevel {
        number: 30,
        relics: 25,
        level: id("ringsofpower"),
    },
];

/// Relic thresholds in centiseconds: a time trial time **under** a threshold
/// earns that relic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelicTimes {
    pub level: LevelId,
    pub platinum: u32,
    pub gold: u32,
    pub sapphire: u32,
}

impl RelicTimes {
    pub fn threshold(&self, relic: Relic) -> u32 {
        match relic {
            Relic::Sapphire => self.sapphire,
            Relic::Gold => self.gold,
            Relic::Platinum => self.platinum,
        }
    }
}

const fn rt(key: &str, platinum: u32, gold: u32, sapphire: u32) -> RelicTimes {
    RelicTimes {
        level: id(key),
        platinum,
        gold,
        sapphire,
    }
}

/// Relic thresholds of the 31 levels with a time trial, in [`LEVELS`] order.
/// The values and the public sources that agree with each one are listed in
/// `docs/FORMAT.md` §9; Toad Village's gold and sapphire were also shown in
/// game. The values that section marks "to be confirmed" are being checked in
/// game.
#[rustfmt::skip]
pub const RELIC_TIMES: [RelicTimes; 31] = [
    rt("toadvillage", 4403, 5750, 6300),
    rt("underpressure", 6350, 7090, 9900),
    rt("orientexpress", 1810, 2780, 4100),
    rt("boneyard", 8100, 10020, 10500),
    rt("makinwaves", 5623, 6323, 7100),
    rt("geewiz", 6590, 8273, 9500),
    rt("hangemhigh", 4380, 5263, 8400),
    rt("hogride", 3806, 4443, 4800),
    rt("tombtime", 5390, 7000, 10200),
    rt("midnightrun", 1820, 3823, 5100),
    rt("dinomight", 6300, 8576, 9400),
    rt("deeptrouble", 6833, 7513, 9700),
    rt("hightime", 5693, 6410, 13200),
    rt("roadcrash", 8310, 8670, 9100),
    rt("doubleheader", 5940, 8113, 8700),
    rt("sphynxinator", 5670, 8263, 10200),
    rt("byebyeblimps", 3150, 3840, 4900),
    rt("tellnotales", 7523, 9563, 13200),
    rt("futurefrenzy", 7963, 9400, 12100),
    rt("tombwader", 8400, 10503, 16400),
    rt("gonetomorrow", 6210, 8560, 12500),
    rt("orangeasphalt", 8380, 9330, 9800),
    rt("flamingpassion", 5940, 7310, 10300),
    rt("madbombers", 5813, 7523, 13800),
    rt("buglite", 7490, 9486, 10900),
    rt("skicrazed", 3630, 5350, 7900),
    rt("area51", 11250, 11780, 12100),
    rt("ringsofpower", 6173, 7146, 9000),
    rt("hotcoco", 1996, 3010, 6000),
    rt("eggipusrex", 4783, 5303, 5800),
    rt("futuretense", 5217, 7543, 12152),
];

/// The relic thresholds of a level; `None` for a boss.
pub fn relic_times(level: LevelId) -> Option<&'static RelicTimes> {
    RELIC_TIMES.iter().find(|t| t.level == level)
}

/// The hub story scenes of one story step: those that play from the previous
/// boss's defeat up to this one's (§3, "Bosses and abilities"). The `start`
/// step belongs to no boss.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CutsceneStep {
    /// `start` or a boss key.
    pub step: &'static str,
    pub scenes: &'static [&'static str],
}

impl CutsceneStep {
    /// The boss of this step; `None` for `start`.
    pub fn boss(&self) -> Option<LevelId> {
        BOSSES.into_iter().find(|b| b.key() == self.step)
    }
}

pub const CUTSCENES: [CutsceneStep; 6] = [
    CutsceneStep {
        step: "start",
        scenes: &[
            "C3_Cortex_Taunt_1",
            "C3_AkuAku_TimeTrialInfo",
            "C3_AkuAku_Warning",
        ],
    },
    CutsceneStep {
        step: "tinytiger",
        scenes: &[
            "C3_Tiny_Intro",
            "C3_Tiny_PreBossFight",
            "C3_Tiny_PostBossFight",
        ],
    },
    CutsceneStep {
        step: "dingodile",
        scenes: &[
            "C3_Hub2_Intro",
            "C3_Dingodile_Intro",
            "C3_Dingodile_PreBossFight",
        ],
    },
    CutsceneStep {
        step: "ntropy",
        scenes: &[
            "C3_NTropy_Hub3_Intro",
            "C3_NTropy_PreFight",
            "C3_NTropy_Taunt",
            "C3_Cortex_Post_NTropy_Fight",
        ],
    },
    CutsceneStep {
        step: "ngin",
        scenes: &["C3_NGin_Intro", "C3_PreNGinFight", "C3_PostNGinFight"],
    },
    CutsceneStep {
        step: "neocortex",
        scenes: &["C3_Cortex_Taunt_2", "C3_PreFinalBattle"],
    },
];

/// Whether `name` is one of the scenes of [`CUTSCENES`].
pub fn is_cutscene(name: &str) -> bool {
    CUTSCENES.iter().any(|s| s.scenes.contains(&name))
}

/// Field indices of a zone session (§4, "Session fields").
pub mod field {
    /// -1 = never entered, 1 = entered.
    pub const STATE: usize = 2;
    /// 1 after the level is first left, then +1 per finish. Not a play count.
    pub const COMPLETIONS: usize = 3;
    /// Time trial leaderboard in centiseconds, best first, 0 = empty slot.
    pub const TIMES: [usize; 3] = [5, 6, 7];
    /// `""` for a player's time, `"---"` for an empty slot.
    pub const INITIALS: [usize; 3] = [8, 9, 10];
    /// 0 none, then [`Relic::tier`](super::Relic::tier).
    pub const RELIC_TIER: usize = 11;
    /// A bool that always mirrors the `CRYSTAL` bit.
    pub const CRYSTAL: usize = 12;
    /// The bits of [`bit`](super::bit).
    pub const COLLECTIBLES: usize = 13;

    /// Our name of each field, as `docs/FORMAT.md` lists them.
    pub const NAMES: [&str; 15] = [
        "unknown_0",
        "unknown_1",
        "state",
        "completions",
        "unknown_4",
        "time_1",
        "time_2",
        "time_3",
        "initials_1",
        "initials_2",
        "initials_3",
        "relic_tier",
        "crystal",
        "collectibles",
        "unknown_14",
    ];
}

/// Bits of the `collectibles` session field (§4, "Collectible bits").
pub mod bit {
    pub const GEM_CLEAR: u32 = 0x0002;
    pub const GEM_BLUE: u32 = 0x0004;
    pub const GEM_GREEN: u32 = 0x0008;
    pub const GEM_PURPLE: u32 = 0x0010;
    pub const GEM_RED: u32 = 0x0020;
    pub const GEM_YELLOW: u32 = 0x0040;
    pub const GEM_SECOND: u32 = 0x0100;
    pub const CRYSTAL: u32 = 0x0200;
    pub const BOSS_DEFEATED: u32 = 0x0800;
    pub const RELIC_PLATINUM: u32 = 0x1000;
    pub const RELIC_SAPPHIRE: u32 = 0x2000;
    pub const RELIC_GOLD: u32 = 0x4000;

    pub const GEM_COLORED: u32 = GEM_BLUE | GEM_GREEN | GEM_PURPLE | GEM_RED | GEM_YELLOW;
    pub const RELICS: u32 = RELIC_SAPPHIRE | RELIC_GOLD | RELIC_PLATINUM;
    /// Every bit the editor knows. The others are never observed and are kept.
    pub const KNOWN: u32 = GEM_CLEAR | GEM_SECOND | CRYSTAL | BOSS_DEFEATED | RELICS | GEM_COLORED;
}

/// Initials of an empty leaderboard slot.
pub const EMPTY_INITIALS: &str = "---";

/// Flag table key of the bonus gem, the 47th gem.
pub const BONUS_GEM_KEY: &str = "LastGemRewarded";

/// Flag table key set when Cortex is beaten again at 108% (secret ending).
pub const SECRET_ENDING_KEY: &str = "OneHundredCortexBeat";

const CUTSCENE_PREFIX: &str = "l300_hub_talkingheadcutscenes.";

/// The flag table key of a hub story scene seen.
pub fn cutscene_key(name: &str) -> String {
    format!("{CUTSCENE_PREFIX}{name} (archetype {CUTSCENE_PREFIX}{name}_entityData)")
}

/// The scene name of a flag table key written by [`cutscene_key`], for any
/// scene name, listed in [`CUTSCENES`] or not.
pub fn cutscene_name(key: &str) -> Option<&str> {
    let name = key
        .strip_prefix(CUTSCENE_PREFIX)?
        .split_once(" (archetype ")?
        .0;
    (cutscene_key(name) == key).then_some(name)
}

/// The flag table key present while a level holds a relic.
pub fn relic_counted_key(level: &Level) -> String {
    format!("{}_RelicCounted", level.name)
}

/// The level of a key written by [`relic_counted_key`].
pub fn relic_counted_level(key: &str) -> Option<LevelId> {
    let name = key.strip_suffix("_RelicCounted")?;
    LevelId::all().find(|l| l.name() == name)
}

/// Maxima of the totals (§6): 25 + 47 + 31 + 5 = 108%.
pub const MAX_CRYSTALS: u32 = 25;
pub const MAX_CLEAR_GEMS: u32 = 31;
pub const MAX_SECOND_GEMS: u32 = 10;
pub const MAX_COLORED_GEMS: u32 = 5;
pub const MAX_BONUS_GEMS: u32 = 1;
pub const MAX_GEMS: u32 = MAX_CLEAR_GEMS + MAX_SECOND_GEMS + MAX_COLORED_GEMS + MAX_BONUS_GEMS;
pub const MAX_RELICS: u32 = 31;
pub const MAX_BOSSES: u32 = 5;
pub const MAX_PERCENT: u32 = MAX_CRYSTALS + MAX_GEMS + MAX_RELICS + MAX_BOSSES;

/// Levels, abilities and relics are written in edit ops as their keys.
macro_rules! serde_as_key {
    ($ty:ty, $what:literal) => {
        impl serde::Serialize for $ty {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(self.key())
            }
        }

        impl<'de> serde::Deserialize<'de> for $ty {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let key = <String as serde::Deserialize>::deserialize(d)?;
                <$ty>::from_key(&key)
                    .ok_or_else(|| serde::de::Error::custom(format!("unknown {} {key:?}", $what)))
            }
        }
    };
}

serde_as_key!(LevelId, "level");
serde_as_key!(Ability, "ability");
serde_as_key!(Relic, "relic");
