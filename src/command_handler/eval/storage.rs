use anyhow::anyhow;
use arc_swap::ArcSwap;
use hebi::ModuleLoader;
use std::{collections::HashMap, env, fs, path::Path, process::Command, sync::Arc};
use tempfile::{tempdir, TempDir};
use tracing::info;

#[derive(Debug, Clone)]
pub struct ModuleStorage {
    pub modules: Arc<ArcSwap<HashMap<String, String>>>,
    temp_dir: Arc<TempDir>,
}

impl ModuleStorage {
    pub fn new(git_url: &str) -> anyhow::Result<ModuleStorage> {
        let temp_dir = Arc::new(tempdir()?);

        info!("Cloning git repo at {git_url}");
        let output = Command::new("git")
            .arg("clone")
            .arg(git_url)
            .arg(temp_dir.path())
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8(output.stderr)?;
            return Err(anyhow!("Could not clone git repo: {stderr}"));
        }

        let modules = Arc::new(load_modules_from_path(temp_dir.path())?.into());

        Ok(Self { modules, temp_dir })
    }

    pub fn update(&self) -> anyhow::Result<Option<String>> {
        let old_commit = get_current_commmit(self.temp_dir.path())?;

        let output = Command::new("git")
            .arg("pull")
            .current_dir(self.temp_dir.path())
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8(output.stderr)?;
            return Err(anyhow!("Could not update git repo: {stderr}"));
        }

        let new_commit = get_current_commmit(self.temp_dir.path())?;

        let new_modules = load_modules_from_path(self.temp_dir.path())?;
        self.modules.store(new_modules);

        if new_commit != old_commit {
            Ok(Some(new_commit))
        } else {
            Ok(None)
        }
    }

    pub fn empty() -> Self {
        info!("Creating empty hebi module storage");
        Self {
            modules: Default::default(),
            temp_dir: Arc::new(TempDir::new().unwrap()),
        }
    }
}

impl ModuleLoader for ModuleStorage {
    fn load(&self, path: &str) -> hebi::Result<hebi::Cow<'static, str>> {
        let modules = self.modules.load();
        match modules.get(path) {
            Some(code) => Ok(hebi::Cow::owned(code.clone())),
            None => Err(hebi::Error::User(format!("Module {path} not found").into())),
        }
    }
}

fn get_current_commmit(path: &Path) -> anyhow::Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(path)
        .output()?;
    let commit = String::from_utf8(output.stdout).unwrap();
    Ok(commit)
}

fn load_modules_from_path(path: &Path) -> anyhow::Result<Arc<HashMap<String, String>>> {
    let mut modules = HashMap::new();
    let read_dir = fs::read_dir(path)?;

    for item in read_dir {
        let item = item?;
        if item.metadata()?.is_file() {
            let file_name = item
                .file_name()
                .into_string()
                .map_err(|err| anyhow!("Module {err:?} does not have a valid name"))?;

            if let Some(module_name) = file_name.strip_suffix(".hebi") {
                let contents = fs::read_to_string(item.path())?;
                modules.insert(module_name.to_owned(), contents);
            }
        }
    }

    info!("Loaded {} hebi modules", modules.len());

    Ok(Arc::new(modules))
}

pub fn create_module_storage_from_env() -> anyhow::Result<ModuleStorage> {
    match env::var("HEBI_MODULES_GIT_URL") {
        Ok(git_url) => ModuleStorage::new(&git_url),
        Err(_) => Ok(ModuleStorage::empty()),
    }
}
