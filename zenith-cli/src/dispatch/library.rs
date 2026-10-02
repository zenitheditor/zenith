//! Dispatch logic for `zenith library`.

use std::path::Path;
use std::process::ExitCode;

use crate::cli::{self, LibraryArgs};
use crate::cli_helpers::{parse_at_spec, read_file, resolve_project_dir};
use crate::commands::serialize_pretty;
use crate::json_types::LibraryAddOutput;
use crate::report::CliError;
use crate::{commands, library};

use super::output::apply_edit;

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
                Err(e) => CliError::new("library.show_failed", e.message, e.exit_code)
                    .emit(show_args.json),
            }
        }

        cli::LibrarySub::Search(search_args) => {
            let project_dir = resolve_project_dir(search_args.path.as_deref());
            let packs = library::resolve_packs(project_dir.as_deref());
            let kind = match search_args.kind.as_deref() {
                None => None,
                Some("component") => Some(library::ItemKind::Component),
                Some("token") => Some(library::ItemKind::Token),
                Some("action") => Some(library::ItemKind::Action),
                // clap's `value_parser` rejects anything else before we get here.
                Some(other) => {
                    return CliError::usage(format!(
                        "error: unknown item kind '{other}'; use component, token, or action"
                    ))
                    .emit(search_args.json);
                }
            };
            let category = search_args.category.as_deref().map(str::to_lowercase);
            let options = commands::library::SearchOptions {
                filter: commands::library::SearchFilter {
                    category: category.as_deref(),
                    kind,
                    pack: search_args.pack.as_deref(),
                },
                limit: search_args.limit,
                json: search_args.json,
            };
            println!(
                "{}",
                commands::library::search(&packs, &search_args.query, options)
            );
            ExitCode::SUCCESS
        }

        cli::LibrarySub::Add(add_args) => dispatch_add(add_args),
    }
}

fn dispatch_add(add_args: cli::LibraryAddArgs) -> ExitCode {
    let json = add_args.json;
    let at = match parse_at_spec(add_args.at.as_deref()) {
        Ok(pair) => pair,
        Err(msg) => return CliError::usage(msg).emit(json),
    };
    let target_src = match read_file(&add_args.into) {
        Ok(s) => s,
        Err(e) => return e.emit(json),
    };
    // The project dir is the --into file's parent directory.
    let project_dir = add_args
        .into
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(std::path::Path::to_path_buf);
    let result = match commands::library::add(
        &target_src,
        &add_args.spec,
        project_dir.as_deref(),
        add_args.page.as_deref(),
        at,
        add_args.id.as_deref(),
    ) {
        Ok(result) => result,
        Err(e) => return CliError::new("library.add_failed", e.message, e.exit_code).emit(json),
    };
    let source = match String::from_utf8(result.formatted) {
        Ok(s) => s,
        Err(_) => {
            return CliError::new(
                "io.not_utf8",
                "error[io.not_utf8]: formatted output is not valid UTF-8",
                2,
            )
            .emit(json);
        }
    };
    if add_args.dry_run {
        if json {
            print_add_json(&add_args.into, false, result.summary, Some(source));
        } else {
            print!("{}", source);
        }
        return ExitCode::SUCCESS;
    }
    let asset_root = project_dir.as_deref().unwrap_or_else(|| Path::new("."));
    if let Err(msg) = write_embedded_assets(asset_root, &result.embedded_assets) {
        return CliError::new("library.asset_write_failed", msg, 2).emit(json);
    }
    if let Err(e) = apply_edit(&add_args.into, source.as_bytes(), "library.add") {
        return e.emit(json);
    }
    if json {
        print_add_json(&add_args.into, true, result.summary, None);
    } else {
        println!("{}", result.summary);
    }
    ExitCode::SUCCESS
}

fn print_add_json(path: &Path, written: bool, summary: String, source: Option<String>) {
    let out = LibraryAddOutput {
        schema: "zenith-library-add-v1",
        path: path.display().to_string(),
        written,
        summary,
        source,
    };
    println!("{}", serialize_pretty(&out));
}

fn write_embedded_assets(
    root: &Path,
    assets: &[library::EmbeddedPresetAsset],
) -> Result<(), String> {
    for asset in assets {
        let path = root.join(&asset.src);
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
        let path = root.join(&asset.src);
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

    fn asset() -> library::EmbeddedPresetAsset {
        library::EmbeddedPresetAsset {
            src: "assets/zenith/icons/lucide/test.svg".to_owned(),
            bytes: b"<svg/>",
        }
    }

    #[test]
    fn write_embedded_assets_creates_missing_files_and_reuses_identical() {
        let dir = tempfile::tempdir().expect("tempdir");

        write_embedded_assets(dir.path(), &[asset()]).expect("write asset");
        let path = dir.path().join(asset().src);
        assert_eq!(std::fs::read(&path).expect("read asset"), asset().bytes);

        write_embedded_assets(dir.path(), &[asset()]).expect("identical asset is ok");
        assert_eq!(
            std::fs::read(path).expect("read asset again"),
            asset().bytes
        );
    }

    #[test]
    fn write_embedded_assets_refuses_different_existing_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(asset().src);
        let parent = path.parent().expect("asset path has parent");
        std::fs::create_dir_all(parent).expect("create parent");
        std::fs::write(&path, b"different").expect("write different asset");

        let err = write_embedded_assets(dir.path(), &[asset()]).expect_err("must refuse overwrite");
        assert!(err.contains("refusing to overwrite"), "err: {}", err);
        assert_eq!(std::fs::read(path).expect("read unchanged"), b"different");
    }
}
