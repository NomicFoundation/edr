fn main() {
    let cargo_toml: toml::Value =
        toml::from_str(include_str!("../../Cargo.toml")).expect("should deserialize Cargo.toml");
    let revm = cargo_toml
        .get("workspace")
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(|dependencies| dependencies.get("revm"))
        .expect("revm dependency not found");
    let revm_version = match revm {
        toml::Value::String(version) => version.clone(),
        toml::Value::Table(detailed) => {
            let Some(toml::Value::String(version)) = detailed.get("version") else {
                panic!("Unrecognized revm dependency format")
            };
            let rev = detailed
                .get("rev")
                .and_then(toml::Value::as_str)
                .map_or(String::new(), |rev| format!("@{rev}"));
            let git = detailed
                .get("git")
                .and_then(toml::Value::as_str)
                .map_or(String::new(), |git| format!("({git}{rev})"));
            format!("{git}{version}")
        }
        _ => panic!("Unrecognized revm dependency format"),
    };
    println!("cargo:rustc-env=REVM_VERSION={revm_version}");
}
