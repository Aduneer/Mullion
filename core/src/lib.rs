//! What Mullion's apps share, so each one isn't carrying its own copy: the hand-written Win32
//! bindings, the settings files, shell icons, the Alt+Tab window list and DWM's live thumbnails of
//! those windows, global shortcuts, the power commands, the bits of look every window has, and how
//! the apps and Mullion.exe find and talk to each other.
//! Windows only, as the apps are.
//!
//! The volume mixer (`audio`), what's playing (`media`), HTTPS (`net`) and performance counters
//! (`pdh`) are behind cargo features of the same names, for the apps that use them.

#[cfg(feature = "audio")]
pub mod audio;
pub mod ffi;
pub mod hotkey;
pub mod icons;
pub mod look;
#[cfg(feature = "media")]
pub mod media;
#[cfg(feature = "net")]
pub mod net;
#[cfg(feature = "pdh")]
pub mod pdh;
pub mod power;
pub mod settings;
pub mod suite;
pub mod thumbs;
pub mod wins;
