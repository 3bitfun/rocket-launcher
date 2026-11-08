fn main() {
    let root = std::path::Path::new("resources/rocket-client");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut jars = std::fs::read_dir(root)
        .expect("read bundled Rocket Client builds")
        .map(|entry| entry.expect("read Rocket Client build entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "jar"))
        .collect::<Vec<_>>();
    jars.sort();
    let mut source = String::from("&[\n");
    for jar in jars {
        let version = jar
            .file_stem()
            .and_then(|name| name.to_str())
            .expect("valid version filename");
        assert!(
            version.split('.').all(|part| part.parse::<u32>().is_ok()),
            "invalid game version: {version}"
        );
        let path = jar.canonicalize().expect("resolve Rocket Client build");
        source.push_str(&format!("    ({version:?}, include_bytes!({path:?})),\n"));
    }
    source.push_str("]\n");
    let out =
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo output directory"));
    std::fs::write(out.join("rocket_client_builds.rs"), source)
        .expect("write Rocket Client build index");
    tauri_build::build()
}
