use sha2::{Digest, Sha256};
use std::path::Path;

fn main() {
    let root = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let root = Path::new(&root).parent().unwrap();
    let mut hash = Sha256::new();
    for path in [
        "kernel/src",
        "flow/src",
        "back/src",
        "cli/src",
        "kernel/lean",
        "front/lib",
        "front/read.exs",
        "Cargo.lock",
        "Cargo.toml",
        "kernel/Cargo.toml",
        "flow/Cargo.toml",
        "back/Cargo.toml",
        "cli/Cargo.toml",
        "cli/build.rs",
        "front/mix.exs",
        "lean-toolchain",
        "lakefile.toml",
    ] {
        digest(root, &root.join(path), &mut hash);
    }
    println!("cargo:rustc-env=VEX_BUILD={:x}", hash.finalize());
}

fn digest(root: &Path, path: &Path, hash: &mut Sha256) {
    println!("cargo:rerun-if-changed={}", path.display());
    if path.is_dir() {
        let mut entries = std::fs::read_dir(path)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect::<Vec<_>>();
        entries.sort();
        for entry in entries {
            digest(root, &entry, hash);
        }
    } else {
        hash.update(
            path.strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .as_bytes(),
        );
        hash.update([0]);
        hash.update(std::fs::read(path).unwrap());
        hash.update([0]);
    }
}
