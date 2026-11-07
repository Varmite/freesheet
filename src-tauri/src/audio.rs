//! Read-aloud audio: one Ogg Opus file per notes section (or course phrase), named by an
//! 8-character hash of its text:
//!   audio/grade-<n>/<subject>/<topic>/<id>.ogg
//!   audio/courses/<course>/<id>.ogg
//! On a computer the audio/ folder is installed next to the program. On Android it is
//! inside the app package (its "assets"), read through Android's AssetManager.

use std::path::PathBuf;

pub fn valid_audio_id(id: &str) -> bool {
    id.len() == 8 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Ids of the .ogg files in a folder of audio/ ("grade-10/maths/algebra").
fn ids_from_names<I: IntoIterator<Item = String>>(names: I) -> Vec<String> {
    let mut ids: Vec<String> = names
        .into_iter()
        .filter_map(|n| n.strip_suffix(".ogg").map(str::to_string))
        .filter(|id| valid_audio_id(id))
        .collect();
    ids.sort();
    ids
}

pub struct Audio {
    #[cfg_attr(target_os = "android", allow(dead_code))]
    root: PathBuf,
}

#[cfg(not(target_os = "android"))]
impl Audio {
    pub fn new(root: PathBuf) -> Self {
        Audio { root }
    }

    pub fn list(&self, folder: &str) -> Vec<String> {
        match std::fs::read_dir(self.root.join(folder)) {
            Ok(rd) => ids_from_names(rd.flatten().filter_map(|e| e.file_name().into_string().ok())),
            Err(_) => Vec::new(),
        }
    }

    pub fn read(&self, folder: &str, id: &str) -> Result<Vec<u8>, String> {
        std::fs::read(self.root.join(folder).join(format!("{id}.ogg"))).map_err(|e| e.to_string())
    }
}

#[cfg(target_os = "android")]
impl Audio {
    pub fn new(root: PathBuf) -> Self {
        Audio { root }
    }

    pub fn list(&self, folder: &str) -> Vec<String> {
        android::list(&format!("audio/{folder}")).map(ids_from_names).unwrap_or_default()
    }

    pub fn read(&self, folder: &str, id: &str) -> Result<Vec<u8>, String> {
        android::read(&format!("audio/{folder}/{id}.ogg"))
    }
}

#[cfg(target_os = "android")]
mod android {
    use std::ffi::CString;
    use std::io::Read;
    use std::ptr::NonNull;
    use std::sync::OnceLock;

    /// The app's AAssetManager (kept alive by a global reference to its Java object).
    struct Manager {
        ptr: NonNull<ndk_sys::AAssetManager>,
        _java: jni::objects::GlobalRef,
    }
    // AAssetManager may be used from any thread.
    unsafe impl Send for Manager {}
    unsafe impl Sync for Manager {}

    fn manager() -> Result<&'static Manager, String> {
        static M: OnceLock<Result<Manager, String>> = OnceLock::new();
        M.get_or_init(|| {
            let ctx = ndk_context::android_context();
            let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }.map_err(|e| e.to_string())?;
            let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
            let context = unsafe { jni::objects::JObject::from_raw(ctx.context().cast()) };
            let assets = env
                .call_method(&context, "getAssets", "()Landroid/content/res/AssetManager;", &[])
                .and_then(|v| v.l())
                .map_err(|e| e.to_string())?;
            let java = env.new_global_ref(&assets).map_err(|e| e.to_string())?;
            let raw = unsafe { ndk_sys::AAssetManager_fromJava(env.get_raw().cast(), java.as_obj().as_raw().cast()) };
            let ptr = NonNull::new(raw).ok_or("No AssetManager")?;
            Ok(Manager { ptr, _java: java })
        })
        .as_ref()
        .map_err(Clone::clone)
    }

    fn assets() -> Result<ndk::asset::AssetManager, String> {
        Ok(unsafe { ndk::asset::AssetManager::from_ptr(manager()?.ptr) })
    }

    pub fn list(dir: &str) -> Result<Vec<String>, String> {
        let c = CString::new(dir).map_err(|e| e.to_string())?;
        let d = assets()?.open_dir(&c).ok_or("No such folder")?;
        Ok(d.filter_map(|name| name.into_string().ok()).collect())
    }

    pub fn read(path: &str) -> Result<Vec<u8>, String> {
        let c = CString::new(path).map_err(|e| e.to_string())?;
        let mut a = assets()?.open(&c).ok_or_else(|| format!("Not found: {path}"))?;
        let mut buf = Vec::with_capacity(a.length());
        a.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        Ok(buf)
    }

    /// Sends the app to the background, like the home button (Activity.moveTaskToBack).
    pub fn move_task_to_back() -> Result<(), String> {
        let ctx = ndk_context::android_context();
        let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }.map_err(|e| e.to_string())?;
        let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
        let activity = unsafe { jni::objects::JObject::from_raw(ctx.context().cast()) };
        env.call_method(&activity, "moveTaskToBack", "(Z)Z", &[true.into()]).map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(target_os = "android")]
pub use android::move_task_to_back;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_ids() {
        assert!(valid_audio_id("0faf3e56"));
        assert!(!valid_audio_id("0FAF3E56") && !valid_audio_id("0faf3e5") && !valid_audio_id("../../x"));
        let ids = ids_from_names(["b190471b.ogg", "notes.txt", "89b3fce3.ogg", "zzzzzzzz.ogg"].map(String::from));
        assert_eq!(ids, ["89b3fce3", "b190471b"]);
    }

    #[cfg(not(target_os = "android"))]
    #[test]
    fn reads_the_repo_audio() {
        let a = Audio::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../audio"));
        let ids = a.list("grade-10/english-home-language/drama");
        assert!(!ids.is_empty());
        assert!(a.read("grade-10/english-home-language/drama", &ids[0]).unwrap().starts_with(b"OggS"));
    }
}
