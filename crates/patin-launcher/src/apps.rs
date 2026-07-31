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
    fn from_entry(entry: DesktopEntry, locales: &[String], desktops: &[String]) -> Option<Self> {
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
        Some(Self {
            name,
            icon: entry.icon().and_then(load_icon),
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

fn load_icon(name: &str) -> Option<AppIcon> {
    icon_candidates(name).into_iter().find_map(|path| {
        match path.extension().and_then(|extension| extension.to_str()) {
            Some("png") => decode_png(&path),
            Some("svg") => decode_svg(&path),
            _ => None,
        }
    })
}

fn icon_candidates(name: &str) -> Vec<PathBuf> {
    let supplied = Path::new(name);
    if supplied.is_absolute() {
        return vec![supplied.to_owned()];
    }

    let filenames = if supplied.extension().is_some() {
        vec![name.to_owned()]
    } else {
        vec![
            format!("{name}.png"),
            format!("{name}.svg"),
            format!("{name}-symbolic.svg"),
        ]
    };
    let mut roots = Vec::new();
    if let Some(data_home) = env::var_os("XDG_DATA_HOME") {
        roots.push(PathBuf::from(data_home));
    } else if let Some(home) = env::var_os("HOME") {
        roots.push(PathBuf::from(home).join(".local/share"));
    }
    roots.extend(
        env::var_os("XDG_DATA_DIRS")
            .map(|value| env::split_paths(&value).collect())
            .unwrap_or_else(|| {
                vec![
                    PathBuf::from("/usr/local/share"),
                    PathBuf::from("/usr/share"),
                ]
            }),
    );

    const THEMES: [&str; 2] = ["hicolor", "Adwaita"];
    const SIZES: [&str; 10] = [
        "64x64", "48x48", "96x96", "128x128", "192x192", "256x256", "32x32", "24x24", "22x22",
        "16x16",
    ];
    let mut candidates = Vec::new();
    for root in &roots {
        for theme in THEMES {
            for size in SIZES {
                for filename in &filenames {
                    candidates.push(
                        root.join("icons")
                            .join(theme)
                            .join(size)
                            .join("apps")
                            .join(filename),
                    );
                }
            }
            for filename in &filenames {
                for directory in ["scalable", "symbolic"] {
                    candidates.push(
                        root.join("icons")
                            .join(theme)
                            .join(directory)
                            .join("apps")
                            .join(filename),
                    );
                }
            }
        }
        for filename in &filenames {
            candidates.push(root.join("pixmaps").join(filename));
        }
    }
    candidates
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
    let mut seen = HashSet::new();
    let mut applications = Iter::new(default_paths())
        .entries(Some(&locales))
        .filter(|entry| seen.insert(entry.id().to_owned()))
        .filter_map(|entry| Application::from_entry(entry, &locales, &desktops))
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

    use super::Application;

    fn entry(name: &str, exec: &str) -> DesktopEntry {
        let mut entry = DesktopEntry::from_appid(format!("org.patin.{name}"));
        entry.add_desktop_entry("Type".into(), "Application".into());
        entry.add_desktop_entry("Name".into(), name.into());
        entry.add_desktop_entry("Exec".into(), exec.into());
        entry
    }

    #[test]
    fn accepts_visible_application_and_parses_exec() {
        let application =
            Application::from_entry(entry("Calculator", "calculator --new-window"), &[], &[])
                .unwrap();
        assert_eq!(application.name, "Calculator");
        assert_eq!(application.command, ["calculator", "--new-window"]);
    }

    #[test]
    fn rejects_hidden_and_non_application_entries() {
        let mut hidden = entry("Hidden", "hidden");
        hidden.add_desktop_entry("NoDisplay".into(), "true".into());
        assert!(Application::from_entry(hidden, &[], &[]).is_none());

        let mut link = entry("Website", "browser");
        link.add_desktop_entry("Type".into(), "Link".into());
        assert!(Application::from_entry(link, &[], &[]).is_none());
    }
}
