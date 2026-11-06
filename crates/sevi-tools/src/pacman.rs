use crate::{Args, Result};
#[cfg(not(target_os = "linux"))]
pub fn run(_: &Args<'_>) -> Result {
    Err("pacman packaging requires Linux with bsdtar and zstd".into())
}
#[cfg(target_os = "linux")]
pub fn run(a: &Args<'_>) -> Result {
    linux::run(a)
}
#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::{
        collections::{BTreeMap, BTreeSet},
        fs,
        io::Write,
        os::unix::fs::MetadataExt,
        path::{Path, PathBuf},
        process::{Command, Stdio},
    };
    fn command(command: &mut Command, input: Option<&[u8]>) -> Result<Vec<u8>> {
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        if input.is_some() {
            command.stdin(Stdio::piped());
        }
        let mut child = command.spawn()?;
        let stdin = child.stdin.take();
        let (output, written) = std::thread::scope(|scope| {
            let writer = input
                .zip(stdin)
                .map(|(bytes, mut stdin)| scope.spawn(move || stdin.write_all(bytes)));
            let output = child.wait_with_output();
            let written = writer
                .map(|w| {
                    w.join()
                        .map_err(|_| std::io::Error::other("package input writer panicked"))
                        .and_then(|r| r)
                })
                .unwrap_or(Ok(()));
            (output, written)
        });
        let output = output?;
        if !output.status.success() {
            return Err(format!(
                "package tool failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }
        written?;
        Ok(output.stdout)
    }
    fn text(command_: &mut Command, input: Option<&[u8]>) -> Result<String> {
        Ok(String::from_utf8(command(command_, input)?)?)
    }
    fn fields(text: &str) -> BTreeMap<String, String> {
        let mut out = BTreeMap::<String, String>::new();
        let mut last = String::new();
        for line in text.lines() {
            if line.starts_with([' ', '\t']) {
                if let Some(v) = out.get_mut(&last) {
                    v.push(' ');
                    v.push_str(line.trim());
                }
            } else if let Some((name, value)) = line.split_once(':') {
                last = name.into();
                out.insert(last.clone(), value.trim().into());
            }
        }
        out
    }
    pub(super) fn dependencies(source: &str) -> Result<String> {
        let mut out = String::new();
        for dep in source.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let (name, constraint) = dep
                .split_once('(')
                .map(|(name, tail)| (name.trim(), Some(tail.trim_end_matches(')').trim())))
                .unwrap_or((dep, None));
            let name = match name {
                "ibus" => "ibus",
                "libgtk-4-1" => "gtk4",
                "libadwaita-1-0" => "libadwaita",
                "libgstreamer1.0-0" => "gstreamer",
                "gstreamer1.0-plugins-base" => "gst-plugins-base",
                "gstreamer1.0-plugins-good" => "gst-plugins-good",
                "libnotify-bin" => "libnotify",
                "fonts-noto-color-emoji" => "noto-fonts-emoji",
                "curl" => "curl",
                "libc6" => "glibc",
                "libstdc++6" | "libgcc-s1" => "gcc-libs",
                "libglib2.0-0" | "libglib2.0-0t64" => "glib2",
                "libibus-1.0-5" => "ibus",
                "libcairo2" => "cairo",
                "libpango-1.0-0" | "libpangocairo-1.0-0" => "pango",
                "libfontconfig1" => "fontconfig",
                "libx11-6" => "libx11",
                "libegl1" | "libgl1" => "libglvnd",
                "libxkbcommon0" => "libxkbcommon",
                "libwayland-client0" => "wayland",
                _ => return Err(format!("no Arch name for dependency: {name}").into()),
            };
            let suffix = if let Some(constraint) = constraint {
                let mut parts = constraint.split_whitespace();
                let op = match parts.next().ok_or("missing dependency constraint")? {
                    ">=" => ">=",
                    "<=" => "<=",
                    "=" => "=",
                    ">" | ">>" => ">",
                    "<" | "<<" => "<",
                    _ => return Err("unsupported dependency constraint".into()),
                };
                let version = parts.next().ok_or("missing dependency version")?;
                format!("{op}{}", version.split('~').next().unwrap_or(version))
            } else {
                String::new()
            };
            out.push_str(&format!("depend = {name}{suffix}\n"));
        }
        Ok(out)
    }
    fn list(root: &Path) -> Result<Vec<PathBuf>> {
        fn walk(root: &Path, relative: &Path, out: &mut Vec<PathBuf>) -> Result {
            for entry in fs::read_dir(root.join(relative))? {
                let entry = entry?;
                let next = relative.join(entry.file_name());
                out.push(next.clone());
                if fs::symlink_metadata(entry.path())?.is_dir() {
                    walk(root, &next, out)?;
                }
            }
            Ok(())
        }
        let mut out = vec![];
        walk(root, Path::new(""), &mut out)?;
        out.sort();
        Ok(out)
    }
    fn size(root: &Path) -> Result<u64> {
        let mut total = 0;
        let mut seen = BTreeSet::new();
        let mut paths = vec![root.to_owned()];
        paths.extend(list(root)?.into_iter().map(|p| root.join(p)));
        for path in paths {
            let m = fs::symlink_metadata(path)?;
            if seen.insert((m.dev(), m.ino())) {
                total += m.len();
            }
        }
        Ok(total)
    }
    fn names(root: &Path) -> Result<Vec<u8>> {
        use std::os::unix::ffi::OsStrExt;
        let mut bytes = vec![];
        for path in list(root)? {
            bytes.extend_from_slice(path.as_os_str().as_bytes());
            bytes.push(0);
        }
        Ok(bytes)
    }
    pub fn run(a: &Args<'_>) -> Result {
        let deb = PathBuf::from(
            a.args
                .first()
                .ok_or("usage: make-pacman <sevi.deb> [outdir]")?,
        )
        .canonicalize()?;
        let out = a
            .args
            .get(1)
            .map(PathBuf::from)
            .unwrap_or_else(|| deb.parent().unwrap().to_owned());
        fs::create_dir_all(&out)?;
        let out = out.canonicalize()?;
        let task = a.root.join("target/pacman").join(format!(
            "{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_millis()
        ));
        let work = task.join("payload");
        fs::create_dir_all(&work)?;
        let members = text(Command::new("bsdtar").arg("-tf").arg(&deb), None)?;
        let member = |prefix: &str| {
            members
                .lines()
                .find(|s| s.starts_with(prefix))
                .ok_or_else(|| format!("missing Debian {prefix} archive"))
        };
        let control_name = member("control.tar")?;
        let data_name = member("data.tar")?;
        let control_archive = command(
            Command::new("bsdtar")
                .arg("-xOf")
                .arg(&deb)
                .arg(control_name),
            None,
        )?;
        let control = text(
            Command::new("bsdtar").args(["-xOf", "-", "./control"]),
            Some(&control_archive),
        )?;
        let fields = fields(&control);
        let version = fields.get("Version").ok_or("missing Debian version")?;
        let arch = match fields.get("Architecture").map(String::as_str) {
            Some("amd64") => "x86_64",
            Some("arm64") => "aarch64",
            _ => return Err("unsupported Debian architecture".into()),
        };
        let (pkgver, pkgrel) = version.rsplit_once('-').unwrap_or((version, "1"));
        let depends = dependencies(fields.get("Depends").map(String::as_str).unwrap_or(""))?;
        fs::write(work.join(".PKGINFO.depends"), &depends)?;
        let payload = command(
            Command::new("bsdtar").arg("-xOf").arg(&deb).arg(data_name),
            None,
        )?;
        command(
            Command::new("bsdtar").args(["-xf", "-", "-C"]).arg(&work),
            Some(&payload),
        )?;
        let size = size(&work)?;
        let info = format!(
            "# Generated by Linux/packaging/make-pacman.sh from {}\npkgname = sevi\npkgbase = sevi\npkgver = {pkgver}-{pkgrel}\npkgdesc = Sevi - Vietnamese input (Telex) with automatic English/Vietnamese switching\nurl = https://vikey.app\nbuilddate = {}\npackager = Tony Dinh <patent@tdinh.me>\nsize = {size}\narch = {arch}\nlicense = GPL-3.0\n{depends}optdepend = desktop-file-utils: refresh the desktop database\n",
            deb.file_name().unwrap().to_string_lossy(),
            chrono::Utc::now().timestamp()
        );
        fs::write(work.join(".PKGINFO"), info)?;
        fs::rename(work.join(".PKGINFO.depends"), task.join("depends"))?;
        fs::write(
            work.join(".INSTALL"),
            "post_install() {\n  if command -v gtk-update-icon-cache >/dev/null 2>&1; then\n    gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor 2>/dev/null || true\n  fi\n  if command -v update-desktop-database >/dev/null 2>&1; then\n    update-desktop-database -q /usr/share/applications 2>/dev/null || true\n  fi\n}\npost_upgrade() {\n  post_install\n}\n",
        )?;
        let mtree = command(
            Command::new("bsdtar")
                .current_dir(&work)
                .env("LC_ALL", "C")
                .args([
                    "-cnf",
                    "-",
                    "--format=mtree",
                    "--options=!all,use-set,type,uid,gid,mode,time,size,sha256,link",
                    "--uid",
                    "0",
                    "--gid",
                    "0",
                    "--uname",
                    "root",
                    "--gname",
                    "root",
                    "--null",
                    "--files-from",
                    "-",
                ]),
            Some(&names(&work)?),
        )?;
        let mut gzip = flate2::GzBuilder::new()
            .mtime(0)
            .operating_system(3)
            .write(Vec::new(), flate2::Compression::default());
        gzip.write_all(&mtree)?;
        fs::write(work.join(".MTREE"), gzip.finish()?)?;
        let archive = command(
            Command::new("bsdtar")
                .current_dir(&work)
                .env("LC_ALL", "C")
                .args([
                    "-cf",
                    "-",
                    "-n",
                    "--uid",
                    "0",
                    "--gid",
                    "0",
                    "--uname",
                    "root",
                    "--gname",
                    "root",
                    "--null",
                    "--files-from",
                    "-",
                ]),
            Some(&names(&work)?),
        )?;
        let package = out.join(format!("sevi-{pkgver}-{pkgrel}-{arch}.pkg.tar.zst"));
        if package.exists() {
            fs::rename(&package, task.join("previous.pkg.tar.zst"))?;
        }
        command(
            Command::new("zstd")
                .args(["-q", "-T0", "-19", "-f", "-o"])
                .arg(&package),
            Some(&archive),
        )?;
        let entries = text(
            Command::new("bsdtar")
                .arg("-tf")
                .arg(&package)
                .env("LC_ALL", "C"),
            None,
        )?;
        let mut seen = BTreeSet::new();
        for path in entries.lines() {
            if !seen.insert(path) {
                return Err(format!("duplicate entry in {}: {path}", package.display()).into());
            }
        }
        println!("{}", package.display());
        Ok(())
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn arch_dependencies_keep_version_constraints() {
            assert_eq!(
                dependencies("ibus (>= 1.5), libgtk-4-1, libnotify-bin (<< 2.0)").unwrap(),
                "depend = ibus>=1.5\ndepend = gtk4\ndepend = libnotify<2.0\n"
            );
            assert!(dependencies("unknown-library").is_err());
        }
        #[test]
        fn arch_dependencies_accept_generated_shared_library_requirements() {
            assert_eq!(
                dependencies("libc6 (>= 2.38), libgcc-s1 (>= 3.0), libstdc++6 (>= 13.1), libglib2.0-0t64 (>= 2.80), libibus-1.0-5 (>= 1.5.29~beta), libcairo2 (>= 1.14), libpango-1.0-0, libpangocairo-1.0-0, libfontconfig1, libx11-6").unwrap(),
                "depend = glibc>=2.38\ndepend = gcc-libs>=3.0\ndepend = gcc-libs>=13.1\ndepend = glib2>=2.80\ndepend = ibus>=1.5.29\ndepend = cairo>=1.14\ndepend = pango\ndepend = pango\ndepend = fontconfig\ndepend = libx11\n"
            );
            assert_eq!(dependencies("libglib2.0-0").unwrap(), "depend = glib2\n");
        }
    }
}
