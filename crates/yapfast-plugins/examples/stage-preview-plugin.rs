//! Verify the real local importer and stage a plugin in a new isolated preview profile.
//! The app's normal installation path still shows its consent review.
use std::{fs,path::Path};
use yapfast_plugin_api::PluginManifest;
use yapfast_plugins::{installing,store::Store};
fn main()->anyhow::Result<()> {
 let args:Vec<_>=std::env::args().skip(1).collect();
 anyhow::ensure!((2..=3).contains(&args.len()),"usage: stage-preview-plugin <plugin-folder> <new-profile> [captured-tree.json]");
 let folder=Path::new(&args[0]);let profile=Path::new(&args[1]);
 anyhow::ensure!(!profile.exists(),"Use a new isolated profile path");
 let fetched=installing::from_wasm(&folder.join("yapfast-plugin.json")).map_err(anyhow::Error::msg)?;
 let manifest=PluginManifest::parse(&fetched.manifest)?;
 if let Some(file)=args.get(2){yapfast_plugins::ui::Tree::parse(&fs::read_to_string(file)?).map_err(anyhow::Error::msg)?;}
 let mut store=Store::open(profile)?;store.install(&fetched.package())?;store.set_enabled(&manifest.id,true)?;
 let entry=store.lock()?.plugins.remove(&manifest.id).ok_or_else(||anyhow::anyhow!("Missing staged plugin"))?;
 store.module(&entry,&manifest).map_err(anyhow::Error::msg)?;
 anyhow::ensure!(store.icon(&entry,&manifest).is_some(),"Missing installed icon");
 println!("Verified local importer, manifest, module digest, icon, and UI tree; staged {} in {}",manifest.name,profile.display());Ok(())
}
