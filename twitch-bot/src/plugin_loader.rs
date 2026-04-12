#![allow(clippy::unwrap_used, clippy::expect_used)]
// TODO: Clean of unwrap and expect

use crate::handler::CommandHandler;
use apatite_api::{API_VERSION, PluginDeclaration};
use libloading::{Library, Symbol};
use std::path::Path;

pub fn load_plugins(handler: &mut CommandHandler) {
    let paths = match std::fs::read_dir("plugins") {
        Ok(paths) => paths,
        Err(_) => {
            return;
        }
    };

    for entry in paths {
        let path = entry.unwrap().path();

        if is_dynamic_lib(&path) {
            unsafe {
                let lib = Library::new(&path).expect("Failed to load plugin");

                let decl: Symbol<*const PluginDeclaration> = lib
                    .get(b"PLUGIN_DECLARATION")
                    .expect("PLUGIN_DECLARATION missing");

                let plugin = &**decl;

                // VERSION CHECK
                if plugin.api_version != API_VERSION {
                    eprintln!(
                        "Skipping plugin {:?}: API mismatch (plugin={}, bot={})",
                        path, plugin.api_version, API_VERSION
                    );
                    continue;
                }

                (plugin.register)(handler);

                std::mem::forget(lib);

                println!("Loaded plugin: {:?}", path);
            }
        }
    }
}

fn is_dynamic_lib(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|s| s.to_str()),
        Some("so" | "dll" | "dylib")
    )
}
