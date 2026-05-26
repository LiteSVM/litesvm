use std::process::Command;

fn main() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let fixture_dir = format!("{}/test_programs", manifest_dir);

    let output = Command::new("cargo")
        .arg("build-sbf")
        .current_dir(&fixture_dir)
        .output()
        .expect("Failed to build test fixture programs");

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!(
            "cargo:error=Test program fixtures failed to build:\n{}",
            stderr
        );
        panic!("Build script failed");
    }

    println!("cargo:warning=fixtures initialized");
}
