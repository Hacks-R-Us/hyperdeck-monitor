use std::{
    fs::{self, DirEntry},
    io::Error,
    path::PathBuf,
    process::Command,
};

fn main() {
    println!("cargo:rerun-if-changed=package.json");
    println!("cargo:rerun-if-changed=package-lock.json");
    println!("cargo:rerun-if-changed=index.ts");
    println!("cargo:rerun-if-changed=./frontend");

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

    let trunk_build_output = Command::new("trunk")
        .arg("build")
        .arg("--dist")
        .arg("./frontend/dist")
        .arg("./frontend/index.html")
        .spawn()
        .unwrap()
        .wait_with_output()
        .expect("Failed to run `trunk build`");
    if !trunk_build_output.status.success() {
        let err = String::from_utf8(trunk_build_output.stderr).unwrap_or("Unknown".to_string());
        panic!("Running `trunk build` failed, reason {err}");
    }

    let dist_paths = fs::read_dir("./frontend/dist").expect("Could not read dist directory");
    let paths: Vec<Result<DirEntry, Error>> = dist_paths.collect();
    let paths: Vec<(String, PathBuf)> = paths
        .into_iter()
        .filter_map(|path| path.ok())
        .collect::<Vec<DirEntry>>()
        .into_iter()
        .filter_map(|entry| {
            let Ok(file_type) = entry.file_type() else {
                return None;
            };

            if file_type.is_file() {
                let Ok(canonical_path) = entry.path().canonicalize() else {
                    return None;
                };

                canonical_path
                    .to_str()
                    .map(|path_str| (path_str.to_string(), canonical_path.clone()))
            } else {
                None
            }
        })
        .collect();

    for (path_str, path) in paths.iter() {
        println!("{path_str} {path:?}");
    }

    let (index_str, index_path) = paths
        .iter()
        .find(|(path_str, _)| path_str.ends_with("index.html"))
        .expect("Could not find index.html");
    let (wasm_str, wasm_path) = paths
        .iter()
        .find(|(path_str, _)| {
            path_str.contains("hyperdeck_monitor_gui") && path_str.ends_with(".wasm")
        })
        .expect("Could not find WASM source file");
    let (js_str, js_path) = paths
        .iter()
        .find(|(path_str, _)| {
            path_str.contains("hyperdeck_monitor_gui") && path_str.ends_with(".js")
        })
        .expect("Could not find JS source file");
    let (manifest_str, manifest_path) = paths
        .iter()
        .find(|(path_str, _)| path_str.contains("manifest") && path_str.ends_with(".json"))
        .expect("Could not find manifest.json");
    let (service_worker_str, service_worker_path) = paths
        .iter()
        .find(|(path_str, _)| path_str.contains("sw") && path_str.ends_with(".js"))
        .expect("Could not find sw.js");

    // Yeah it's a bit redundant but 🤷
    let index_name = index_path
        .file_name()
        .expect("Could not get file name for index.html")
        .to_str()
        .expect("Could not get file name for index.html");
    let wasm_name = wasm_path
        .file_name()
        .expect("Could not get file name for WASM source file")
        .to_str()
        .expect("Could not get file name for WASM source file");
    let js_name = js_path
        .file_name()
        .expect("Could not get file name for JS source file")
        .to_str()
        .expect("Could not get file name for JS source file");
    let manifest_name = manifest_path
        .file_name()
        .expect("Could not get file name for manifest.json file")
        .to_str()
        .expect("Could not get file name for manifest.json file");
    let service_worker_name = service_worker_path
        .file_name()
        .expect("Could not get file name for service worker file")
        .to_str()
        .expect("Could not get file name for service worker file");

    println!("cargo:rustc-env=INCLUDE_PATH_INDEX={index_str}");
    println!("cargo:rustc-env=INCLUDE_PATH_WASM={wasm_str}");
    println!("cargo:rustc-env=INCLUDE_PATH_JS={js_str}");
    println!("cargo:rustc-env=INCLUDE_PATH_MANIFEST={manifest_str}");
    println!("cargo:rustc-env=INCLUDE_PATH_SERVICE_WORKER={service_worker_str}");

    println!("cargo:rustc-env=FILE_NAME_INDEX={index_name}");
    println!("cargo:rustc-env=FILE_NAME_WASM={wasm_name}");
    println!("cargo:rustc-env=FILE_NAME_JS={js_name}");
    println!("cargo:rustc-env=FILE_NAME_MANIFEST={manifest_name}");
    println!("cargo:rustc-env=FILE_NAME_SERVICE_WORKER={service_worker_name}");
}
