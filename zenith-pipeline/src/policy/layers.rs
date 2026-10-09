//! Config layers: the global and local policy and brand contract read
//! through a [`ConfigSource`].

use std::path::Path;

use zenith_core::{
    BrandContract, DiagnosticPolicy, merge_brand_contract, parse_brand_contract,
    parse_diagnostic_policy,
};

use super::flags::{PolicyFlags, merge_policy};
use crate::io::{ConfigFile, ConfigSource};

/// The config tiers below the document: global and local policy and brand.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ConfigLayers {
    /// Policy from the global config.
    pub global_policy: DiagnosticPolicy,
    /// Policy from the nearest local config.
    pub local_policy: DiagnosticPolicy,
    /// Brand contract from the global config.
    pub global_brand: BrandContract,
    /// Brand contract from the nearest local config.
    pub local_brand: BrandContract,
}

impl ConfigLayers {
    /// Read both config tiers. The global config is always consulted. The
    /// local config is walked up from `start_dir` when it is `Some`.
    ///
    /// Missing files yield empty tiers.
    ///
    /// # Errors
    ///
    /// Returns a message naming the file on a read error or a malformed
    /// `diagnostics` or `brand` block.
    pub fn load(config: &dyn ConfigSource, start_dir: Option<&Path>) -> Result<Self, String> {
        let global = config.global()?;
        let global_policy = policy_of(global.as_ref())?;
        let global_brand = brand_of(global.as_ref())?;
        let (local_policy, local_brand) = match start_dir {
            Some(dir) => {
                let local = config.local(dir)?;
                (policy_of(local.as_ref())?, brand_of(local.as_ref())?)
            }
            None => (DiagnosticPolicy::default(), BrandContract::default()),
        };
        Ok(Self {
            global_policy,
            local_policy,
            global_brand,
            local_brand,
        })
    }

    /// The effective policy for a document whose in-file policy is
    /// `in_file`: `merge_policy(global, local, in_file, flags)`.
    #[must_use]
    pub fn policy(&self, in_file: &DiagnosticPolicy, flags: &PolicyFlags) -> DiagnosticPolicy {
        merge_policy(&self.global_policy, &self.local_policy, in_file, flags)
    }

    /// The effective brand contract for a document whose in-file contract is
    /// `in_file`. Per category: in-file > local > global.
    #[must_use]
    pub fn brand(&self, in_file: &BrandContract) -> BrandContract {
        merge_brand_contract(
            &merge_brand_contract(&self.global_brand, &self.local_brand),
            in_file,
        )
    }
}

/// The global and local policy tiers alone. Brand blocks are not parsed, so
/// a malformed `brand` block is no error here.
///
/// # Errors
///
/// Returns a message naming the file on a read error or a malformed
/// `diagnostics` block.
pub fn load_policy_layers(
    config: &dyn ConfigSource,
    start_dir: Option<&Path>,
) -> Result<(DiagnosticPolicy, DiagnosticPolicy), String> {
    let global = policy_of(config.global()?.as_ref())?;
    let local = match start_dir {
        Some(dir) => policy_of(config.local(dir)?.as_ref())?,
        None => DiagnosticPolicy::default(),
    };
    Ok((global, local))
}

/// The `diagnostics` policy of `file`. No file is the empty policy.
///
/// # Errors
///
/// Returns `invalid config '<path>': <reason>` for a malformed block.
pub fn policy_of(file: Option<&ConfigFile>) -> Result<DiagnosticPolicy, String> {
    match file {
        Some(file) => parse_diagnostic_policy(&file.bytes)
            .map_err(|e| format!("invalid config '{}': {}", file.path.display(), e.message)),
        None => Ok(DiagnosticPolicy::default()),
    }
}

/// The `brand` contract of `file`. No file, or no `brand` node, is the
/// empty contract.
///
/// # Errors
///
/// Returns `invalid config '<path>': <reason>` for a malformed block.
pub fn brand_of(file: Option<&ConfigFile>) -> Result<BrandContract, String> {
    match file {
        Some(file) => parse_brand_contract(&file.bytes)
            .map_err(|e| format!("invalid config '{}': {}", file.path.display(), e.message)),
        None => Ok(BrandContract::default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemFs;
    use crate::io::{FsConfig, NoConfig};
    use std::path::PathBuf;
    use zenith_core::PolicyVerb;

    #[test]
    fn no_config_is_empty() {
        let layers = ConfigLayers::load(&NoConfig, Some(Path::new("p"))).expect("load");
        assert_eq!(layers, ConfigLayers::default());
    }

    #[test]
    fn local_and_global_policies_load() {
        let fs = MemFs::new()
            .with(
                "home/config.kdl",
                b"diagnostics {\n  deny \"a.code\"\n}\n".to_vec(),
            )
            .with(
                "p/.zenith.kdl",
                b"diagnostics {\n  allow \"b.code\"\n}\n".to_vec(),
            );
        let config = FsConfig::new(&fs, Some(PathBuf::from("home/config.kdl")));
        let layers = ConfigLayers::load(&config, Some(Path::new("p/sub"))).expect("load");
        let merged = layers.policy(&DiagnosticPolicy::default(), &PolicyFlags::default());
        assert_eq!(merged.verb_for("a.code", None), Some(&PolicyVerb::Deny));
        assert_eq!(merged.verb_for("b.code", None), Some(&PolicyVerb::Allow));
        let (global, local) = load_policy_layers(&config, None).expect("policy only");
        assert_eq!(global.entries.len(), 1);
        assert!(local.entries.is_empty());
    }

    #[test]
    fn malformed_config_names_the_file() {
        let fs = MemFs::new().with("p/.zenith.kdl", b"diagnostics { deny }\n".to_vec());
        let config = FsConfig::new(&fs, None);
        let err = ConfigLayers::load(&config, Some(Path::new("p"))).expect_err("bad");
        let want = format!(
            "invalid config '{}'",
            Path::new("p").join(".zenith.kdl").display()
        );
        assert!(err.starts_with(&want), "{err}");
    }
}
