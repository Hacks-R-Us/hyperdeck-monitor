use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=package.json");
    println!("cargo:rerun-if-changed=package-lock.json");
    println!("cargo:rerun-if-changed=index.ts");

    let npm_install_output = Command::new("npm")
        .arg("install")
        .output()
        .expect("Failed to run `npm install`");
    if !npm_install_output.status.success() {
        let err = String::from_utf8(npm_install_output.stderr).unwrap_or("Unknown".to_string());
        panic!("Running `npm install` failed, reason: {err}");
    }

    let ts_build_output = Command::new("npm")
        .arg("run")
        .arg("build")
        .output()
        .expect("Failed to run `npm run build`");
    if !ts_build_output.status.success() {
        let err = String::from_utf8(ts_build_output.stderr).unwrap_or("Unknown".to_string());
        panic!("Running `npm run build` failed, reason: {err}");
    }
}
