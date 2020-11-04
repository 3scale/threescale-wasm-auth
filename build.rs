static REQUIRED_MAJOR: usize = 1;
static REQUIRED_MINOR: usize = 52;

#[allow(dead_code)]
fn emit_paths_maybe_using_feature(
    ac: &mut autocfg::AutoCfg,
    feature: &str,
    paths: &[&str],
) -> bool {
    let (mut emitted_paths, feature_paths): (Vec<_>, Vec<_>) = paths
        .iter()
        .map(|path| (*path, ac.emit_has_path(path)))
        .partition(|(_, result)| *result);

    ac.emit_features_with(&[feature], |fac| {
        let emitted_feature_paths = feature_paths
            .iter()
            .map(|(path, _)| (*path, fac.emit_has_path(path)))
            .filter(|(_, result)| *result)
            .collect::<Vec<_>>();
        emitted_paths.extend(emitted_feature_paths.iter());

        !emitted_feature_paths.is_empty()
    });

    // emit has_<feature> if all paths are emitted
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

#[allow(dead_code)]
fn emit_expression_maybe_using_feature(
    ac: &mut autocfg::AutoCfg,
    feature: &str,
    expr: &str,
) -> bool {
    let cfg = format!("supports_{}", feature);
    emit_expression_maybe_using_feature_cfg(ac, feature, &cfg, expr)
}

fn emit_expression_maybe_using_feature_cfg(
    ac: &mut autocfg::AutoCfg,
    feature: &str,
    cfg: &str,
    expr: &str,
) -> bool {
    if !ac.emit_expression_cfg(expr, cfg) {
        ac.emit_features_with(&[feature], |fac| fac.emit_expression_cfg(expr, cfg))
    } else {
        true
    }
}

#[allow(dead_code)]
fn emit_constant_maybe_using_feature(ac: &mut autocfg::AutoCfg, feature: &str, expr: &str) -> bool {
    let cfg = format!("supports_{}", feature);
    if !ac.emit_constant_cfg(expr, &cfg) {
        ac.emit_features_with(&[feature], |fac| fac.emit_constant_cfg(expr, &cfg))
    } else {
        true
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut ac = autocfg::AutoCfg::new()?;

    if !ac.probe_rustc_version(REQUIRED_MAJOR, REQUIRED_MINOR) {
        println!(
            "cargo:warning=rustc version {}.{} or greater required, compilation might fail",
            REQUIRED_MAJOR, REQUIRED_MINOR
        );
    }

    emit_expression_maybe_using_feature(
        &mut ac,
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
