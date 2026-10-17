use crate::config::Task;
use anyhow::{Result, bail, ensure};
use chrono::{DateTime, Local};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

const TOKENS: [&str; 8] = [
    "timestamp",
    "date",
    "time",
    "task",
    "taskid",
    "taskid8",
    "display",
    "displayindex",
];
fn invalid(c: char) -> bool {
    c.is_control() || "\\/:<>\"|?*".contains(c)
}
fn reserved(s: &str) -> bool {
    let n = s.split('.').next().unwrap_or("").to_uppercase();
    matches!(
        n.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || ["COM", "LPT"].iter().any(|prefix| {
        n.strip_prefix(prefix).is_some_and(|s| {
            matches!(
                s,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    })
}
pub fn sanitize(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| if invalid(c) { '_' } else { c })
        .collect();
    let mut cleaned = cleaned.trim().trim_end_matches(['.', ' ']).to_string();
    if cleaned.is_empty() {
        cleaned = "unnamed".into();
    }
    if reserved(&cleaned) {
        cleaned.insert(0, '_');
    }
    cleaned
}
pub fn validate_template(s: &str) -> Result<()> {
    ensure!(
        !s.trim().is_empty() && s.chars().count() <= 180,
        "Filename template must contain 1–180 characters."
    );
    ensure!(
        !s.chars().any(invalid),
        "Filename contains a Windows path separator or invalid character."
    );
    let mut rest = s;
    while let Some(i) = rest.find('{') {
        ensure!(!rest[..i].contains('}'), "Malformed filename token.");
        let tail = &rest[i + 1..];
        let j = tail
            .find('}')
            .ok_or_else(|| anyhow::anyhow!("Unclosed filename token."))?;
        ensure!(
            TOKENS.contains(&tail[..j].to_lowercase().as_str()),
            "Unknown filename token: {{{}}}.",
            &tail[..j]
        );
        rest = &tail[j + 1..];
    }
    ensure!(!rest.contains('}'), "Malformed filename token.");
    Ok(())
}

pub fn resolve_folder(base: &Path, folder: &str) -> Result<PathBuf> {
    // Match legacy ExpandEnvironmentVariables: unknown variables and literal
    // percent signs are retained, including folders selected by the native picker.
    let mut expanded = String::new();
    let mut tail = folder;
    while let Some(start) = tail.find('%') {
        expanded.push_str(&tail[..start]);
        let rest = &tail[start + 1..];
        let Some(end) = rest.find('%') else {
            expanded.push_str(&tail[start..]);
            tail = "";
            break;
        };
        expanded.push_str(
            &std::env::var(&rest[..end]).unwrap_or_else(|_| format!("%{}%", &rest[..end])),
        );
        tail = &rest[end + 1..];
    }
    expanded.push_str(tail);
    let path = PathBuf::from(expanded);
    ensure!(!path.as_os_str().is_empty(), "Output folder is required.");
    #[cfg(windows)]
    ensure!(
        !path.has_root() || path.is_absolute(),
        "Use a full drive path, such as C:\\Captures."
    );
    #[cfg(windows)]
    ensure!(
        !path
            .components()
            .any(|c| matches!(c, std::path::Component::Prefix(_)))
            || path.is_absolute(),
        "Drive-relative paths are ambiguous; use a full drive path."
    );
    Ok(if path.is_absolute() {
        path
    } else {
        base.join(path)
    })
}

pub fn path_for(
    base: &Path,
    t: &Task,
    at: DateTime<Local>,
    display: &str,
    index: usize,
) -> Result<PathBuf> {
    validate_template(&t.capture.file_name_template)?;
    let mut name = String::new();
    let mut rest = t.capture.file_name_template.as_str();
    while let Some(i) = rest.find('{') {
        name.push_str(&rest[..i]);
        let end = rest[i..].find('}').unwrap() + i;
        let value = match rest[i + 1..end].to_lowercase().as_str() {
            "timestamp" => at.format("%Y%m%d_%H%M%S_%3f").to_string(),
            "date" => at.format("%Y%m%d").to_string(),
            "time" => at.format("%H%M%S").to_string(),
            "task" => sanitize(&t.name),
            "taskid" => t.id.clone(),
            "taskid8" => t.id.replace('-', "").chars().take(8).collect(),
            "display" => sanitize(display.trim_start_matches("\\\\.\\")),
            "displayindex" => index.to_string(),
            _ => unreachable!(),
        };
        name.push_str(&value);
        rest = &rest[end + 1..];
    }
    name.push_str(rest);
    name = sanitize(&name);
    for ext in [".jpeg", ".jpg", ".png"] {
        if name.to_lowercase().ends_with(ext) {
            name.truncate(name.len() - ext.len());
            break;
        }
    }
    name = sanitize(&name);
    ensure!(
        name.encode_utf16().count() <= 220,
        "Expanded filename is too long; shorten the template."
    );
    let mut folder = resolve_folder(base, &t.capture.output_folder)?;
    if t.capture.date_subfolders {
        folder = folder.join(at.format("%Y-%m-%d").to_string());
    }
    Ok(folder.join(format!(
        "{name}.{}",
        if t.capture.image_format == "png" {
            "png"
        } else {
            "jpg"
        }
    )))
}

// Reserve with CREATE_NEW, never exists()+write(). Independent tasks/processes
// cannot overwrite each other. Caller removes only its own file on failure.
pub fn reserve(path: &Path) -> Result<(File, PathBuf)> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Missing output directory."))?;
    fs::create_dir_all(parent)?;
    for n in 0..10_000 {
        let p = if n == 0 {
            path.to_path_buf()
        } else {
            parent.join(format!(
                "{}_{}.{}",
                path.file_stem().unwrap().to_string_lossy(),
                n,
                path.extension().unwrap().to_string_lossy()
            ))
        };
        match OpenOptions::new().write(true).create_new(true).open(&p) {
            Ok(f) => return Ok((f, p)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    bail!("Too many filename collisions; use {{timestamp}} in the template.")
}

pub fn save_image(path: &Path, image: &image::RgbImage, t: &Task) -> Result<PathBuf> {
    let (mut file, path) = reserve(path)?;
    let result = (|| -> Result<()> {
        if t.capture.image_format == "png" {
            image.write_to(&mut file, image::ImageFormat::Png)?;
        } else {
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, t.capture.jpeg_quality)
                .encode_image(image)?;
        }
        file.flush()?;
        file.sync_all()?;
        Ok(())
    })();
    drop(file);
    if let Err(e) = result {
        let _ = fs::remove_file(&path);
        return Err(e);
    }
    Ok(path)
}
