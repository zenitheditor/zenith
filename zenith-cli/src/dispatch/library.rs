//! Dispatch logic for `zenith library`.

use std::path::Path;
use std::process::ExitCode;

use crate::cli::{self, LibraryArgs};
use crate::cli_helpers::{parse_at_spec, read_file, resolve_project_dir};
use crate::{commands, history, library};

pub(super) fn dispatch_library(args: LibraryArgs) -> ExitCode {
    match args.command {
        cli::LibrarySub::List(list_args) => {
            // Resolve the project directory: if `path` names an existing
            // file (e.g. a `.zen`), use its parent; if it names a directory,
            // use it directly; if omitted, use the current working directory.
            let project_dir = resolve_project_dir(list_args.path.as_deref());
            let packs = library::resolve_packs(project_dir.as_deref());
            println!("{}", commands::library::list(&packs, list_args.json));
            ExitCode::SUCCESS
        }

        cli::LibrarySub::Show(show_args) => {
            let project_dir = resolve_project_dir(show_args.path.as_deref());
            match commands::library::show(&show_args.spec, project_dir.as_deref(), show_args.json) {
                Ok(out) => {
                    println!("{}", out);
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{}", e.message);
                    ExitCode::from(e.exit_code)
                }
            }
        }

        cli::LibrarySub::Add(add_args) => {
            // Parse the optional `--at "X,Y"` origin up front.
            let at = match parse_at_spec(add_args.at.as_deref()) {
                Ok(pair) => pair,
                Err(msg) => {
                    eprintln!("{}", msg);
                    return ExitCode::from(2);
                }
            };

            let target_src = match read_file(&add_args.into) {
                Ok(s) => s,
                Err(msg) => {
                    eprintln!("{}", msg);
                    return ExitCode::from(2);
                }
            };

            // The project dir is the --into file's parent directory.
            let project_dir = add_args
                .into
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .map(std::path::Path::to_path_buf);

            match commands::library::add(
                &target_src,
                &add_args.spec,
                project_dir.as_deref(),
                add_args.page.as_deref(),
                at,
                add_args.id.as_deref(),
            ) {
                Ok(result) => {
                    if add_args.dry_run {
                        // Print the resulting source WITHOUT writing.
                        match String::from_utf8(result.formatted) {
                            Ok(s) => print!("{}", s),
                            Err(_) => {
                                eprintln!("error: formatted output is not valid UTF-8");
                                return ExitCode::from(2);
                            }
                        }
                    } else {
                        let asset_root = project_dir.as_deref().unwrap_or_else(|| Path::new("."));
                        if let Err(msg) = write_embedded_assets(asset_root, &result.embedded_assets)
                        {
                            eprintln!("{msg}");
                            return ExitCode::from(2);
                        }
                        let recorded =
                            history::record_edit(&result.formatted, &add_args.into, "library.add");
                        if let Some(w) = &recorded.warning {
                            eprintln!("warning: {w}");
                        }
                        if let Err(e) = std::fs::write(&add_args.into, &recorded.bytes) {
                            eprintln!("error writing '{}': {}", add_args.into.display(), e);
                            return ExitCode::from(2);
                        }
                        println!("{}", result.summary);
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{}", e.message);
                    ExitCode::from(e.exit_code)
                }
            }
        }
    }
}

fn write_embedded_assets(
    root: &Path,
    assets: &[library::EmbeddedPresetAsset],
) -> Result<(), String> {
    for asset in assets {
        let path = root.join(asset.src);
        if path.exists() {
            let existing = std::fs::read(&path)
                .map_err(|e| format!("error reading existing asset '{}': {}", path.display(), e))?;
            if existing != asset.bytes {
                return Err(format!(
                    "error: embedded asset target '{}' already exists with different bytes; \
                     refusing to overwrite",
                    path.display()
                ));
            }
        }
    }

    for asset in assets {
        let path = root.join(asset.src);
        if path.exists() {
            continue;
        }
        if let Some(parent) = path.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            return Err(format!(
                "error creating asset directory '{}': {}",
                parent.display(),
                e
            ));
        }
        if let Err(e) = std::fs::write(&path, asset.bytes) {
            return Err(format!(
                "error writing embedded asset '{}': {}",
                path.display(),
                e
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ASSET: library::EmbeddedPresetAsset = library::EmbeddedPresetAsset {
        src: "assets/zenith/icons/lucide/test.svg",
        bytes: b"<svg/>",
    };

    #[test]
    fn write_embedded_assets_creates_missing_files_and_reuses_identical() {
        let dir = tempfile::tempdir().expect("tempdir");

        write_embedded_assets(dir.path(), &[ASSET]).expect("write asset");
        let path = dir.path().join(ASSET.src);
        assert_eq!(std::fs::read(&path).expect("read asset"), ASSET.bytes);

        write_embedded_assets(dir.path(), &[ASSET]).expect("identical asset is ok");
        assert_eq!(std::fs::read(path).expect("read asset again"), ASSET.bytes);
    }

    #[test]
    fn write_embedded_assets_refuses_different_existing_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(ASSET.src);
        let parent = path.parent().expect("asset path has parent");
        std::fs::create_dir_all(parent).expect("create parent");
        std::fs::write(&path, b"different").expect("write different asset");

        let err = write_embedded_assets(dir.path(), &[ASSET]).expect_err("must refuse overwrite");
        assert!(err.contains("refusing to overwrite"), "err: {}", err);
        assert_eq!(std::fs::read(path).expect("read unchanged"), b"different");
    }
}
