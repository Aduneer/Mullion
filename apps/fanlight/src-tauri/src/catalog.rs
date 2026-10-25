//! The fixed results: Windows' Settings pages, and commands (lock, sleep, restart, take a
//! screenshot, Fanlight's own). Each is found by its name or by the other words it goes by.

/// A Settings page: its name, other words for it, and its `ms-settings:` page.
pub struct Page {
    pub name: &'static str,
    pub words: &'static str,
    pub uri: &'static str,
}

const fn page(name: &'static str, words: &'static str, uri: &'static str) -> Page {
    Page { name, words, uri }
}

pub const PAGES: &[Page] = &[
    page("Display", "screen resolution scale monitor brightness hdr", "ms-settings:display"),
    page("Night light", "blue light warm evening", "ms-settings:nightlight"),
    page("Sound", "audio volume speakers output input microphone", "ms-settings:sound"),
    page("Volume mixer", "app volume audio", "ms-settings:apps-volume"),
    page("Notifications", "alerts toasts do not disturb", "ms-settings:notifications"),
    page("Focus", "do not disturb concentrate", "ms-settings:quiethours"),
    page("Power & battery", "sleep energy saver battery power mode", "ms-settings:powersleep"),
    page("Storage", "disk space cleanup drives storage sense", "ms-settings:storagesense"),
    page("Multitasking", "snap windows alt tab virtual desktops", "ms-settings:multitasking"),
    page("Clipboard", "copy paste history", "ms-settings:clipboard"),
    page("Remote Desktop", "rdp remote", "ms-settings:remotedesktop"),
    page("About", "pc name specs system info version windows edition rename", "ms-settings:about"),
    page("Bluetooth & devices", "bluetooth pair headphones devices", "ms-settings:bluetooth"),
    page("Printers & scanners", "printer print scanner", "ms-settings:printers"),
    page("Mouse", "pointer cursor scroll", "ms-settings:mousetouchpad"),
    page("Touchpad", "trackpad gestures", "ms-settings:devices-touchpad"),
    page("Typing", "keyboard autocorrect spelling", "ms-settings:typing"),
    page("Camera", "webcam", "ms-settings:camera"),
    page("Wi-Fi", "wifi wireless network internet", "ms-settings:network-wifi"),
    page("Network & internet", "ethernet network internet connection", "ms-settings:network"),
    page("VPN", "vpn virtual private network", "ms-settings:network-vpn"),
    page("Mobile hotspot", "hotspot tethering share internet", "ms-settings:network-mobilehotspot"),
    page("Proxy", "proxy server", "ms-settings:network-proxy"),
    page("Airplane mode", "flight mode", "ms-settings:network-airplanemode"),
    page("Personalization", "personalise customize customise look", "ms-settings:personalization"),
    page("Background", "wallpaper desktop picture", "ms-settings:personalization-background"),
    page("Colors", "colours accent dark mode light mode theme transparency", "ms-settings:colors"),
    page("Themes", "theme cursor icons", "ms-settings:themes"),
    page("Lock screen", "lockscreen", "ms-settings:lockscreen"),
    page("Start", "start menu", "ms-settings:personalization-start"),
    page("Taskbar", "task bar tray", "ms-settings:taskbar"),
    page("Fonts", "typeface", "ms-settings:fonts"),
    page("Installed apps", "uninstall programs apps features remove", "ms-settings:appsfeatures"),
    page("Default apps", "default browser open with file associations", "ms-settings:defaultapps"),
    page("Startup apps", "startup autostart sign in launch", "ms-settings:startupapps"),
    page("Optional features", "features add", "ms-settings:optionalfeatures"),
    page("Your info", "account profile picture", "ms-settings:yourinfo"),
    page("Sign-in options", "password pin windows hello fingerprint face", "ms-settings:signinoptions"),
    page("Email & accounts", "accounts email microsoft", "ms-settings:emailandaccounts"),
    page("Family & other users", "users family add user", "ms-settings:otherusers"),
    page("Date & time", "clock time zone timezone", "ms-settings:dateandtime"),
    page("Language & region", "language region locale format keyboard layout", "ms-settings:regionlanguage"),
    page("Speech", "voice", "ms-settings:speech"),
    page("Gaming", "game mode xbox game bar", "ms-settings:gaming-gamemode"),
    page("Accessibility", "ease of access a11y", "ms-settings:easeofaccess"),
    page("Text size", "font size bigger", "ms-settings:easeofaccess-display"),
    page("Magnifier", "zoom", "ms-settings:easeofaccess-magnifier"),
    page("Narrator", "screen reader", "ms-settings:easeofaccess-narrator"),
    page("Privacy & security", "privacy permissions", "ms-settings:privacy"),
    page("Location", "gps privacy", "ms-settings:privacy-location"),
    page("Microphone privacy", "mic permissions", "ms-settings:privacy-microphone"),
    page("Windows Security", "antivirus defender firewall virus", "ms-settings:windowsdefender"),
    page("Windows Update", "update updates patch upgrade", "ms-settings:windowsupdate"),
    page("Recovery", "reset this pc restore", "ms-settings:recovery"),
    page("Activation", "product key license", "ms-settings:activation"),
    page("Troubleshoot", "troubleshooter fix problems", "ms-settings:troubleshoot"),
    page("For developers", "developer mode sudo", "ms-settings:developers"),
    page("Nearby sharing", "share", "ms-settings:crossdevice"),
    page("Settings", "control panel preferences", "ms-settings:"),
];

/// A command: what it does, and other words for it.
pub struct Command {
    pub id: &'static str,
    pub name: &'static str,
    pub words: &'static str,
    pub detail: &'static str,
    /// Asked about first, with a second Enter: it can't be undone.
    pub confirm: bool,
}

const fn command(id: &'static str, name: &'static str, words: &'static str, detail: &'static str, confirm: bool) -> Command {
    Command { id, name, words, detail, confirm }
}

pub const COMMANDS: &[Command] = &[
    command("lock", "Lock", "lock screen lock pc away", "Lock the PC", false),
    command("sleep", "Sleep", "suspend standby", "Put the PC to sleep", false),
    command("signout", "Sign out", "log off logout log out sign off", "Sign out of Windows", true),
    command("restart", "Restart", "reboot restart pc", "Restart the PC", true),
    command("shutdown", "Shut down", "shutdown power off turn off", "Shut the PC down", true),
    command("desktop", "Show desktop", "minimize all hide windows", "As Win+D does", false),
    command("screenshot", "Take a screenshot", "snip snipping tool screen capture print screen", "Snip part of the screen, as Win+Shift+S does", false),
    command("emoji", "Emoji", "emoji smiley gif symbols kaomoji", "Windows' emoji panel, for the app you were in", false),
    command("clipboard", "Clipboard history", "clipboard copied paste history cb", "What you've copied lately", false),
    command("clear-clipboard", "Clear clipboard history", "clipboard forget erase", "Forget everything Fanlight kept from the clipboard", true),
    command("settings", "Fanlight Settings", "fanlight preferences options config setup", "Shortcut, look, what to search", false),
    command("reindex", "Update file index", "fanlight files index rescan refresh", "Look through your folders again", false),
    command("quit", "Quit Fanlight", "fanlight exit close", "Until you start it again", false),
];

pub fn command_by_id(id: &str) -> Option<&'static Command> {
    COMMANDS.iter().find(|c| c.id == id)
}

pub fn page_by_uri(uri: &str) -> Option<&'static Page> {
    PAGES.iter().find(|p| p.uri == uri)
}

/// The best score of `name` and each of `words` (a little less for those), for the matcher.
pub fn score(q: &crate::matcher::Query, name: &str, words: &str) -> Option<i32> {
    let by_name = q.score(name);
    let by_word = words.split(' ').filter_map(|w| q.score(w)).max().map(|s| s - 120);
    // All of the query's words somewhere in the name and the other words together; each word as
    // a word or inside one, not just its letters in order across them all ("code" isn't in
    // "Clipboard history" for being in "clipboard copied").
    let together = q.score(&format!("{name} {words}")).filter(|s| *s >= 400).map(|s| s - 200);
    [by_name, by_word, together].into_iter().flatten().max()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matcher::Query;

    #[test]
    fn found_by_other_words() {
        let q = Query::new("wallpaper");
        assert!(score(&q, "Background", PAGES.iter().find(|p| p.name == "Background").unwrap().words).is_some());
        let q = Query::new("reboot");
        let restart = command_by_id("restart").unwrap();
        assert!(score(&q, restart.name, restart.words).is_some() && restart.confirm);
        let q = Query::new("dark mode");
        let colors = PAGES.iter().find(|p| p.name == "Colors").unwrap();
        assert!(score(&q, colors.name, colors.words).is_some());
        // Not by letters scattered across the other words.
        let q = Query::new("code");
        let clipboard = command_by_id("clipboard").unwrap();
        assert!(score(&q, clipboard.name, clipboard.words).is_none_or(|s| s < 0));
    }

    #[test]
    fn pages_are_settings_uris() {
        for p in PAGES {
            assert!(p.uri.starts_with("ms-settings:") && p.uri.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == ':'), "{}", p.uri);
        }
        let mut ids: Vec<_> = COMMANDS.iter().map(|c| c.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), COMMANDS.len());
    }
}
