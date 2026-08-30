use cargo_toml::{Dependency, DependencyDetail, Manifest, VersionReq};

/// `VersionReq`'s `Display` spells out the caret that a bare `"41.0.0"` leaves
/// implicit.
fn render_version_req(version: &VersionReq) -> String {
    let rendered = version.to_string();
    rendered.strip_prefix('^').unwrap_or(&rendered).to_owned()
}

fn main() {
    let cargo_toml: Manifest =
        toml::from_str(include_str!("../../Cargo.toml")).expect("should deserialize Cargo.toml");
    let revm_version = match cargo_toml
        .workspace
        .expect("there is a workspace")
        .dependencies
        .get("revm")
    {
        Some(Dependency::Simple(s)) => render_version_req(s),
        Some(Dependency::Detailed(detailed)) => {
            let DependencyDetail {
                version: Some(version),
                git,
                rev,
                ..
            } = &**detailed
            else {
                panic!("Unrecognized revm dependency format")
            };
            let rev = rev.clone().map_or(String::new(), |rev| format!("@{rev}"));
            let git = git
                .clone()
                .map_or(String::new(), |git| format!("({git}{rev})"));
            format!("{git}{}", render_version_req(version))
        }
        None => panic!("revm dependency not found"),
        _ => panic!("Unrecognized revm dependency format"),
    };
    println!("cargo:rustc-env=REVM_VERSION={revm_version}");
}
