//! The accelerator text of a GNOME custom shortcut, for example
//! `<Super>space`. The rules are the rules of the parser of GNOME Shell 46
//! (Mutter, `meta-accel-parse.c`), which reads the `binding` key. See
//! `docs/gnome-desktop.md`.

use std::ffi::{c_char, c_int, c_void, CString};
use std::fmt;
use std::sync::OnceLock;

/// Why an accelerator text is not correct. The text is for the owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceleratorError {
    /// `<...>` with a name that is not a modifier, for example `<Win>`.
    UnknownModifier(String),
    /// A `<` with no `>` after it.
    UnclosedModifier,
    /// Only modifiers, for example `<Super>`.
    MissingKey,
    /// The key name is not a key, for example `spcae`.
    UnknownKey(String),
}

impl fmt::Display for AcceleratorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownModifier(name) => write!(
                f,
                "<{name}> is not a modifier. Use <Shift>, <Control>, <Alt>, <Super>, \
                 <Meta>, <Hyper> or <Mod1> to <Mod5>."
            ),
            Self::UnclosedModifier => f.write_str("A modifier has no closing \">\"."),
            Self::MissingKey => f.write_str(
                "The text has no key after the modifiers, for example \"space\" in \
                 \"<Super>space\".",
            ),
            Self::UnknownKey(name) => write!(
                f,
                "\"{name}\" is not a key name. Use a key name such as \"space\", \"a\" or \"F1\"."
            ),
        }
    }
}

impl std::error::Error for AcceleratorError {}

/// Checks an accelerator text before the program writes it to the custom
/// shortcut. An empty text and `disabled` mean "no key" and are correct.
pub fn validate_accelerator(text: &str) -> Result<(), AcceleratorError> {
    parse(text).map(drop)
}

/// True when the two texts give the same key to GNOME Shell. Examples:
/// `<Super>space`, `<super>Space` and `<Mod4>space`; `<Shift><Control>x` and
/// `<Control><Shift>x`. `<Mod4>` is `<Super>`, because the keyboard layouts
/// of GNOME give the Super key the modifier Mod4. A text that is not correct
/// is the same only as an equal text.
pub(crate) fn same_key(a: &str, b: &str) -> bool {
    match (parse(a), parse(b)) {
        (Ok(Some(a)), Ok(Some(b))) => a == b,
        (Ok(_), Ok(_)) => false,
        _ => a == b,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Accelerator {
    modifiers: u16,
    key: Key,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Key {
    Keysym(u32),
    Keycode(u32),
    AboveTab,
    /// The key name in small letters, when the key names cannot be read
    /// because the library libxkbcommon is not available.
    Name(String),
}

const SHIFT: u16 = 1;
const CONTROL: u16 = 1 << 1;
const MOD1: u16 = 1 << 2;
const MOD2: u16 = 1 << 3;
const MOD3: u16 = 1 << 4;
const MOD5: u16 = 1 << 6;
const META: u16 = 1 << 7;
const SUPER: u16 = 1 << 8;
const HYPER: u16 = 1 << 9;

/// `None`: no key.
fn parse(text: &str) -> Result<Option<Accelerator>, AcceleratorError> {
    if text.is_empty() || text == "disabled" {
        return Ok(None);
    }
    let mut modifiers = 0;
    let mut rest = text;
    while let Some(after) = rest.strip_prefix('<') {
        let (name, tail) = after
            .split_once('>')
            .ok_or(AcceleratorError::UnclosedModifier)?;
        modifiers |=
            modifier(name).ok_or_else(|| AcceleratorError::UnknownModifier(name.to_owned()))?;
        rest = tail;
    }
    if rest.is_empty() {
        return Err(AcceleratorError::MissingKey);
    }
    Ok(Some(Accelerator {
        modifiers,
        key: key(rest)?,
    }))
}

fn modifier(name: &str) -> Option<u16> {
    let bit = match name.to_ascii_lowercase().as_str() {
        "shift" | "shft" => SHIFT,
        "control" | "primary" | "ctrl" | "ctl" => CONTROL,
        "alt" | "mod1" => MOD1,
        "mod2" => MOD2,
        "mod3" => MOD3,
        "mod4" | "super" => SUPER,
        "mod5" => MOD5,
        "meta" => META,
        "hyper" => HYPER,
        _ => return None,
    };
    Some(bit)
}

fn key(name: &str) -> Result<Key, AcceleratorError> {
    if let Some(code) = keycode(name) {
        return Ok(Key::Keycode(code));
    }
    if name == "Above_Tab" {
        return Ok(Key::AboveTab);
    }
    let Some(from_name) = keysym_from_name() else {
        return Ok(Key::Name(name.to_lowercase()));
    };
    [name.to_owned(), format!("XF86{name}")]
        .iter()
        .find_map(|candidate| keysym(from_name, candidate))
        .map(Key::Keysym)
        .ok_or_else(|| AcceleratorError::UnknownKey(name.to_owned()))
}

/// `0x` and two or more hexadecimal digits, as in Mutter.
fn keycode(name: &str) -> Option<u32> {
    let digits = name.strip_prefix("0x")?;
    let end = digits
        .find(|c: char| !c.is_ascii_hexdigit())
        .unwrap_or(digits.len());
    if end < 2 {
        return None;
    }
    u32::from_str_radix(&digits[..end], 16).ok()
}

/// `xkb_keysym_from_name` of libxkbcommon.
type KeysymFromName = unsafe extern "C" fn(*const c_char, c_int) -> u32;

const XKB_KEYSYM_CASE_INSENSITIVE: c_int = 1;
const XKB_KEY_NO_SYMBOL: u32 = 0;

fn keysym(from_name: KeysymFromName, name: &str) -> Option<u32> {
    let name = CString::new(name).ok()?;
    // SAFETY: `name` is a C string that lives during the call, and the
    // function does not keep the pointer.
    let keysym = unsafe { from_name(name.as_ptr(), XKB_KEYSYM_CASE_INSENSITIVE) };
    (keysym != XKB_KEY_NO_SYMBOL).then_some(keysym)
}

/// Mutter reads the key name with `xkb_keysym_from_name`, so this module
/// calls the same function. The library is loaded at run time, because the
/// build has no libxkbcommon development files, and GNOME Shell always has
/// the library.
fn keysym_from_name() -> Option<KeysymFromName> {
    static FUNCTION: OnceLock<Option<KeysymFromName>> = OnceLock::new();
    *FUNCTION.get_or_init(|| {
        // SAFETY: the two names are C strings. The library is never
        // unloaded, so the function pointer stays valid. The symbol has the
        // type `KeysymFromName` in `xkbcommon.h`.
        unsafe {
            let library = libc::dlopen(
                c"libxkbcommon.so.0".as_ptr(),
                libc::RTLD_NOW | libc::RTLD_LOCAL,
            );
            if library.is_null() {
                return None;
            }
            let symbol = libc::dlsym(library, c"xkb_keysym_from_name".as_ptr());
            (!symbol.is_null()).then(|| std::mem::transmute::<*mut c_void, KeysymFromName>(symbol))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_key_of_the_owner_and_other_usual_keys() {
        for text in [
            "<Super>space",
            "<Control><Super>plus",
            "<Alt>F2",
            "<Primary><Shift>a",
            "Print",
            "<Super>Page_Up",
            "<Super>odiaeresis",
        ] {
            assert_eq!(validate_accelerator(text), Ok(()), "{text}");
        }
    }

    #[test]
    fn empty_text_and_disabled_mean_no_key() {
        assert_eq!(parse(""), Ok(None));
        assert_eq!(parse("disabled"), Ok(None));
    }

    #[test]
    fn modifier_and_key_names_ignore_letter_case() {
        assert!(same_key("<Super>space", "<super>Space"));
        assert!(same_key("<SHIFT>a", "<Shift>A"));
    }

    #[test]
    fn the_order_of_the_modifiers_has_no_effect() {
        assert!(same_key("<Control><Super>plus", "<Super><Control>plus"));
    }

    #[test]
    fn the_other_names_of_control_and_shift_are_the_same_modifier() {
        for control in ["<Primary>", "<Ctrl>", "<Ctl>"] {
            assert!(same_key(&format!("{control}x"), "<Control>x"), "{control}");
        }
        assert!(same_key("<Shft>x", "<Shift>x"));
    }

    #[test]
    fn two_names_of_one_key_are_the_same_key() {
        assert!(same_key("<Super>Prior", "<Super>Page_Up"));
    }

    #[test]
    fn a_media_key_name_can_be_written_without_its_xf86_prefix() {
        assert!(same_key("AudioPlay", "XF86AudioPlay"));
    }

    #[test]
    fn a_different_key_or_modifier_is_not_the_same_key() {
        assert!(!same_key("<Super>space", "<Super>a"));
        assert!(!same_key("<Super>space", "<Control>space"));
        assert!(!same_key("<Super>space", "space"));
        assert!(!same_key("<Super>space", ""));
        assert!(!same_key("", ""));
    }

    #[test]
    fn super_and_mod4_are_the_same_modifier() {
        assert!(same_key("<Super>space", "<Mod4>space"));
    }

    #[test]
    fn a_text_that_is_not_correct_is_the_same_only_as_an_equal_text() {
        assert!(same_key("<Win>space", "<Win>space"));
        assert!(!same_key("<Win>space", "<Super>space"));
    }

    #[test]
    fn reads_a_key_code_and_above_tab() {
        assert_eq!(
            parse("<Super>0x41").unwrap().unwrap().key,
            Key::Keycode(0x41)
        );
        assert_eq!(parse("<Alt>Above_Tab").unwrap().unwrap().key, Key::AboveTab);
    }

    #[test]
    fn rejects_an_unknown_modifier() {
        assert_eq!(
            validate_accelerator("<Win>space"),
            Err(AcceleratorError::UnknownModifier("Win".into()))
        );
    }

    #[test]
    fn rejects_a_modifier_with_no_closing_bracket() {
        assert_eq!(
            validate_accelerator("<Super"),
            Err(AcceleratorError::UnclosedModifier)
        );
    }

    #[test]
    fn rejects_modifiers_with_no_key() {
        assert_eq!(
            validate_accelerator("<Super>"),
            Err(AcceleratorError::MissingKey)
        );
    }

    #[test]
    fn rejects_a_key_name_with_a_typing_error_or_a_space() {
        assert_eq!(
            validate_accelerator("<Super>spcae"),
            Err(AcceleratorError::UnknownKey("spcae".into()))
        );
        assert_eq!(
            validate_accelerator("<Super> space"),
            Err(AcceleratorError::UnknownKey(" space".into()))
        );
    }

    #[test]
    fn the_error_text_tells_the_owner_what_to_write() {
        let text = AcceleratorError::UnknownModifier("Win".into()).to_string();

        assert!(
            text.starts_with("<Win> is not a modifier. Use <Shift>"),
            "{text}"
        );
        assert!(AcceleratorError::MissingKey
            .to_string()
            .contains("<Super>space"));
        assert!(AcceleratorError::UnclosedModifier.to_string().contains('>'));
        assert!(AcceleratorError::UnknownKey("x y".into())
            .to_string()
            .starts_with("\"x y\" is not a key name"));
    }
}
