use std::path::PathBuf;

const ROOT_PREFIX: &str = "/cloud/";
const BUILD_DIRECTORY_VARIABLE: &str = "CARGO_MANIFEST_DIR";

#[derive(Default, Debug)]
struct TraversalInfo {
    protos: Vec<PathBuf>,
    dirs: Vec<PathBuf>,
}

pub fn build_protos() {
    let Ok(build_directory) = std::env::var(BUILD_DIRECTORY_VARIABLE) else {
        panic!("No manifest directory provided for build");
    };
    let Some(index) = build_directory.find(ROOT_PREFIX) else {
        panic!("No root directory found");
    };
    let root_path = format!("{}{}", &build_directory[0..index], "/protos");
    let proto_paths = find_all_protos(PathBuf::from(&root_path));

    for path in proto_paths {
        dbg!(&path);
        tonic_build::compile_protos(path).expect("Should compile successfully");
    }
}

fn find_all_protos(root: PathBuf) -> Vec<PathBuf> {
    let mut info = TraversalInfo::default();
    info.dirs.push(root);

    while !info.dirs.is_empty() {
        let path = info.dirs.pop().expect("Should not be empty");
        let Ok(directory) = std::fs::read_dir(path) else {
                panic!("Failed to find proto directory");    
            };

        directory
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().is_dir() || !entry.path().ends_with(".proto"))
            .for_each(|entry| {
                dbg!(&entry);
                if entry.path().is_dir() {
                    info.dirs.push(entry.path());
                } else {
                    info.protos.push(entry.path());
                }
            });
    }

    info.protos
}
