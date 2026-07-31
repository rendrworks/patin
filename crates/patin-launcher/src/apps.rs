use std::{
    collections::HashSet,
    env,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
};

use freedesktop_desktop_entry::{
    DesktopEntry, Iter, current_desktop, default_paths, get_languages_from_env,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppIcon {
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Application {
    pub name: String,
    pub icon: Option<AppIcon>,
    command: Vec<String>,
    working_directory: Option<PathBuf>,
}

impl Application {
    fn from_entry(
        entry: DesktopEntry,
        locales: &[String],
        desktops: &[String],
        icon_theme: Option<&str>,
    ) -> Option<Self> {
        if entry.type_() != Some("Application")
            || entry.hidden()
            || entry.no_display()
            || !shown_on_desktop(&entry, desktops)
        {
            return None;
        }
        if let Some(executable) = entry.try_exec()
            && !executable_exists(executable)
        {
            return None;
        }
        let name = entry.name(locales)?.trim().to_owned();
        let command = entry.parse_exec().ok()?;
        if name.is_empty() || command.is_empty() {
            return None;
        }
        let icon_name = entry.icon();
        let icon = icon_name.and_then(|name| load_icon(name, icon_theme));
        if env::var_os("PATIN_TRACE").is_some()
            && let Some(icon_name) = icon_name
            && icon.is_none()
        {
            eprintln!("patin-launcher: no usable icon for {name} ({icon_name})");
        }
        Some(Self {
            name,
            icon,
            command,
            working_directory: entry.path().map(PathBuf::from),
        })
    }

    pub fn launch(&self) -> Result<(), String> {
        let (program, arguments) = self
            .command
            .split_first()
            .ok_or_else(|| "application has no command".to_owned())?;
        let mut command = Command::new(program);
        command
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(directory) = &self.working_directory {
            command.current_dir(directory);
        }
        command
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("Could not open {}: {error}", self.name))
    }

    #[cfg(test)]
    pub fn fixture(name: &str) -> Self {
        Self {
            name: name.into(),
            icon: None,
            command: vec!["false".into()],
            working_directory: None,
        }
    }
}

fn load_icon(name: &str, icon_theme: Option<&str>) -> Option<AppIcon> {
    let supplied = Path::new(name);
    let path = if supplied.is_absolute() {
        Some(supplied.to_owned())
    } else {
        let lookup_name = icon_lookup_name(name);
        let lookup = freedesktop_icons::lookup(lookup_name)
            .with_size(32)
            .with_cache();
        let resolved = match icon_theme {
            Some(theme) => lookup.with_theme(theme).find(),
            None => lookup.find(),
        };
        resolved.or_else(|| greedy_theme_fallback(lookup_name, icon_theme))
    };
    let Some(path) = path else {
        if env::var_os("PATIN_TRACE").is_some() {
            eprintln!("patin-launcher: theme resolver found no path for {name}");
        }
        return None;
    };
    let icon = match path.extension().and_then(|extension| extension.to_str()) {
        Some("png") => decode_png(&path),
        Some("svg") => decode_svg(&path),
        _ => None,
    };
    if icon.is_none() && env::var_os("PATIN_TRACE").is_some() {
        eprintln!("patin-launcher: could not decode icon {}", path.display());
    }
    icon
}

fn icon_lookup_name(name: &str) -> &str {
    let supplied = Path::new(name);
    match supplied
        .extension()
        .and_then(|extension| extension.to_str())
    {
        Some("png" | "svg") => supplied
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or(name),
        _ => name,
    }
}

fn greedy_theme_fallback(name: &str, icon_theme: Option<&str>) -> Option<PathBuf> {
    let mut data_roots = Vec::new();
    if let Some(data_home) = env::var_os("XDG_DATA_HOME") {
        data_roots.push(PathBuf::from(data_home));
    } else if let Some(home) = env::var_os("HOME") {
        data_roots.push(PathBuf::from(home).join(".local/share"));
    }
    data_roots.extend(
        env::var_os("XDG_DATA_DIRS")
            .map(|value| env::split_paths(&value).collect())
            .unwrap_or_else(|| {
                vec![
                    PathBuf::from("/usr/local/share"),
                    PathBuf::from("/usr/share"),
                ]
            }),
    );
    for root in [
        PathBuf::from("/usr/local/share"),
        PathBuf::from("/usr/share"),
        PathBuf::from("/var/lib/flatpak/exports/share"),
    ] {
        if !data_roots.contains(&root) {
            data_roots.push(root);
        }
    }

    let mut themes = icon_theme.into_iter().collect::<Vec<_>>();
    if !themes.contains(&"hicolor") {
        themes.push("hicolor");
    }
    let filenames = [
        format!("{name}.png"),
        format!("{name}.svg"),
        format!("{name}-symbolic.svg"),
    ];
    for filename in &filenames {
        for root in &data_roots {
            for theme in &themes {
                let theme_root = root.join("icons").join(theme);
                for directory in ["scalable/apps", "symbolic/apps"] {
                    let direct = theme_root.join(directory).join(filename);
                    if direct.is_file() {
                        return Some(direct);
                    }
                }
                if let Some(path) = find_file(&theme_root, filename, 0) {
                    return Some(path);
                }
            }
            let pixmap = root.join("pixmaps").join(filename);
            if pixmap.is_file() {
                return Some(pixmap);
            }
        }
    }
    None
}

fn find_file(directory: &Path, filename: &str, depth: u8) -> Option<PathBuf> {
    if depth > 4 {
        return None;
    }
    for entry in directory.read_dir().ok()?.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_file() && entry.file_name() == filename {
            return Some(path);
        }
        if path.is_dir()
            && let Some(found) = find_file(&path, filename, depth + 1)
        {
            return Some(found);
        }
    }
    None
}

fn decode_svg(path: &Path) -> Option<AppIcon> {
    const RASTER_SIZE: u32 = 64;
    let data = std::fs::read(path).ok()?;
    let tree = resvg::usvg::Tree::from_data(&data, &resvg::usvg::Options::default()).ok()?;
    let size = tree.size();
    let scale = RASTER_SIZE as f32 / size.width().max(size.height());
    let x = (RASTER_SIZE as f32 - size.width() * scale) / 2.0;
    let y = (RASTER_SIZE as f32 - size.height() * scale) / 2.0;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(RASTER_SIZE, RASTER_SIZE)?;
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale).post_translate(x, y);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    Some(AppIcon {
        width: RASTER_SIZE,
        height: RASTER_SIZE,
        rgba: pixmap.data().to_vec().into(),
    })
}

fn decode_png(path: &Path) -> Option<AppIcon> {
    let image = image::ImageReader::open(path)
        .ok()?
        .decode()
        .ok()?
        .into_rgba8();
    let (width, height) = image.dimensions();
    let mut rgba = image.into_raw();
    for pixel in rgba.chunks_exact_mut(4) {
        let alpha = u16::from(pixel[3]);
        pixel[0] = (u16::from(pixel[0]) * alpha / 255) as u8;
        pixel[1] = (u16::from(pixel[1]) * alpha / 255) as u8;
        pixel[2] = (u16::from(pixel[2]) * alpha / 255) as u8;
    }
    Some(AppIcon {
        width,
        height,
        rgba: rgba.into(),
    })
}

pub fn discover() -> Vec<Application> {
    let locales = get_languages_from_env();
    let desktops = current_desktop().unwrap_or_default();
    let icon_theme = freedesktop_icons::default_theme_gtk();
    let mut seen = HashSet::new();
    let mut applications = Iter::new(default_paths())
        .entries(Some(&locales))
        .filter(|entry| seen.insert(entry.id().to_owned()))
        .filter_map(|entry| {
            Application::from_entry(entry, &locales, &desktops, icon_theme.as_deref())
        })
        .collect::<Vec<_>>();
    applications.sort_by_cached_key(|application| application.name.to_lowercase());
    applications
}

fn shown_on_desktop(entry: &DesktopEntry, desktops: &[String]) -> bool {
    if entry.only_show_in().is_some_and(|allowed| {
        !allowed
            .iter()
            .any(|name| desktops.iter().any(|d| d == name))
    }) {
        return false;
    }
    !entry.not_show_in().is_some_and(|blocked| {
        blocked
            .iter()
            .any(|name| desktops.iter().any(|d| d == name))
    })
}

fn executable_exists(executable: &str) -> bool {
    let path = Path::new(executable);
    if path.components().count() > 1 {
        return path.is_file();
    }
    env::var_os("PATH")
        .is_some_and(|paths| env::split_paths(&paths).any(|dir| dir.join(path).is_file()))
}

#[cfg(test)]
mod tests {
    use freedesktop_desktop_entry::DesktopEntry;

    use super::{Application, icon_lookup_name};

    fn entry(name: &str, exec: &str) -> DesktopEntry {
        let mut entry = DesktopEntry::from_appid(format!("org.patin.{name}"));
        entry.add_desktop_entry("Type".into(), "Application".into());
        entry.add_desktop_entry("Name".into(), name.into());
        entry.add_desktop_entry("Exec".into(), exec.into());
        entry
    }

    #[test]
    fn accepts_visible_application_and_parses_exec() {
        let application = Application::from_entry(
            entry("Calculator", "calculator --new-window"),
            &[],
            &[],
            None,
        )
        .unwrap();
        assert_eq!(application.name, "Calculator");
        assert_eq!(application.command, ["calculator", "--new-window"]);
    }

    #[test]
    fn rejects_hidden_and_non_application_entries() {
        let mut hidden = entry("Hidden", "hidden");
        hidden.add_desktop_entry("NoDisplay".into(), "true".into());
        assert!(Application::from_entry(hidden, &[], &[], None).is_none());

        let mut link = entry("Website", "browser");
        link.add_desktop_entry("Type".into(), "Link".into());
        assert!(Application::from_entry(link, &[], &[], None).is_none());
    }

    #[test]
    fn preserves_reverse_dns_icon_names_and_strips_real_extensions() {
        assert_eq!(
            icon_lookup_name("org.gnome.Calculator"),
            "org.gnome.Calculator"
        );
        assert_eq!(icon_lookup_name("firefox.png"), "firefox");
        assert_eq!(icon_lookup_name("camera.svg"), "camera");
    }
}
