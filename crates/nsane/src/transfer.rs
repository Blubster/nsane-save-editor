//! Switch ↔ PC conversion.
//!
//! Both platforms hold the same game data, but the container around it
//! differs: type registry, object order and ids, memory pools, the "no
//! reference" value and the padding (`docs/FORMAT.md` §1). Nothing of that
//! is converted. Instead, the game data of the source is copied into a save
//! of the target platform, the *template*, matched by meaning: zone path,
//! variable name, flag key and summary key. Everything else stays the
//! template's own.

use std::collections::BTreeMap;
use std::fmt;

use igb::{FieldValue, IgObject, IgbError, IgbFile, SummaryValue};
use thiserror::Error;

use crate::crash3::Crash3Save;
use crate::{Error, Platform};

/// Whether a converted save was confirmed to load in game, on both
/// platforms, with the same progress. While false, the GUI says the
/// conversion is experimental. Confirmed in round 8 of the live tests
/// (`docs/FORMAT.md` §10).
pub const VERIFIED_IN_GAME: bool = true;

/// The only `save_version` the editor knows.
pub const SAVE_VERSION: i32 = 75;

const SWITCH_NEW_GAME: &[u8] = include_bytes!("../../../saves/initial_game.sav");
const PC_NEW_GAME: &[u8] = include_bytes!("../../../saves/ALCHEMYXSAVE_8.sav");

/// A new-game save of `platform`: the template to use when the user has no
/// save of their own from that platform.
pub fn bundled_template(platform: Platform) -> &'static [u8] {
    match platform {
        Platform::Switch => SWITCH_NEW_GAME,
        Platform::Pc => PC_NEW_GAME,
    }
}

/// [`bundled_template`], parsed.
pub fn bundled_template_save(platform: Platform) -> Result<Crash3Save, Error> {
    Crash3Save::parse(bundled_template(platform))
}

/// Which of the two saves an error is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Source,
    Template,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Role::Source => "source",
            Role::Template => "template",
        })
    }
}

/// Why a save can't be converted. Nothing is ever converted in part.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TransferError {
    #[error("the {role} save has version {}, the editor knows version {SAVE_VERSION}",
        found.map_or_else(|| "none".to_owned(), |v| v.to_string()))]
    Version { role: Role, found: Option<i32> },

    #[error("the template has no zone {0}")]
    MissingZone(String),

    #[error("the template has no game variable {0}")]
    MissingVariable(String),

    /// The two saves store the same thing differently.
    #[error("the saves differ in layout: {0}")]
    Layout(String),

    /// The source holds data the editor can't carry over.
    #[error("the source save holds data the editor can't convert: {0}")]
    Unsupported(String),

    #[error(transparent)]
    Save(#[from] Error),
}

impl From<IgbError> for TransferError {
    fn from(e: IgbError) -> Self {
        TransferError::Save(Error::Igb(e))
    }
}

fn layout(what: impl Into<String>) -> TransferError {
    TransferError::Layout(what.into())
}

fn version(save: &Crash3Save) -> Option<i32> {
    match save.igb().summary().get("save_version") {
        Some(SummaryValue::Int(v)) => Some(*v),
        _ => None,
    }
}

/// The only object of `class` in `file`.
fn singleton<'a>(
    file: &'a IgbFile,
    class: &'a str,
    role: Role,
) -> Result<&'a IgObject, TransferError> {
    let mut objects = file.objects_of(class);
    match (objects.next(), objects.next()) {
        (Some(object), None) => Ok(object),
        _ => Err(layout(format!("the {role} save has no single {class}"))),
    }
}

/// Fields `fields` of `source` and `template` have the same types.
fn same_types(
    source: &IgObject,
    template: &IgObject,
    fields: impl IntoIterator<Item = usize>,
    what: &str,
) -> Result<(), TransferError> {
    for i in fields {
        let (s, t) = (source.field(i), template.field(i));
        if s.map(FieldValue::type_name) != t.map(FieldValue::type_name) || s.is_none() {
            return Err(layout(format!("field {i} of {what}")));
        }
    }
    Ok(())
}

/// The source's value of a field, in terms of the output file.
///
/// A bool takes the source's value byte and keeps the template's upper
/// bytes, which are arbitrary data that differs between platforms (§2). A
/// string is carried by its text. Ids are not portable, so a reference is
/// refused; the game data copied holds none.
fn carry(
    value: FieldValue,
    template: FieldValue,
    source: &IgbFile,
    out: &mut IgbFile,
    what: &str,
) -> Result<FieldValue, TransferError> {
    Ok(match (value, template) {
        (FieldValue::Bool { value, .. }, FieldValue::Bool { padding, .. }) => {
            FieldValue::Bool { value, padding }
        }
        (FieldValue::Str(0), FieldValue::Str(_)) => FieldValue::Str(0),
        (FieldValue::Str(i), FieldValue::Str(_)) => {
            let text = source
                .string(i)
                .ok_or_else(|| TransferError::Unsupported(format!("{what} holds no string")))?;
            FieldValue::Str(out.string_index(text)?)
        }
        (
            FieldValue::ObjRef(_) | FieldValue::MemRef(_) | FieldValue::Vector(..),
            FieldValue::ObjRef(_) | FieldValue::MemRef(_) | FieldValue::Vector(..),
        ) => {
            return Err(TransferError::Unsupported(format!("{what} is a reference")));
        }
        (value, _) => value,
    })
}

/// Copies fields `fields` of source object `from` into object `to` of `out`.
fn copy_fields(
    source: &IgbFile,
    from: &IgObject,
    out: &mut IgbFile,
    to: u32,
    fields: impl IntoIterator<Item = usize>,
    what: &str,
) -> Result<(), TransferError> {
    for i in fields {
        let missing = || layout(format!("field {i} of {what}"));
        let value = *from.field(i).ok_or_else(missing)?;
        let template = *out
            .object(to)
            .and_then(|o| o.field(i))
            .ok_or_else(missing)?;
        let value = carry(
            value,
            template,
            source,
            out,
            &format!("field {i} of {what}"),
        )?;
        *out.object_mut(to)
            .and_then(|o| o.field_mut(i))
            .ok_or_else(missing)? = value;
    }
    Ok(())
}

/// A list object (count, capacity, memory) that holds nothing.
fn empty_list(list: &IgObject) -> bool {
    list.field(0) == Some(&FieldValue::Int(0))
}

/// The singletons copied field by field: `(class, fields)`.
const SINGLETONS: [(&str, &[usize]); 2] = [
    ("CGameSaveData", &[0, 1, 2, 3, 4]),
    // Field 0 is an empty vector holding the platform's "no reference"
    // value; it is checked to be empty, not copied.
    ("CDynamicDifficultySaveData", &[1, 2]),
];

/// Lists that are empty in every known save. Their element types aren't
/// known, so a source that fills one is refused rather than dropped.
const EMPTY_LISTS: [&str; 2] = [
    "CQuestTrackSaveDataList",
    "CGameVariableValueSavedStringList",
];

/// Checks everything [`transfer`] relies on, before anything is copied.
fn validate(source: &Crash3Save, template: &Crash3Save) -> Result<(), TransferError> {
    for (role, save) in [(Role::Source, source), (Role::Template, template)] {
        let found = version(save);
        if found != Some(SAVE_VERSION) {
            return Err(TransferError::Version { role, found });
        }
    }

    let template_zones: BTreeMap<&str, &IgObject> = template.zones().collect();
    for (path, session) in source.zones() {
        let target = template_zones
            .get(path)
            .ok_or_else(|| TransferError::MissingZone(path.to_owned()))?;
        if session.fields.len() != target.fields.len() {
            return Err(layout(format!("session of {path}")));
        }
        same_types(session, target, 0..session.fields.len(), path)?;
    }

    let template_vars: BTreeMap<&str, &IgObject> = template.variable_objects().collect();
    for (name, var) in source.variable_objects() {
        let target = template_vars
            .get(name)
            .ok_or_else(|| TransferError::MissingVariable(name.to_owned()))?;
        if var.class != target.class {
            return Err(layout(format!(
                "variable {name} is a {} in the source and a {} in the template",
                var.class, target.class
            )));
        }
        same_types(var, target, [0], name)?;
    }

    for (class, fields) in SINGLETONS {
        let s = singleton(source.igb(), class, Role::Source)?;
        let t = singleton(template.igb(), class, Role::Template)?;
        same_types(s, t, fields.iter().copied(), class)?;
    }
    let difficulty = singleton(source.igb(), "CDynamicDifficultySaveData", Role::Source)?;
    match difficulty.field(0) {
        Some(&FieldValue::Vector(reference, 0)) if reference >= 0x8000_0000 => {}
        _ => {
            return Err(TransferError::Unsupported(
                "a non-empty CDynamicDifficultySaveData vector".to_owned(),
            ));
        }
    }
    for class in EMPTY_LISTS {
        singleton(template.igb(), class, Role::Template)?;
        if !empty_list(singleton(source.igb(), class, Role::Source)?) {
            return Err(TransferError::Unsupported(format!("a non-empty {class}")));
        }
    }

    if source.int_table_id().is_some() != template.int_table_id().is_some() {
        return Err(layout("only one of the saves has a checkpoint table"));
    }
    Ok(())
}

/// Rebuilds hash table `to` of `out` with the items of table `from` of
/// `source`, at its capacity and in its slot order, so the layout comes out
/// as close to the source's as the game's hashing allows. The table
/// object's bool takes the source's value byte.
fn copy_table(
    source: &IgbFile,
    from: u32,
    out: &mut IgbFile,
    to: u32,
    what: &str,
) -> Result<(), TransferError> {
    let table = source.hash_table(from)?;
    let items = table.items()?;
    out.hash_table_mut(to)?.rebuild(table.capacity(), &items)?;
    let object = source.object(from).ok_or_else(|| layout(what))?;
    copy_fields(source, object, out, to, [3], what)
}

/// The game data of `source` in the container of `template`.
///
/// The template is usually a save of the other platform: the user's own
/// when they have one, since it matches their game version, or a
/// [`bundled_template`]. A template of the same platform is allowed; the
/// result then equals the source in game terms.
///
/// The summary is copied as is; writing the result recomputes it, as for
/// any save.
pub fn transfer(source: &Crash3Save, template: &Crash3Save) -> Result<Crash3Save, TransferError> {
    validate(source, template)?;
    let src = source.igb();
    let mut out = template.igb().clone();

    // The summary has its own key and value blobs, outside the string table.
    *out.summary_mut() = src.summary().clone();

    let template_zones: BTreeMap<&str, u32> =
        template.zones().map(|(path, s)| (path, s.id)).collect();
    for (path, session) in source.zones() {
        let to = *template_zones
            .get(path)
            .ok_or_else(|| TransferError::MissingZone(path.to_owned()))?;
        copy_fields(src, session, &mut out, to, 0..session.fields.len(), path)?;
    }

    let template_vars: BTreeMap<&str, u32> = template
        .variable_objects()
        .map(|(name, v)| (name, v.id))
        .collect();
    for (name, var) in source.variable_objects() {
        let to = *template_vars
            .get(name)
            .ok_or_else(|| TransferError::MissingVariable(name.to_owned()))?;
        copy_fields(src, var, &mut out, to, [0], name)?;
    }

    for (class, fields) in SINGLETONS {
        let from = singleton(src, class, Role::Source)?;
        let to = singleton(template.igb(), class, Role::Template)?.id;
        copy_fields(src, from, &mut out, to, fields.iter().copied(), class)?;
    }

    copy_table(
        src,
        source.flag_table_id(),
        &mut out,
        template.flag_table_id(),
        "flag table",
    )?;
    if let (Some(from), Some(to)) = (source.int_table_id(), template.int_table_id()) {
        copy_table(src, from, &mut out, to, "checkpoint table")?;
    }

    Ok(Crash3Save::from_igb(out)?)
}
