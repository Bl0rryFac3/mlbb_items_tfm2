//! Global ordering policy. Settings apply to future builds, across careers.
use std::sync::{OnceLock, atomic::{AtomicU8, Ordering}};
use serde::{Serialize,Deserialize};
use crate::{role_gear::Choice,build_config::Role};
#[derive(Clone,Copy,Debug,Serialize,Deserialize)]
#[serde(default)]
pub(crate) struct Settings { pub jungle_first: bool, pub roam_first: bool }
impl Default for Settings {fn default()->Self{Self{jungle_first:true,roam_first:true}}}
impl Settings {
    pub fn code(self)->u8 {u8::from(self.jungle_first) | (u8::from(self.roam_first)<<1)}
    pub fn decode(bits:u8)->Self{Self{jungle_first:bits&1!=0,roam_first:bits&2!=0}}
}
static FLAGS:AtomicU8=AtomicU8::new(3);
static LOADED:OnceLock<()>=OnceLock::new();
pub(crate) fn own_bits()->u8 {
    LOADED.get_or_init(||{
        let path=crate::config::mod_dir().join("first-slot-rules.json");
        if let Some(settings)=std::fs::read(path).ok().and_then(|b|serde_json::from_slice::<Settings>(&b).ok()) {
            FLAGS.store(settings.code(),Ordering::Relaxed);
        }
    });FLAGS.load(Ordering::Relaxed)
}
pub(crate) fn current()->Settings {Settings::decode(crate::multiplayer::host_first_slot_rules().unwrap_or_else(own_bits))}
pub(crate) fn choose(settings:Settings)->bool {
    // Called only between matches. Existing purchases are never rewritten.
    let _=own_bits();let dir=crate::config::mod_dir();
    let Ok(bytes)=serde_json::to_vec_pretty(&settings) else{return false};
    if persist(&dir.join("first-slot-rules.json"),&bytes).is_err(){return false}
    FLAGS.store(settings.code(),Ordering::Relaxed);
    crate::build_config::invalidate_caches();crate::build_config::pins_changed();true
}
pub(crate) fn apply(choice:Choice,role:Role)->Choice {apply_with(choice,role,current())}
pub(crate) fn apply_with(mut choice:Choice,role:Role,s:Settings)->Choice {
    choice.jungle_first=role==Role::Jungle && s.jungle_first;
    choice.roam_first=role==Role::Support && s.roam_first;
    choice
}
#[cfg(test)] mod tests {
 use super::*;
 #[test] fn old_configuration_defaults_both_on_and_flags_round_trip(){
  let old:Settings=serde_json::from_str("{}").unwrap();assert_eq!(old.code(),3);
  for code in 0..4{assert_eq!(Settings::decode(code).code(),code);}
 }
}

#[cfg(test)] mod policy_tests {
 use super::*;
 #[test] fn independent_global_switches_override_legacy_order_but_preserve_none_and_roles(){
  for code in 0..4 {let s=Settings::decode(code);
   for role in [Role::Jungle,Role::Support,Role::Top,Role::Mid,Role::Bottom] {
    let c=apply_with(Choice::default(),role,s);
    assert_eq!(c.jungle_first,role==Role::Jungle&&s.jungle_first);
    assert_eq!(c.roam_first,role==Role::Support&&s.roam_first);
    let disabled=apply_with(Choice::disabled(),role,s);
    assert_eq!(disabled.jungle,crate::role_gear::Jungle::None);
    assert_eq!(disabled.roam,crate::role_gear::Roam::None);
   }
  }
 }
}

fn persist(path:&std::path::Path,bytes:&[u8])->std::io::Result<()> {
    let tmp=path.with_extension("json.tmp");
    std::fs::write(&tmp,bytes)?;
    std::fs::rename(&tmp,path)
}
#[cfg(test)] mod persistence_tests {
 use super::*;
 #[test] fn global_switch_file_round_trips_and_replaces_prior_settings(){
  let dir=std::env::temp_dir().join(format!("mlbb-first-slot-test-{}",std::process::id()));std::fs::create_dir_all(&dir).unwrap();
  let p=dir.join("first-slot-rules.json");
  for bits in [3,0,1,2] {
   persist(&p,&serde_json::to_vec(&Settings::decode(bits)).unwrap()).unwrap();
   let loaded:Settings=serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();assert_eq!(loaded.code(),bits);
  }
  assert!(!p.with_extension("json.tmp").exists());std::fs::remove_dir_all(dir).unwrap();
 }
}
