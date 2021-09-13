static REQUIRED_MAJOR: usize = 1;
static REQUIRED_MINOR: usize = 52;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut ac = autocfg::AutoCfg::new()?;

    if !ac.probe_rustc_version(REQUIRED_MAJOR, REQUIRED_MINOR) {
        println!(
            "cargo:warning=rustc version {}.{} or greater required, compilation might fail",
            REQUIRED_MAJOR, REQUIRED_MINOR
        );
    }

    ac.emit_expression_maybe_using_feature(
        "unsafe_op_in_unsafe_fn",
        "{\n#[deny(unknown_lints, unsafe_op_in_unsafe_fn)]\nunsafe fn t() {}\nunsafe { t() }\n}",
    );

    if !ac.emit_type_cfg("!", "supports_never_type") {
        ac.emit_features_with(&["never_type"], |fac| {
            fac.emit_type_cfg("!", "supports_never_type")
        });
    }

    ac.emit_feature("test");

    autocfg::rerun_path("build.rs");

    Ok(())
}

// *** autocfg-with-feature-detection inline vendoring ***

// This vendored fork of autocfg has been modified to conform to the Rust
// edition of the crate, with the main difference being the try! macro which
// has been dropped in favor of the `?` operator. This is otherwise the same
// code except modules have also been inlined and dead code warnings have been
// suppressed, as we don't need to use the full public interface.
mod autocfg {
    #![allow(dead_code)]
    #![deny(missing_debug_implementations)]
    #![deny(missing_docs)]
    // allow future warnings that can't be fixed while keeping 1.0 compatibility
    #![allow(unknown_lints)]
    #![allow(bare_trait_objects)]
    #![allow(ellipsis_inclusive_range_patterns)]

    use std::collections::HashSet;
    use std::env;
    use std::ffi::OsString;
    use std::fs;
    use std::io::{stderr, Write};
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    #[allow(deprecated)]
    use std::sync::atomic::ATOMIC_USIZE_INIT;
    use std::sync::atomic::{AtomicUsize, Ordering};

    mod error {
        use std::error;
        use std::fmt;
        use std::io;
        use std::num;
        use std::str;

        /// A common error type for the `autocfg` crate.
        #[derive(Debug)]
        pub struct Error {
            kind: ErrorKind,
        }

        impl error::Error for Error {
            fn description(&self) -> &str {
                "AutoCfg error"
            }

            fn cause(&self) -> Option<&error::Error> {
                match self.kind {
                    ErrorKind::Io(ref e) => Some(e),
                    ErrorKind::Num(ref e) => Some(e),
                    ErrorKind::Utf8(ref e) => Some(e),
                    ErrorKind::Other(_) => None,
                }
            }
        }

        impl fmt::Display for Error {
            fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
                match self.kind {
                    ErrorKind::Io(ref e) => e.fmt(f),
                    ErrorKind::Num(ref e) => e.fmt(f),
                    ErrorKind::Utf8(ref e) => e.fmt(f),
                    ErrorKind::Other(s) => s.fmt(f),
                }
            }
        }

        #[derive(Debug)]
        enum ErrorKind {
            Io(io::Error),
            Num(num::ParseIntError),
            Utf8(str::Utf8Error),
            Other(&'static str),
        }

        pub fn from_io(e: io::Error) -> Error {
            Error {
                kind: ErrorKind::Io(e),
            }
        }

        pub fn from_num(e: num::ParseIntError) -> Error {
            Error {
                kind: ErrorKind::Num(e),
            }
        }

        pub fn from_utf8(e: str::Utf8Error) -> Error {
            Error {
                kind: ErrorKind::Utf8(e),
            }
        }

        pub fn from_str(s: &'static str) -> Error {
            Error {
                kind: ErrorKind::Other(s),
            }
        }
    }
    pub use error::Error;

    mod version {
        use std::path::Path;
        use std::process::Command;
        use std::str;

        use super::{error, Error};

        /// A version structure for making relative comparisons.
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
        pub struct Version {
            major: usize,
            minor: usize,
            patch: usize,
            extra: Option<String>,
        }

        impl Version {
            /// Creates a `Version` instance for a specific `major.minor.patch` version.
            pub fn new(major: usize, minor: usize, patch: usize) -> Self {
                Version {
                    major,
                    minor,
                    patch,
                    extra: None,
                }
            }

            pub fn from_rustc(rustc: &Path) -> Result<Self, Error> {
                // Get rustc's verbose version
                let output = Command::new(rustc)
                    .args(&["--version", "--verbose"])
                    .output()
                    .map_err(error::from_io)?;
                if !output.status.success() {
                    return Err(error::from_str("could not execute rustc"));
                }
                let output = str::from_utf8(&output.stdout).map_err(error::from_utf8)?;

                // Find the release line in the verbose version output.
                let release = match output.lines().find(|line| line.starts_with("release: ")) {
                    Some(line) => &line["release: ".len()..],
                    None => return Err(error::from_str("could not find rustc release")),
                };

                // Strip off any extra channel info, e.g. "-beta.N", "-nightly", and
                // store the contents after the dash in the `extra` field.
                let (version, extra) = match release.find('-') {
                    Some(i) => (&release[..i], Some(release[i + 1..].to_string())),
                    None => (release, None),
                };

                // Split the version into semver components.
                let mut iter = version.splitn(3, '.');
                let major = iter
                    .next()
                    .ok_or_else(|| error::from_str("missing major version"))?;
                let minor = iter
                    .next()
                    .ok_or_else(|| error::from_str("missing minor version"))?;
                let patch = iter
                    .next()
                    .ok_or_else(|| error::from_str("missing patch version"))?;

                Ok(Version {
                    major: major.parse().map_err(error::from_num)?,
                    minor: minor.parse().map_err(error::from_num)?,
                    patch: patch.parse().map_err(error::from_num)?,
                    extra,
                })
            }

            pub(crate) fn extra(&self) -> Option<&str> {
                #[allow(clippy::option_as_ref_deref)]
                self.extra.as_ref().map(|s| s.as_str())
            }
        }
    }
    use version::Version;

    /// Helper to detect compiler features for `cfg` output in build scripts.
    #[derive(Clone, Debug)]
    pub struct AutoCfg {
        out_dir: PathBuf,
        rustc: PathBuf,
        rustc_version: Version,
        target: Option<OsString>,
        no_std: bool,
        features: HashSet<String>,
        rustflags: Option<Vec<String>>,
    }

    /// Writes a config flag for rustc on standard out.
    ///
    /// This looks like: `cargo:rustc-cfg=CFG`
    ///
    /// Cargo will use this in arguments to rustc, like `--cfg CFG`.
    pub fn emit(cfg: &str) {
        println!("cargo:rustc-cfg={}", cfg);
    }

    /// Writes a line telling Cargo to rerun the build script if `path` changes.
    ///
    /// This looks like: `cargo:rerun-if-changed=PATH`
    ///
    /// This requires at least cargo 0.7.0, corresponding to rustc 1.6.0.  Earlier
    /// versions of cargo will simply ignore the directive.
    pub fn rerun_path(path: &str) {
        println!("cargo:rerun-if-changed={}", path);
    }

    /// Writes a line telling Cargo to rerun the build script if the environment
    /// variable `var` changes.
    ///
    /// This looks like: `cargo:rerun-if-env-changed=VAR`
    ///
    /// This requires at least cargo 0.21.0, corresponding to rustc 1.20.0.  Earlier
    /// versions of cargo will simply ignore the directive.
    pub fn rerun_env(var: &str) {
        println!("cargo:rerun-if-env-changed={}", var);
    }

    /// Create a new `AutoCfg` instance.
    ///
    /// # Panics
    ///
    /// Panics if `AutoCfg::new()` returns an error.
    pub fn new() -> AutoCfg {
        AutoCfg::new().unwrap()
    }

    impl AutoCfg {
        /// Create a new `AutoCfg` instance.
        ///
        /// # Common errors
        ///
        /// - `rustc` can't be executed, from `RUSTC` or in the `PATH`.
        /// - The version output from `rustc` can't be parsed.
        /// - `OUT_DIR` is not set in the environment, or is not a writable directory.
        ///
        pub fn new() -> Result<Self, Error> {
            match env::var_os("OUT_DIR") {
                Some(d) => Self::with_dir(d),
                None => Err(error::from_str("no OUT_DIR specified!")),
            }
        }

        /// Create a new `AutoCfg` instance with the specified output directory.
        ///
        /// # Common errors
        ///
        /// - `rustc` can't be executed, from `RUSTC` or in the `PATH`.
        /// - The version output from `rustc` can't be parsed.
        /// - `dir` is not a writable directory.
        ///
        pub fn with_dir<T: Into<PathBuf>>(dir: T) -> Result<Self, Error> {
            let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
            let rustc: PathBuf = rustc.into();
            let rustc_version = Version::from_rustc(&rustc)?;

            let target = env::var_os("TARGET");

            // Sanity check the output directory
            let dir = dir.into();
            let meta = fs::metadata(&dir).map_err(error::from_io)?;
            if !meta.is_dir() || meta.permissions().readonly() {
                return Err(error::from_str("output path is not a writable directory"));
            }

            // Cargo only applies RUSTFLAGS for building TARGET artifact in
            // cross-compilation environment. Sadly, we don't have a way to detect
            // when we're building HOST artifact in a cross-compilation environment,
            // so for now we only apply RUSTFLAGS when cross-compiling an artifact.
            //
            // See https://github.com/cuviper/autocfg/pull/10#issuecomment-527575030.
            let rustflags = if target != env::var_os("HOST")
                || dir_contains_target(&target, &dir, env::var_os("CARGO_TARGET_DIR"))
            {
                env::var("RUSTFLAGS").ok().map(|rustflags| {
                    // This is meant to match how cargo handles the RUSTFLAG environment
                    // variable.
                    // See https://github.com/rust-lang/cargo/blob/69aea5b6f69add7c51cca939a79644080c0b0ba0/src/cargo/core/compiler/build_context/target_info.rs#L434-L441
                    rustflags
                        .split(' ')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                        .collect::<Vec<String>>()
                })
            } else {
                None
            };

            let mut ac = AutoCfg {
                out_dir: dir,
                rustc,
                rustc_version,
                target,
                no_std: false,
                features: HashSet::new(),
                rustflags,
            };

            // Sanity check with and without `std`.
            if !ac.probe("").unwrap_or(false) {
                ac.no_std = true;
                if !ac.probe("").unwrap_or(false) {
                    // Neither worked, so assume nothing...
                    ac.no_std = false;
                    let warning = b"warning: autocfg could not probe for `std`\n";
                    stderr().write_all(warning).ok();
                }
            }
            Ok(ac)
        }

        /// Test whether the current `rustc` reports a version greater than
        /// or equal to "`major`.`minor`".
        pub fn probe_rustc_version(&self, major: usize, minor: usize) -> bool {
            self.rustc_version >= Version::new(major, minor, 0)
        }

        /// Sets a `cfg` value of the form `rustc_major_minor`, like `rustc_1_29`,
        /// if the current `rustc` is at least that version.
        ///
        /// Returns true if the underlying probe was successful.
        pub fn emit_rustc_version(&self, major: usize, minor: usize) -> bool {
            if self.probe_rustc_version(major, minor) {
                emit(&format!("rustc_{}_{}", major, minor));
                true
            } else {
                false
            }
        }

        fn probe<T: AsRef<[u8]>>(&self, code: T) -> Result<bool, Error> {
            #[allow(deprecated)]
            static ID: AtomicUsize = ATOMIC_USIZE_INIT;

            let id = ID.fetch_add(1, Ordering::Relaxed);
            let mut command = Command::new(&self.rustc);
            command
                .arg("--crate-name")
                .arg(format!("probe{}", id))
                .arg("--crate-type=lib")
                .arg("--out-dir")
                .arg(&self.out_dir)
                .arg("--emit=llvm-ir");

            if let Some(ref rustflags) = self.rustflags {
                command.args(rustflags);
            }

            if let Some(target) = self.target.as_ref() {
                command.arg("--target").arg(target);
            }

            command.arg("-").stdin(Stdio::piped());
            let mut child = command.spawn().map_err(error::from_io)?;
            let mut stdin = child.stdin.take().expect("rustc stdin");

            if self.no_std {
                stdin.write_all(b"#![no_std]\n").map_err(error::from_io)?;
            }

            for feature in &self.features {
                stdin
                    .write_all(format!("#![feature({})]\n", feature).as_bytes())
                    .map_err(error::from_io)?;
            }

            stdin.write_all(code.as_ref()).map_err(error::from_io)?;
            drop(stdin);

            let status = child.wait().map_err(error::from_io)?;
            Ok(status.success())
        }

        /// Tests whether the given sysroot crate can be used.
        ///
        /// The test code is subject to change, but currently looks like:
        ///
        /// ```ignore
        /// extern crate CRATE as probe;
        /// ```
        pub fn probe_sysroot_crate(&self, name: &str) -> bool {
            self.probe(format!("extern crate {} as probe;", name)) // `as _` wasn't stabilized until Rust 1.33
                .unwrap_or(false)
        }

        /// Emits a config value `has_CRATE` if `probe_sysroot_crate` returns true.
        ///
        /// Returns true if the underlying probe was successful.
        pub fn emit_sysroot_crate(&self, name: &str) -> bool {
            if self.probe_sysroot_crate(name) {
                emit(&format!("has_{}", mangle(name)));
                true
            } else {
                false
            }
        }

        /// Tests whether the given path can be used.
        ///
        /// The test code is subject to change, but currently looks like:
        ///
        /// ```ignore
        /// pub use PATH;
        /// ```
        pub fn probe_path(&self, path: &str) -> bool {
            self.probe(format!("pub use {};", path)).unwrap_or(false)
        }

        /// Emits a config value `has_PATH` if `probe_path` returns true.
        ///
        /// Any non-identifier characters in the `path` will be replaced with
        /// `_` in the generated config value.
        ///
        /// Returns true if the underlying probe was successful.
        pub fn emit_has_path(&self, path: &str) -> bool {
            if self.probe_path(path) {
                emit(&format!("has_{}", mangle(path)));
                true
            } else {
                false
            }
        }

        /// Emits the given `cfg` value if `probe_path` returns true.
        ///
        /// Returns true if the underlying probe was successful.
        pub fn emit_path_cfg(&self, path: &str, cfg: &str) -> bool {
            if self.probe_path(path) {
                emit(cfg);
                true
            } else {
                false
            }
        }

        /// Tests whether the given trait can be used.
        ///
        /// The test code is subject to change, but currently looks like:
        ///
        /// ```ignore
        /// pub trait Probe: TRAIT + Sized {}
        /// ```
        pub fn probe_trait(&self, name: &str) -> bool {
            self.probe(format!("pub trait Probe: {} + Sized {{}}", name))
                .unwrap_or(false)
        }

        /// Emits a config value `has_TRAIT` if `probe_trait` returns true.
        ///
        /// Any non-identifier characters in the trait `name` will be replaced with
        /// `_` in the generated config value.
        ///
        /// Returns true if the underlying probe was successful.
        pub fn emit_has_trait(&self, name: &str) -> bool {
            if self.probe_trait(name) {
                emit(&format!("has_{}", mangle(name)));
                true
            } else {
                false
            }
        }

        /// Emits the given `cfg` value if `probe_trait` returns true.
        ///
        /// Returns true if the underlying probe was successful.
        pub fn emit_trait_cfg(&self, name: &str, cfg: &str) -> bool {
            if self.probe_trait(name) {
                emit(cfg);
                true
            } else {
                false
            }
        }

        /// Tests whether the given type can be used.
        ///
        /// The test code is subject to change, but currently looks like:
        ///
        /// ```ignore
        /// pub type Probe = TYPE;
        /// ```
        pub fn probe_type(&self, name: &str) -> bool {
            self.probe(format!("pub type Probe = {};", name))
                .unwrap_or(false)
        }

        /// Emits a config value `has_TYPE` if `probe_type` returns true.
        ///
        /// Any non-identifier characters in the type `name` will be replaced with
        /// `_` in the generated config value.
        ///
        /// Returns true if the underlying probe was successful.
        pub fn emit_has_type(&self, name: &str) -> bool {
            if self.probe_type(name) {
                emit(&format!("has_{}", mangle(name)));
                true
            } else {
                false
            }
        }

        /// Emits the given `cfg` value if `probe_type` returns true.
        ///
        /// Returns true if the underlying probe was successful.
        pub fn emit_type_cfg(&self, name: &str, cfg: &str) -> bool {
            if self.probe_type(name) {
                emit(cfg);
                true
            } else {
                false
            }
        }

        /// Tests whether the given expression can be used.
        ///
        /// The test code is subject to change, but currently looks like:
        ///
        /// ```ignore
        /// pub fn probe() { let _ = EXPR; }
        /// ```
        pub fn probe_expression(&self, expr: &str) -> bool {
            self.probe(format!("pub fn probe() {{ let _ = {}; }}", expr))
                .unwrap_or(false)
        }

        /// Emits the given `cfg` value if `probe_expression` returns true.
        ///
        /// Returns true if the underlying probe was successful.
        pub fn emit_expression_cfg(&self, expr: &str, cfg: &str) -> bool {
            if self.probe_expression(expr) {
                emit(cfg);
                true
            } else {
                false
            }
        }

        /// Tests whether the given constant expression can be used.
        ///
        /// The test code is subject to change, but currently looks like:
        ///
        /// ```ignore
        /// pub const PROBE: () = ((), EXPR).0;
        /// ```
        pub fn probe_constant(&self, expr: &str) -> bool {
            self.probe(format!("pub const PROBE: () = ((), {}).0;", expr))
                .unwrap_or(false)
        }

        /// Emits the given `cfg` value if `probe_constant` returns true.
        ///
        /// Returns true if the underlying probe was successful.
        pub fn emit_constant_cfg(&self, expr: &str, cfg: &str) -> bool {
            if self.probe_constant(expr) {
                emit(cfg);
                true
            } else {
                false
            }
        }

        /// Runs an `action` with `features` enabled and cleans up enabled features
        /// afterwards.
        ///
        /// The returned value will be the boolean returned by the given `action`.
        pub fn probe_features_with<F: FnOnce(&mut Self) -> bool>(
            &mut self,
            features: &[&str],
            probe_fn: F,
        ) -> bool {
            for &feature in features {
                if !self.features.insert(feature.to_string()) {
                    panic!("feature {} enabled twice", feature);
                }
            }

            let res = probe_fn(self);

            for &feature in features {
                self.features.remove(feature);
            }

            res
        }

        /// Emits a config value `feature_FEATURE` for every feature in `features`
        /// if `probe_features_with` returns true.
        ///
        /// Any non-identifier characters in the `feature` will be replaced with
        /// `_` in the generated config value.
        ///
        /// Returns true if the underlying probe was successful.
        pub fn emit_features_with<F: FnOnce(&mut Self) -> bool>(
            &mut self,
            features: &[&str],
            probe_fn: F,
        ) -> bool {
            if self.probe_features_with(features, probe_fn) {
                for &feature in features {
                    emit(&format!("feature_{}", mangle(feature)));
                }
                true
            } else {
                false
            }
        }

        /// Probes the acceptance of a particular `feature`.
        pub fn probe_feature(&mut self, feature: &str) -> bool {
            let features = &[feature];
            self.probe_features_with(features, |ac| ac.probe("").unwrap_or(false))
        }

        /// Emits a config value `feature_FEATURE` if `probe_feature` returns true.
        ///
        /// Any non-identifier characters in the `feature` will be replaced with
        /// `_` in the generated config value.
        ///
        /// Returns true if the underlying probe was successful.
        pub fn emit_feature(&mut self, feature: &str) -> bool {
            self.emit_features_with(&[feature], |ac| ac.probe("").unwrap_or(false))
        }

        /// Returns true if using a nightly channel compiler
        pub fn is_nightly(&self) -> bool {
            self.rustc_version
                .extra()
                .map(|extra| extra.starts_with("nightly"))
                .unwrap_or(false)
        }

        /// Emits paths via `emit_has_path` determining whether `feature` is needed,
        /// and if so it emits the corresponding feature flag.
        ///
        /// If all paths are emitted, then `supports_<feature>` will be emitted.
        ///
        /// Returns true if the underlying probe was successful.
        pub fn emit_paths_maybe_using_feature(&mut self, feature: &str, paths: &[&str]) -> bool {
            let (mut emitted_paths, feature_paths): (Vec<_>, Vec<_>) = paths
                .iter()
                .map(|path| (*path, self.emit_has_path(path)))
                .partition(|(_, result)| *result);

            self.emit_features_with(&[feature], |fac| {
                let emitted_feature_paths = feature_paths
                    .iter()
                    .map(|(path, _)| (*path, fac.emit_has_path(path)))
                    .filter(|(_, result)| *result)
                    .collect::<Vec<_>>();
                emitted_paths.extend(emitted_feature_paths.iter());

                !emitted_feature_paths.is_empty()
            });

            // emit supports_<feature> if all paths are emitted
            if paths
                .iter()
                .all(|&path| emitted_paths.contains(&(path, true)))
            {
                println!("cargo:rustc-cfg=supports_{}", feature);
                true
            } else {
                false
            }
        }

        /// Emits expressions via `emit_expression_cfg` determining whether `feature` is needed,
        /// and if so it emits the corresponding feature flag.
        ///
        /// Uses the format `supports_feature` for the configured feature flag.
        ///
        /// Returns true if the underlying probe was successful.
        pub fn emit_expression_maybe_using_feature(&mut self, feature: &str, expr: &str) -> bool {
            let cfg = format!("supports_{}", feature);
            self.emit_expression_maybe_using_feature_cfg(feature, &cfg, expr)
        }

        /// Emits expressions via `emit_expression_cfg` determining whether `feature` is needed,
        /// and if so it emits the corresponding feature flag.
        ///
        /// Returns true if the underlying probe was successful.
        pub fn emit_expression_maybe_using_feature_cfg(
            &mut self,
            feature: &str,
            cfg: &str,
            expr: &str,
        ) -> bool {
            if !self.emit_expression_cfg(expr, cfg) {
                self.emit_features_with(&[feature], |fac| fac.emit_expression_cfg(expr, cfg))
            } else {
                true
            }
        }

        /// Emits constants via `emit_constant_cfg` determining whether `feature` is needed,
        /// and if so it emits the corresponding feature flag.
        ///
        /// Uses the format `supports_feature` for the configured feature flag.
        ///
        /// Returns true if the underlying probe was successful.
        pub fn emit_constant_maybe_using_feature(&mut self, feature: &str, expr: &str) -> bool {
            let cfg = format!("supports_{}", feature);
            if !self.emit_constant_cfg(expr, &cfg) {
                self.emit_features_with(&[feature], |fac| fac.emit_constant_cfg(expr, &cfg))
            } else {
                true
            }
        }
    }

    fn mangle(s: &str) -> String {
        s.chars()
            .map(|c| match c {
                'A'...'Z' | 'a'...'z' | '0'...'9' => c,
                _ => '_',
            })
            .collect()
    }

    fn dir_contains_target(
        target: &Option<OsString>,
        dir: &Path,
        cargo_target_dir: Option<OsString>,
    ) -> bool {
        target
            .as_ref()
            .and_then(|target| {
                dir.to_str().and_then(|dir| {
                    let mut cargo_target_dir = cargo_target_dir
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from("target"));
                    cargo_target_dir.push(target);

                    cargo_target_dir
                        .to_str()
                        .map(|cargo_target_dir| dir.contains(&cargo_target_dir))
                })
            })
            .unwrap_or(false)
    }
}
