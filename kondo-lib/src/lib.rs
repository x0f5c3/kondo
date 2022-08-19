use jwalk::Parallelism;
use rayon::prelude::*;
use std::borrow::Borrow;
use std::fmt::Error;
use std::iter::FromIterator;
use std::path::PathBuf;
use std::{
    error::{self, Error},
    fs, path,
};

const SYMLINK_FOLLOW: bool = true;

const FILE_CARGO_TOML: &str = "Cargo.toml";
const FILE_PACKAGE_JSON: &str = "package.json";
const FILE_ASSEMBLY_CSHARP: &str = "Assembly-CSharp.csproj";
const FILE_STACK_HASKELL: &str = "stack.yaml";
const FILE_SBT_BUILD: &str = "build.sbt";
const FILE_MVN_BUILD: &str = "pom.xml";
const FILE_CMAKE_BUILD: &str = "CMakeLists.txt";
const FILE_UNREAL_SUFFIX: &str = ".uproject";
const FILE_JUPYTER_SUFFIX: &str = ".ipynb";
const FILE_PYTHON_SUFFIX: &str = ".py";
const FILE_COMPOSER_JSON: &str = "composer.json";

const PROJECT_CARGO_DIRS: [&str; 1] = ["target"];
const PROJECT_NODE_DIRS: [&str; 1] = ["node_modules"];
const PROJECT_UNITY_DIRS: [&str; 7] = [
    "Library",
    "Temp",
    "Obj",
    "Logs",
    "MemoryCaptures",
    "Build",
    "Builds",
];
const PROJECT_STACK_DIRS: [&str; 1] = [".stack-work"];
const PROJECT_SBT_DIRS: [&str; 2] = ["target", "project/target"];
const PROJECT_MVN_DIRS: [&str; 1] = ["target"];
const PROJECT_CMAKE_DIRS: [&str; 1] = ["build"];
const PROJECT_UNREAL_DIRS: [&str; 5] = [
    "Binaries",
    "Build",
    "Saved",
    "DerivedDataCache",
    "Intermediate",
];
const PROJECT_JUPYTER_DIRS: [&str; 1] = [".ipynb_checkpoints"];
const PROJECT_PYTHON_DIRS: [&str; 3] = ["__pycache__", "__pypackages__", ".venv"];
const PROJECT_COMPOSER_DIRS: [&str; 1] = ["vendor"];

const PROJECT_CARGO_NAME: &str = "Cargo";
const PROJECT_NODE_NAME: &str = "Node";
const PROJECT_UNITY_NAME: &str = "Unity";
const PROJECT_STACK_NAME: &str = "Stack";
const PROJECT_SBT_NAME: &str = "SBT";
const PROJECT_MVN_NAME: &str = "Maven";
const PROJECT_CMAKE_NAME: &str = "CMake";
const PROJECT_UNREAL_NAME: &str = "Unreal";
const PROJECT_JUPYTER_NAME: &str = "Jupyter";
const PROJECT_PYTHON_NAME: &str = "Python";
const PROJECT_COMPOSER_NAME: &str = "Composer";

#[derive(Debug, Clone)]
pub enum ProjectType {
    Cargo,
    Node,
    Unity,
    Stack,
    #[allow(clippy::upper_case_acronyms)]
    SBT,
    Maven,
    CMake,
    Unreal,
    Jupyter,
    Python,
    Composer,
}

#[derive(Debug, Clone)]
pub struct Project {
    pub project_type: ProjectType,
    pub path: path::PathBuf,
}

#[derive(Debug, Clone)]
pub struct ProjectSize {
    pub artifact_size: u64,
    pub non_artifact_size: u64,
    pub dirs: Vec<(String, u64, bool)>,
}

impl Project {
    pub fn artifact_dirs(&self) -> &[&str] {
        match self.project_type {
            ProjectType::Cargo => &PROJECT_CARGO_DIRS,
            ProjectType::Node => &PROJECT_NODE_DIRS,
            ProjectType::Unity => &PROJECT_UNITY_DIRS,
            ProjectType::Stack => &PROJECT_STACK_DIRS,
            ProjectType::SBT => &PROJECT_SBT_DIRS,
            ProjectType::Maven => &PROJECT_MVN_DIRS,
            ProjectType::Unreal => &PROJECT_UNREAL_DIRS,
            ProjectType::Jupyter => &PROJECT_JUPYTER_DIRS,
            ProjectType::Python => &PROJECT_PYTHON_DIRS,
            ProjectType::CMake => &PROJECT_CMAKE_DIRS,
            ProjectType::Composer => &PROJECT_COMPOSER_DIRS,
        }
    }

    pub fn name(&self) -> String {
        self.path.to_str().unwrap().to_string()
    }

    pub fn size(&self) -> u64 {
        self.artifact_dirs()
            .iter()
            .copied()
            .map(|p| dir_size(&self.path.join(p)))
            .sum()
    }

    pub fn size_dirs(&self) -> ProjectSize {
        let mut artifact_size = 0;
        let mut non_artifact_size = 0;
        let mut dirs = Vec::new();

        let project_root = match fs::read_dir(&self.path) {
            Err(_) => {
                return ProjectSize {
                    artifact_size,
                    non_artifact_size,
                    dirs,
                }
            }
            Ok(rd) => rd,
        };

        for entry in project_root.filter_map(|rd| rd.ok()) {
            let file_type = match entry.file_type() {
                Err(_) => continue,
                Ok(file_type) => file_type,
            };

            if file_type.is_file() {
                if let Ok(metadata) = entry.metadata() {
                    non_artifact_size += metadata.len();
                }
                continue;
            }

            if file_type.is_dir() {
                let file_name = match entry.file_name().into_string() {
                    Err(_) => continue,
                    Ok(file_name) => file_name,
                };
                let size = dir_size(&entry.path());
                let artifact_dir = self.artifact_dirs().contains(&file_name.as_str());
                if artifact_dir {
                    artifact_size += size;
                } else {
                    non_artifact_size += size;
                }
                dirs.push((file_name, size, artifact_dir));
            }
        }

        ProjectSize {
            artifact_size,
            non_artifact_size,
            dirs,
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self.project_type {
            ProjectType::Cargo => PROJECT_CARGO_NAME,
            ProjectType::Node => PROJECT_NODE_NAME,
            ProjectType::Unity => PROJECT_UNITY_NAME,
            ProjectType::Stack => PROJECT_STACK_NAME,
            ProjectType::SBT => PROJECT_SBT_NAME,
            ProjectType::Maven => PROJECT_MVN_NAME,
            ProjectType::Unreal => PROJECT_UNREAL_NAME,
            ProjectType::Jupyter => PROJECT_JUPYTER_NAME,
            ProjectType::Python => PROJECT_PYTHON_NAME,
            ProjectType::CMake => PROJECT_CMAKE_NAME,
            ProjectType::Composer => PROJECT_COMPOSER_NAME,
        }
    }

    /// Deletes the project's artifact directories and their contents
    pub fn clean(&self) {
        for artifact_dir in self
            .artifact_dirs()
            .iter()
            .copied()
            .map(|ad| self.path.join(ad))
            .filter(|ad| ad.exists())
        {
            if let Err(e) = fs::remove_dir_all(&artifact_dir) {
                eprintln!("error removing directory {:?}: {:?}", artifact_dir, e);
            }
        }
    }
}

fn is_hidden(entry: &jwalk::DirEntry<((), ())>) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

struct ProjectIter {
    it: walkdir::IntoIter,
    it2: jwalk::DirEntryIter<((), ())>,
}

pub enum Red {
    IOError(std::io::Error),
    WalkdirError(jwalk::Error),
}

impl Iterator for ProjectIter {
    type Item = Result<Project, Red>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let entry: jwalk::DirEntry<((), ())> = match self.it2.next() {
                None => return None,
                Some(Err(e)) => return Some(Err(Red::WalkdirError(e))),
                Some(Ok(entry)) => entry,
            };
            if !entry.file_type().is_dir() {
                continue;
            }
            if is_hidden(&entry) {
                self.it.skip_current_dir();
                continue;
            }
            let rd = match entry.path().read_dir() {
                Err(e) => return Some(Err(Red::IOError(e))),
                Ok(rd) => rd,
            };
            return rd
                .into_iter()
                .par_bridge()
                .filter_map(|rd| rd.ok())
                .filter_map(|de| {
                    if let Some(name) = de.file_name().to_str() {
                        return Some((name.to_string(), de.path()));
                    }
                    None
                })
                .filter_map(|(filename, path)| {
                    if let Some(ty) = get_project_type(&filename) {
                        return Some(Ok(Project {
                            project_type: ty,
                            path,
                        }));
                    }
                    None
                })
                .collect::<Vec<_>>()
                .into_iter()
                .next();

            // intentionally ignoring errors while iterating the ReadDir
            // can't return them because we'll lose the context of where we are
            // for dir_entry in to_iter.into_par_iter() {
            //     let file_name = match dir_entry.to_str() {
            //         None => continue,
            //         Some(file_name) => file_name,
            //     };
            //     if let Some(project_type) = get_project_type(file_name) {
            //         self.it.skip_current_dir();
            //         return Some(Ok(Project {
            //             project_type,
            //             path: entry.path(),
            //         }));
            //     }
            // }
        }
    }
}

fn get_project_type(file_name: &str) -> Option<ProjectType> {
    match file_name {
        FILE_CARGO_TOML => Some(ProjectType::Cargo),
        FILE_PACKAGE_JSON => Some(ProjectType::Node),
        FILE_ASSEMBLY_CSHARP => Some(ProjectType::Unity),
        FILE_STACK_HASKELL => Some(ProjectType::Stack),
        FILE_SBT_BUILD => Some(ProjectType::SBT),
        FILE_MVN_BUILD => Some(ProjectType::Maven),
        FILE_CMAKE_BUILD => Some(ProjectType::CMake),
        FILE_COMPOSER_JSON => Some(ProjectType::Composer),
        file_name if file_name.ends_with(FILE_UNREAL_SUFFIX) => Some(ProjectType::Unreal),
        file_name if file_name.ends_with(FILE_JUPYTER_SUFFIX) => Some(ProjectType::Jupyter),
        file_name if file_name.ends_with(FILE_PYTHON_SUFFIX) => Some(ProjectType::Python),
        _ => None,
    }
}

pub fn scan<P: AsRef<path::Path>>(p: &P) -> impl Iterator<Item = Result<Project, Red>> {
    let j = jwalk::WalkDir::new(p)
        .follow_links(SYMLINK_FOLLOW)
        .skip_hidden(true)
        .process_read_dir(|_, _, _, v| {
            v.par_iter_mut()
                .filter_map(|x| x.as_mut().ok())
                .for_each(|mut x| {
                    if !x.file_type.is_dir() {
                        x.read_children_path = None;
                        return;
                    }
                    if let Some(filename) = x.file_name.to_str() {
                        if get_project_type(filename).is_some() {
                            x.read_children_path = None;
                        }
                    }
                })
        })
        .parallelism(Parallelism::RayonDefaultPool)
        .into_iter();
    ProjectIter {
        it: walkdir::WalkDir::new(p)
            .follow_links(SYMLINK_FOLLOW)
            .into_iter(),
        it2: j,
    }
}

pub fn dir_size(path: &path::Path) -> u64 {
    jwalk::WalkDir::new(path)
        .follow_links(SYMLINK_FOLLOW)
        .parallelism(Parallelism::RayonDefaultPool)
        .into_iter()
        .par_bridge()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|e| e.len())
        .sum()
}

pub fn pretty_size(size: u64) -> String {
    const KIBIBYTE: u64 = 1024;
    const MEBIBYTE: u64 = 1_048_576;
    const GIBIBYTE: u64 = 1_073_741_824;
    const TEBIBYTE: u64 = 1_099_511_627_776;
    const PEBIBYTE: u64 = 1_125_899_906_842_624;
    const EXBIBYTE: u64 = 1_152_921_504_606_846_976;

    let (size, symbol) = match size {
        size if size < KIBIBYTE => (size as f64, "B"),
        size if size < MEBIBYTE => (size as f64 / KIBIBYTE as f64, "KiB"),
        size if size < GIBIBYTE => (size as f64 / MEBIBYTE as f64, "MiB"),
        size if size < TEBIBYTE => (size as f64 / GIBIBYTE as f64, "GiB"),
        size if size < PEBIBYTE => (size as f64 / TEBIBYTE as f64, "TiB"),
        size if size < EXBIBYTE => (size as f64 / PEBIBYTE as f64, "PiB"),
        _ => (size as f64 / EXBIBYTE as f64, "EiB"),
    };

    format!("{:.1}{}", size, symbol)
}
#[derive(Debug, Clone)]
pub struct MultiError<E: Error> {
    errs: Vec<E>,
    success: Vec<Project>,
}

impl<E: Error> Error for MultiError<E> {}

impl<E: Error> MultiError<E> {
    pub fn errs(&self) -> &Vec<E> {
        &self.errs
    }
    pub fn success(&self) -> &Vec<Project> {
        &self.success
    }
}

impl<E: Error> FromIterator<Result<Project, E>> for MultiError<E> {
    fn from_iter<T: IntoIterator<Item = Result<Project, E>>>(iter: T) -> Self {
        let mut res = MultiError {
            errs: Vec::new(),
            success: Vec::new(),
        };
        iter.into_iter().for_each(|x: Result<Project, E>| match x {
            Ok(p) => res.success.push(p),
            Err(e) => res.errs.push(e),
        });
        res
    }
}

pub fn clean(project_path: &str) -> Result<(), Box<dyn error::Error>> {
    let project = fs::read_dir(project_path)?
        .par_bridge()
        .filter_map(|rd| rd.ok())
        .find_map_any(|dir_entry| {
            let file_name = dir_entry.file_name().into_string().ok()?;
            // let p_type = match file_name.as_str() {
            //     FILE_CARGO_TOML => Some(ProjectType::Cargo),
            //     FILE_PACKAGE_JSON => Some(ProjectType::Node),
            //     FILE_ASSEMBLY_CSHARP => Some(ProjectType::Unity),
            //     FILE_STACK_HASKELL => Some(ProjectType::Stack),
            //     FILE_SBT_BUILD => Some(ProjectType::SBT),
            //     FILE_MVN_BUILD => Some(ProjectType::Maven),
            //     FILE_CMAKE_BUILD => Some(ProjectType::CMake),
            //     FILE_COMPOSER_JSON => Some(ProjectType::Composer),
            //     _ => None,
            // };
            if let Some(project_type) = get_project_type(&file_name) {
                return Some(Project {
                    project_type,
                    path: project_path.into(),
                });
            }
            None
        })
        .map(|x| {
            x.artifact_dirs()
                .into_par_iter()
                .copied()
                .map(|ad| path::PathBuf::from(project_path).join(ad))
                .filter(|ad| ad.exists())
                .try_for_each(|x| {
                    if let Err(e) = fs::remove_dir_all(x) {
                        eprintln!("error removing directory {:?}: {:?}", x, e);
                        return Err(e);
                    }
                    Ok(())
                })
        });

    if let Some(project) = project {
        for artifact_dir in project
            .artifact_dirs()
            .iter()
            .copied()
            .map(|ad| path::PathBuf::from(project_path).join(ad))
            .filter(|ad| ad.exists())
        {
            if let Err(e) = fs::remove_dir_all(&artifact_dir) {
                eprintln!("error removing directory {:?}: {:?}", artifact_dir, e);
            }
        }
    }

    Ok(())
}
pub fn path_canonicalise(
    base: &path::Path,
    tail: path::PathBuf,
) -> Result<path::PathBuf, Box<dyn Error>> {
    if tail.is_absolute() {
        Ok(tail)
    } else {
        Ok(base.join(tail).canonicalize()?)
    }
}
