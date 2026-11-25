//! Champion/role/slot outcomes from the existing deduplicated end-inventory capture.
//! Slot means final inventory position, never a fabricated purchase timestamp.
use std::collections::{BTreeMap,BTreeSet};
use serde::{Serialize,Deserialize};
use super::sim::CapturedPlayer;
#[derive(Clone,Copy,Default,Debug,Serialize,Deserialize,PartialEq)]
pub(crate) struct Count {pub games:u32,pub wins:u32}
impl Count {
 fn add(&mut self,won:bool){self.games=self.games.saturating_add(1);self.wins=self.wins.saturating_add(u32::from(won));}
 fn merge(&mut self,x:Self){self.games=self.games.saturating_add(x.games);self.wins=self.wins.saturating_add(x.wins);}
 pub fn rate(self)->Option<f64>{(self.games>0).then(||100.*self.wins as f64/self.games as f64)}
 pub fn adjusted(self)->f64{50.+(self.rate().unwrap_or(50.)-50.)*(self.games.min(60) as f64/60.)}
}
#[derive(Clone,Default)]
pub(crate) struct Table {
 heroes:BTreeMap<(String,String,Option<usize>),Count>,
 slots:BTreeMap<(String,String,Option<usize>,usize,String),Count>,
 balances:BTreeMap<String,Table>,
}
#[derive(Serialize,Deserialize,Default)]
struct Stored {
    #[serde(default)] heroes: Vec<(String, String, Option<usize>, Count)>,
    #[serde(default)] slots: Vec<(String, String, Option<usize>, usize, String, Count)>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    balances: BTreeMap<String, Stored>,
}
#[derive(Clone,Debug)]
pub(crate) struct Row {pub key:String,pub count:Count,pub picks:f64}
#[derive(Default)]
pub(crate) struct View {pub hero:Count,pub denominators:[u32;6],pub slots:[Vec<Row>;6]}
impl Table {
 pub fn fold(&mut self,patch:&str,players:&[CapturedPlayer]){
  for p in players.iter().filter(|p|!p.champion.is_empty()){
   self.heroes.entry((patch.into(),p.champion.clone(),p.lane)).or_default().add(p.won);
   for(slot,key)in p.items.iter().take(6).enumerate(){
    if key.is_empty(){continue}
    self.slots.entry((patch.into(),p.champion.clone(),p.lane,slot,key.clone())).or_default().add(p.won);
   }
  }
 }
 pub fn fold_revision(&mut self, patch: &str, players: &[CapturedPlayer], revision: Option<&str>) {
    self.fold(patch, players);
    if let Some(revision) = revision {
        self.balances.entry(revision.to_owned()).or_default().fold(patch, players);
    }
 }
 pub fn under_balance(&self, revision: Option<&str>) -> Self {
    let Some(selected) = revision.map_or(Some(self), |revision| self.balances.get(revision)) else {
        return Self::default();
    };
    Self { heroes: selected.heroes.clone(), slots: selected.slots.clone(), balances: BTreeMap::new() }
 }
 pub fn hero_counts(&self, patch: Option<&str>) -> BTreeMap<String, Count> {
    let mut counts: BTreeMap<_, Count> = BTreeMap::new();
    for ((record_patch, champion, _), count) in &self.heroes {
        if patch.is_none_or(|selected| selected == record_patch) {
            counts.entry(champion.clone()).or_default().merge(*count);
        }
    }
    counts
 }
 pub fn champions(&self)->BTreeSet<String>{self.heroes.keys().map(|(_,c,_)|c.clone()).collect()}
 pub fn patches(&self)->Vec<String>{let mut p:Vec<_>=self.heroes.keys().map(|(p,_,_)|p.clone()).collect::<BTreeSet<_>>().into_iter().collect();p.sort_by_cached_key(|s|s.split('.').map(|n|n.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>());p.reverse();p}
 pub fn view(&self,champion:&str,lane:Option<usize>,patch:Option<&str>,eligible:&BTreeSet<String>)->View{
  let matches=|p:&str,c:&str,l:Option<usize>|c==champion&&patch.is_none_or(|x|x==p)&&lane.is_none_or(|x|Some(x)==l);
  let mut v=View::default();
  for((p,c,l),n)in &self.heroes{if matches(p,c,*l){v.hero.merge(*n);}}
  let mut rows:[BTreeMap<String,Count>;6]=Default::default();
  for((p,c,l,slot,key),n)in &self.slots{
   if *slot>=6||!matches(p,c,*l){continue}
   v.denominators[*slot]=v.denominators[*slot].saturating_add(n.games);
   if eligible.contains(key){rows[*slot].entry(key.clone()).or_default().merge(*n);}
  }
  for(slot,rows)in rows.into_iter().enumerate(){
   v.slots[slot]=rows.into_iter().map(|(key,count)|Row{key,count,picks:100.*count.games as f64/v.denominators[slot].max(1) as f64}).collect();
   v.slots[slot].sort_by(|a,b|b.count.adjusted().total_cmp(&a.count.adjusted()).then(b.count.games.cmp(&a.count.games)).then(a.key.cmp(&b.key)));
  }v
 }
 fn stored(&self) -> Stored {
    Stored {
        heroes: self.heroes.iter().map(|((p,c,l),n)| (p.clone(),c.clone(),*l,*n)).collect(),
        slots: self.slots.iter().map(|((p,c,l,s,k),n)| (p.clone(),c.clone(),*l,*s,k.clone(),*n)).collect(),
        balances: self.balances.iter().map(|(key, table)| (key.clone(), table.stored())).collect(),
    }
 }
 pub fn encode(&self) -> serde_json::Value {
    serde_json::to_value(self.stored()).unwrap_or_default()
 }
 pub fn decode(v: &serde_json::Value) -> Self {
    let Ok(stored) = serde_json::from_value::<Stored>(v.clone()) else { return Self::default() };
    Self::from_stored(stored, true)
 }
 fn from_stored(stored: Stored, allow_balances: bool) -> Self {
    let mut table = Self::default();
    for (patch, champion, lane, count) in stored.heroes {
        if count.wins <= count.games && lane.is_none_or(|lane| lane < 5) {
            table.heroes.insert((patch, champion, lane), count);
        }
    }
    for (patch, champion, lane, slot, key, count) in stored.slots {
        if slot < 6 && count.wins <= count.games && lane.is_none_or(|lane| lane < 5) {
            table.slots.insert((patch, champion, lane, slot, key), count);
        }
    }
    if allow_balances {
        table.balances = stored.balances.into_iter()
            .map(|(revision, table)| (revision, Self::from_stored(table, false))).collect();
    }
    table
 }
}
pub(crate) fn family(k:&str)->&str{k.strip_prefix("mythical_").unwrap_or(k)}
pub(crate) fn recommended(view: &View) -> Vec<Option<String>> {
    fill_open_slots(view, &[])
}

/// Fill only open slots, including when a base or Mythical family is pinned elsewhere.
pub(crate) fn fill_open_slots(view: &View, current: &[Option<String>]) -> Vec<Option<String>> {
    let mut plan: Vec<_> = current.iter().cloned().chain(std::iter::repeat(None)).take(6).collect();
    let mut seen: BTreeSet<_> = plan.iter().flatten().map(|key| family(key).to_owned()).collect();
    for (pick, candidates) in plan.iter_mut().zip(&view.slots) {
        if pick.is_some() {
            continue;
        }
        if let Some(row) = candidates.iter().find(|row| row.count.games >= 10 && !seen.contains(family(&row.key))) {
            seen.insert(family(&row.key).to_owned());
            *pick = Some(row.key.clone());
        }
    }
    plan
}
#[cfg(test)] mod tests {
 use super::*;
 fn player(c:&str,l:usize,keys:Vec<&str>,won:bool)->CapturedPlayer{CapturedPlayer{champion:c.into(),lane:Some(l),items:keys.into_iter().map(str::to_string).collect(),won}}
 #[test]fn slot_rates_use_champion_role_and_reached_slot_denominators(){
  let mut t=Table::default();t.fold("a",&[player("karrie",3,vec!["boots","staff"],true),player("karrie",3,vec!["boots"],false),player("karrie",2,vec!["wand"],true),player("eudora",2,vec!["wand"],false)]);
  t.fold("b",&[player("karrie",3,vec!["component"],false)]);
  let eligible=BTreeSet::from(["boots".into(),"staff".into(),"wand".into()]);
  let v=t.view("karrie",Some(3),Some("a"),&eligible);assert_eq!(v.hero,Count{games:2,wins:1});assert_eq!(v.denominators,[2,1,0,0,0,0]);assert_eq!(v.slots[0][0].picks,100.);assert_eq!(v.slots[0][0].count.rate(),Some(50.));
  let all=t.view("karrie",None,None,&eligible);assert_eq!(all.hero.games,4);assert_eq!(all.slots[0].iter().find(|r|r.key=="boots").unwrap().picks,50.);
 }
 #[test]fn recommendation_shrinks_lucky_samples_filters_under_ten_and_rejects_duplicate_families(){
  assert!(Count{games:60,wins:36}.adjusted()>Count{games:10,wins:8}.adjusted());
  let mut v=View::default();v.slots[0]=vec![Row{key:"staff".into(),count:Count{games:10,wins:7},picks:80.}];v.slots[1]=vec![Row{key:"mythical_staff".into(),count:Count{games:40,wins:30},picks:80.},Row{key:"boots".into(),count:Count{games:9,wins:9},picks:20.}];
  assert_eq!(recommended(&v),vec![Some("staff".into()),None,None,None,None,None]);
 }
 #[test]
 fn balance_history_keeps_old_results_and_filters_new_results() {
    let mut table = Table::default();
    let players = &[player("estes", 4, vec!["boots"], true)];
    table.fold("a", players);
    table.fold_revision("a", players, Some("1234"));
    let decoded = Table::decode(&table.encode());
    for (balance, count) in [(None, 2), (Some("1234"), 1), (Some("5678"), 0)] {
        assert_eq!(decoded.under_balance(balance).view("estes", None, None, &BTreeSet::new()).hero.games, count);
    }
 }
 #[test]
 fn recommendations_keep_existing_pins_and_avoid_duplicate_families() {
    let mut view = View::default();
    let count = Count { games: 60, wins: 36 };
    for key in ["mythical_staff", "wand"] {
        view.slots[0].push(Row { key: key.into(), count, picks: 25. });
    }
    let plan = fill_open_slots(&view, &[None, Some("staff".into())]);
    assert_eq!(plan[..2], [Some("wand".into()), Some("staff".into())]);
 }
 #[test]fn save_round_trip_preserves_slots_and_old_missing_table_starts_empty(){
  let mut t=Table::default();t.fold("a",&[player("estes",4,vec!["boots"],true)]);let restored=Table::decode(&t.encode());assert_eq!(restored.heroes,t.heroes);assert_eq!(restored.slots,t.slots);assert!(Table::decode(&serde_json::Value::Null).heroes.is_empty());

 }
}
