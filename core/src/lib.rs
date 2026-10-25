//! What Mullion's apps share, so each one isn't carrying its own copy: the hand-written Win32
//! bindings, the settings files, shell icons, the Alt+Tab window list, the power commands, the
//! bits of look every window has, and how the apps and Mullion.exe find and talk to each other.
//! Windows only, as the apps are.
//!
//! The volume mixer (`audio`), what's playing (`media`) and performance counters (`pdh`) are
//! behind cargo features of the same names, for the apps that show them.

#[cfg(feature = "audio")]
pub mod audio;
pub mod ffi;
pub mod icons;
pub mod look;
#[cfg(feature = "media")]
pub mod media;
#[cfg(feature = "pdh")]
pub mod pdh;
pub mod power;
pub mod settings;
pub mod suite;
pub mod wins;
