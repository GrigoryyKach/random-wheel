//! Dead by Daylight mode: the rules engine and the killer list. Pure logic, no UI and no I/O.
//!
//! There is exactly one state (one player per PC): pool, ban counters, immunity, the lock on
//! the last winner and the history. The rules are described in the README.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Number of bans in committed rounds after which a killer becomes immune for one round.
pub const BANS_FOR_IMMUNITY: u32 = 2;
/// Rounds kept in the history.
pub const HISTORY_LIMIT: usize = 50;

/// The killer list shipped inside the exe (and mirrored in the repository for updates).
pub const EMBEDDED_JSON: &str = include_str!("../data/dbd_killers.json");

// ───────────────────────── killer list ─────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Killer {
    pub id: String,
    pub en: String,
    pub ru: String,
    pub order: u32,
    /// Russian name not confirmed from two sources.
    #[serde(default, skip_serializing_if = "is_false")]
    pub ru_unverified: bool,
    /// No longer sold in the stores; owners still play it.
    #[serde(default, skip_serializing_if = "is_false")]
    pub delisted: bool,
    /// Added by the user (one name for every language).
    #[serde(default, skip_serializing_if = "is_false")]
    pub custom: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct KillerList {
    pub version: u32,
    pub killers: Vec<Killer>,
}

impl KillerList {
    pub fn parse(json: &str) -> Option<KillerList> {
        let mut l: KillerList = serde_json::from_str(json.trim_start_matches('\u{feff}')).ok()?;
        // Drop entries with empty ids and duplicates, keep release order.
        let mut seen = BTreeSet::new();
        l.killers.retain(|k| !k.id.is_empty() && seen.insert(k.id.clone()));
        l.killers.sort_by_key(|k| k.order);
        if l.killers.is_empty() { None } else { Some(l) }
    }

    pub fn embedded() -> KillerList {
        KillerList::parse(EMBEDDED_JSON).expect("embedded killer list is valid")
    }

    /// The list with the higher version wins; on a tie the first one is kept.
    pub fn newest(a: KillerList, b: Option<KillerList>) -> KillerList {
        match b {
            Some(b) if b.version > a.version => b,
            _ => a,
        }
    }
}

impl Killer {
    /// English uses `en`; Russian and Ukrainian both use the official Russian name.
    pub fn name(&self, english: bool) -> &str {
        if english { &self.en } else { &self.ru }
    }

    pub fn custom(id: String, name: &str, order: u32) -> Killer {
        Killer { id, en: name.into(), ru: name.into(), order, ru_unverified: false, delisted: false, custom: true }
    }
}

/// Official list followed by the user's own killers.
pub fn merged(base: &KillerList, custom: &[Killer]) -> Vec<Killer> {
    let mut v = base.killers.clone();
    let max = v.iter().map(|k| k.order).max().unwrap_or(0);
    for (i, c) in custom.iter().enumerate() {
        if v.iter().all(|k| k.id != c.id) {
            let mut c = c.clone();
            c.custom = true;
            c.order = max + 1 + i as u32;
            v.push(c);
        }
    }
    v
}

// ───────────────────────── state ─────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct HistoryEntry {
    pub when: String,
    pub pool_size: usize,
    pub banned: Vec<String>,
    pub winner: String,
}

/// Everything that "Undo last round" restores.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Core {
    /// Killers that take part (checked in the picker).
    pub pool: BTreeSet<String>,
    /// Banned in the current, not yet spun round.
    pub banned: BTreeSet<String>,
    /// Bans in committed rounds, per killer.
    pub counters: BTreeMap<String, u32>,
    /// Immune in the current round (cannot be banned).
    pub immune: BTreeSet<String>,
    /// Won the previous round: sits out the current one.
    pub last_winner: Option<String>,
    pub history: Vec<HistoryEntry>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct State {
    #[serde(flatten)]
    pub core: Core,
    /// State before the last committed round (one step of undo).
    pub prev: Option<Core>,
    /// Killers the user added before the list was updated.
    pub custom: Vec<Killer>,
}

impl State {
    pub fn is_locked(&self, id: &str) -> bool {
        self.core.last_winner.as_deref() == Some(id)
    }

    pub fn is_immune(&self, id: &str) -> bool {
        self.core.immune.contains(id)
    }

    pub fn is_banned(&self, id: &str) -> bool {
        self.core.banned.contains(id)
    }

    pub fn in_pool(&self, id: &str) -> bool {
        self.core.pool.contains(id) && !self.is_locked(id)
    }

    pub fn counter(&self, id: &str) -> u32 {
        self.core.counters.get(id).copied().unwrap_or(0)
    }

    /// Ids of the pool in list order (locked and unknown ids are ignored).
    pub fn pool_ids<'a>(&self, killers: &'a [Killer]) -> Vec<&'a str> {
        killers.iter().filter(|k| self.in_pool(&k.id)).map(|k| k.id.as_str()).collect()
    }

    /// `pool - banned this round - locked last winner`, in list order.
    pub fn eligible<'a>(&self, killers: &'a [Killer]) -> Vec<&'a str> {
        killers.iter().filter(|k| self.in_pool(&k.id) && !self.is_banned(&k.id)).map(|k| k.id.as_str()).collect()
    }

    pub fn banned_count(&self, killers: &[Killer]) -> usize {
        killers.iter().filter(|k| self.in_pool(&k.id) && self.is_banned(&k.id)).count()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn can_spin(&self, killers: &[Killer]) -> bool {
        !self.eligible(killers).is_empty()
    }

    pub fn can_toggle_pool(&self, id: &str) -> bool {
        !self.is_locked(id)
    }

    /// Why the ban button of `id` is unavailable, if it is.
    pub fn ban_block(&self, id: &str) -> Option<BanBlock> {
        if self.is_locked(id) {
            Some(BanBlock::Locked)
        } else if self.is_immune(id) {
            Some(BanBlock::Immune)
        } else if !self.core.pool.contains(id) {
            Some(BanBlock::NotInPool)
        } else {
            None
        }
    }

    pub fn set_in_pool(&mut self, id: &str, on: bool) {
        if on {
            if self.can_toggle_pool(id) {
                self.core.pool.insert(id.to_string());
            }
        } else {
            self.core.pool.remove(id);
            self.core.banned.remove(id);
        }
    }

    pub fn toggle_pool(&mut self, id: &str) {
        let on = !self.core.pool.contains(id);
        self.set_in_pool(id, on);
    }

    pub fn select_all<'a>(&mut self, ids: impl IntoIterator<Item = &'a str>) {
        for id in ids {
            self.set_in_pool(id, true);
        }
    }

    pub fn select_none<'a>(&mut self, ids: impl IntoIterator<Item = &'a str>) {
        for id in ids {
            self.set_in_pool(id, false);
        }
    }

    pub fn invert<'a>(&mut self, ids: impl IntoIterator<Item = &'a str>) {
        for id in ids {
            self.toggle_pool(id);
        }
    }

    /// Ban `id` in the current round. Returns false if it is not allowed.
    pub fn ban(&mut self, id: &str) -> bool {
        if self.ban_block(id).is_some() {
            return false;
        }
        self.core.banned.insert(id.to_string());
        true
    }

    pub fn unban(&mut self, id: &str) {
        self.core.banned.remove(id);
    }

    /// Ends the round with `winner` and commits its effects. `pool_size` and the ban list are
    /// recorded for the history. Returns false (and changes nothing) if `winner` was not eligible.
    pub fn commit(&mut self, killers: &[Killer], winner: &str, when: String) -> bool {
        if !self.eligible(killers).contains(&winner) {
            return false;
        }
        let pool_size = self.pool_ids(killers).len();
        let banned: Vec<String> = killers.iter().filter(|k| self.in_pool(&k.id) && self.is_banned(&k.id)).map(|k| k.id.clone()).collect();
        self.prev = Some(self.core.clone());

        let mut immune = BTreeSet::new();
        for id in &banned {
            let n = self.counter(id) + 1;
            if n >= BANS_FOR_IMMUNITY {
                self.core.counters.remove(id);
                immune.insert(id.clone());
            } else {
                self.core.counters.insert(id.clone(), n);
            }
        }
        // Immunity lasts one round whether or not the killer was in the pool.
        self.core.immune = immune;
        self.core.last_winner = Some(winner.to_string());
        self.core.banned.clear();
        self.core.history.insert(0, HistoryEntry { when, pool_size, banned, winner: winner.to_string() });
        self.core.history.truncate(HISTORY_LIMIT);
        true
    }

    pub fn can_undo(&self) -> bool {
        self.prev.is_some()
    }

    /// One step back: the state from before the last committed round.
    pub fn undo(&mut self) -> bool {
        match self.prev.take() {
            Some(c) => {
                self.core = c;
                true
            }
            None => false,
        }
    }

    /// Clears bans, immunity and the lock. Pool and history stay.
    pub fn reset_counters(&mut self) {
        self.core.counters.clear();
        self.core.immune.clear();
        self.core.banned.clear();
        self.core.last_winner = None;
        self.prev = None;
    }

    pub fn add_custom(&mut self, name: &str, now: u64) -> Option<String> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 40 {
            return None;
        }
        let mut id = format!("custom-{now}");
        while self.custom.iter().any(|k| k.id == id) {
            id.push('x');
        }
        self.custom.push(Killer::custom(id.clone(), name, 0));
        Some(id)
    }

    pub fn remove_custom(&mut self, id: &str) {
        self.custom.retain(|k| k.id != id);
        for c in std::iter::once(&mut self.core).chain(self.prev.iter_mut()) {
            c.pool.remove(id);
            c.banned.remove(id);
            c.counters.remove(id);
            c.immune.remove(id);
            if c.last_winner.as_deref() == Some(id) {
                c.last_winner = None;
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BanBlock {
    Locked,
    Immune,
    NotInPool,
}

/// Case-insensitive match against the displayed name and the English name.
pub fn matches_search(k: &Killer, query: &str, english: bool) -> bool {
    let q = query.trim().to_lowercase();
    q.is_empty() || k.name(english).to_lowercase().contains(&q) || k.en.to_lowercase().contains(&q)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list() -> Vec<Killer> {
        KillerList::embedded().killers
    }

    fn ready(ids: &[&str]) -> State {
        let mut s = State::default();
        s.select_all(ids.iter().copied());
        s
    }

    fn play(s: &mut State, k: &[Killer], winner: &str) {
        assert!(s.commit(k, winner, "t".into()));
    }

    #[test]
    fn embedded_list_is_sane() {
        let l = KillerList::embedded();
        assert_eq!(l.killers.len(), 44);
        let mut ids = BTreeSet::new();
        assert!(l.killers.iter().all(|k| ids.insert(k.id.clone())));
        assert!(l.killers.windows(2).all(|w| w[0].order < w[1].order));
        assert!(l.killers.iter().all(|k| !k.en.is_empty() && !k.ru.is_empty()));
        assert_eq!(l.killers[0].id, "trapper");
        assert!(l.killers.iter().filter(|k| k.delisted).count() >= 2);
    }

    #[test]
    fn parse_rejects_garbage_and_dedups() {
        assert!(KillerList::parse("nope").is_none());
        assert!(KillerList::parse(r#"{"version":1,"killers":[]}"#).is_none());
        let l = KillerList::parse(
            r#"{"version":2,"killers":[{"id":"a","en":"A","ru":"А","order":2},{"id":"a","en":"A2","ru":"А2","order":3},{"id":"b","en":"B","ru":"Б","order":1}]}"#,
        )
        .unwrap();
        assert_eq!(l.killers.iter().map(|k| k.id.as_str()).collect::<Vec<_>>(), ["b", "a"]);
    }

    #[test]
    fn higher_version_wins() {
        let a = KillerList { version: 1, killers: list() };
        let b = KillerList { version: 5, killers: vec![] };
        assert_eq!(KillerList::newest(a.clone(), Some(b)).version, 5);
        let c = KillerList { version: 1, killers: vec![] };
        assert_eq!(KillerList::newest(a.clone(), Some(c)).killers.len(), a.killers.len());
        assert_eq!(KillerList::newest(a.clone(), None).version, 1);
    }

    #[test]
    fn names_follow_language() {
        let k = &list()[0];
        assert_eq!(k.name(true), "The Trapper");
        assert_eq!(k.name(false), "Охотник");
    }

    #[test]
    fn eligible_is_pool_minus_bans_minus_lock() {
        let k = list();
        let mut s = ready(&["trapper", "wraith", "nurse"]);
        assert_eq!(s.eligible(&k), ["trapper", "wraith", "nurse"]);
        assert!(s.ban("wraith"));
        assert_eq!(s.eligible(&k), ["trapper", "nurse"]);
        assert_eq!(s.banned_count(&k), 1);
        s.unban("wraith");
        assert_eq!(s.eligible(&k).len(), 3);
        play(&mut s, &k, "nurse");
        assert!(s.is_locked("nurse"));
        assert_eq!(s.eligible(&k), ["trapper", "wraith"]);
    }

    #[test]
    fn locked_killer_cannot_join_or_be_banned() {
        let k = list();
        let mut s = ready(&["trapper", "nurse"]);
        play(&mut s, &k, "nurse");
        s.set_in_pool("nurse", true);
        assert!(!s.ban("nurse"));
        assert_eq!(s.ban_block("nurse"), Some(BanBlock::Locked));
        s.select_all(["nurse"]);
        assert!(!s.eligible(&k).contains(&"nurse"));
        // The lock lasts one round and then moves on.
        play(&mut s, &k, "trapper");
        assert!(!s.is_locked("nurse"));
        assert!(s.is_locked("trapper"));
        assert!(s.eligible(&k).contains(&"nurse"));
    }

    #[test]
    fn two_bans_make_immunity_for_one_round() {
        let k = list();
        let mut s = ready(&["trapper", "wraith", "nurse", "hag"]);
        s.ban("wraith");
        play(&mut s, &k, "trapper");
        assert_eq!(s.counter("wraith"), 1);
        assert!(!s.is_immune("wraith"));
        s.ban("wraith");
        play(&mut s, &k, "nurse");
        assert_eq!(s.counter("wraith"), 0);
        assert!(s.is_immune("wraith"));
        // Immune: cannot be banned this round.
        assert!(!s.ban("wraith"));
        assert_eq!(s.ban_block("wraith"), Some(BanBlock::Immune));
        play(&mut s, &k, "hag");
        assert!(!s.is_immune("wraith"));
        assert!(s.ban("wraith"));
    }

    #[test]
    fn immunity_ends_even_if_not_in_pool() {
        let k = list();
        let mut s = ready(&["trapper", "wraith", "nurse", "hag"]);
        for w in ["trapper", "nurse"] {
            s.ban("wraith");
            play(&mut s, &k, w);
        }
        assert!(s.is_immune("wraith"));
        s.set_in_pool("wraith", false);
        play(&mut s, &k, "hag");
        assert!(!s.is_immune("wraith"));
    }

    #[test]
    fn commit_requires_an_eligible_winner() {
        let k = list();
        let mut s = ready(&["trapper", "wraith"]);
        s.ban("wraith");
        assert!(!s.commit(&k, "wraith", "t".into()));
        assert!(!s.commit(&k, "nurse", "t".into()));
        assert!(s.core.history.is_empty());
        assert!(!s.can_undo());
    }

    #[test]
    fn single_eligible_killer_can_spin_and_empty_cannot() {
        let k = list();
        let mut s = ready(&["trapper", "wraith"]);
        s.ban("wraith");
        assert!(s.can_spin(&k));
        s.ban("trapper");
        assert!(!s.can_spin(&k));
        assert!(State::default().eligible(&k).is_empty());
    }

    #[test]
    fn undo_restores_the_state_before_the_round() {
        let k = list();
        let mut s = ready(&["trapper", "wraith", "nurse"]);
        s.ban("wraith");
        let before = s.core.clone();
        play(&mut s, &k, "trapper");
        assert!(s.can_undo());
        assert!(s.undo());
        assert_eq!(s.core, before);
        assert!(!s.can_undo());
        assert!(!s.undo());
        assert!(s.core.history.is_empty());
    }

    #[test]
    fn reset_clears_bans_immunity_and_lock() {
        let k = list();
        let mut s = ready(&["trapper", "wraith", "nurse"]);
        s.ban("wraith");
        play(&mut s, &k, "trapper");
        s.core.immune.insert("nurse".into());
        s.ban("wraith");
        s.reset_counters();
        assert_eq!(s.counter("wraith"), 0);
        assert!(s.core.immune.is_empty() && s.core.banned.is_empty());
        assert!(s.core.last_winner.is_none());
        assert_eq!(s.core.history.len(), 1);
        assert!(!s.can_undo());
    }

    #[test]
    fn history_records_round_and_is_capped() {
        let k = list();
        let mut s = ready(&["trapper", "wraith", "nurse"]);
        s.ban("wraith");
        play(&mut s, &k, "nurse");
        let h = &s.core.history[0];
        assert_eq!((h.pool_size, h.banned.clone(), h.winner.as_str()), (3, vec!["wraith".to_string()], "nurse"));
        for i in 0..60 {
            play(&mut s, &k, if i % 2 == 0 { "trapper" } else { "wraith" });
        }
        assert_eq!(s.core.history.len(), HISTORY_LIMIT);
    }

    #[test]
    fn select_all_none_invert_skip_the_lock() {
        let k = list();
        let ids: Vec<&str> = k.iter().take(4).map(|k| k.id.as_str()).collect();
        let mut s = ready(&[ids[0], ids[1]]);
        play(&mut s, &k, ids[0]);
        s.select_none(ids.iter().copied());
        assert!(s.core.pool.is_empty());
        s.select_all(ids.iter().copied());
        assert!(!s.core.pool.contains(ids[0]) || s.is_locked(ids[0]));
        assert_eq!(s.pool_ids(&k).len(), 3);
        s.invert(ids.iter().copied());
        assert_eq!(s.pool_ids(&k), Vec::<&str>::new());
    }

    #[test]
    fn unknown_ids_in_saved_state_are_ignored() {
        let k = list();
        let json = r#"{"pool":["trapper","ghost_from_the_future"],"banned":["nope"],"counters":{"nope":1},"immune":["nope"],"last_winner":"nope","prev":null}"#;
        let s: State = serde_json::from_str(json).unwrap();
        assert_eq!(s.eligible(&k), ["trapper"]);
        assert_eq!(s.pool_ids(&k), ["trapper"]);
        assert_eq!(s.banned_count(&k), 0);
    }

    #[test]
    fn state_is_stored_flat_and_round_trips() {
        let k = list();
        let mut s = ready(&["trapper", "wraith"]);
        s.ban("wraith");
        play(&mut s, &k, "trapper");
        let json = serde_json::to_string(&s).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(v.get("pool").is_some() && v.get("history").is_some() && v.get("profiles").is_none());
        assert_eq!(serde_json::from_str::<State>(&json).unwrap(), s);
        // Missing or empty files give a default state.
        assert_eq!(serde_json::from_str::<State>("{}").unwrap(), State::default());
    }

    #[test]
    fn custom_killers_can_be_added_and_removed() {
        let mut s = State::default();
        assert!(s.add_custom("   ", 1).is_none());
        let id = s.add_custom("The New One", 5).unwrap();
        let all = merged(&KillerList::embedded(), &s.custom);
        assert_eq!(all.len(), 45);
        assert_eq!(all.last().unwrap().name(true), "The New One");
        assert_eq!(all.last().unwrap().name(false), "The New One");
        s.set_in_pool(&id, true);
        assert!(s.in_pool(&id));
        s.remove_custom(&id);
        assert!(s.custom.is_empty() && !s.core.pool.contains(&id));
        let id2 = s.add_custom("X", 5).unwrap();
        let id3 = s.add_custom("Y", 5).unwrap();
        assert_ne!(id2, id3);
    }

    #[test]
    fn search_matches_both_names() {
        let k = &list()[0];
        assert!(matches_search(k, "trap", false));
        assert!(matches_search(k, "охот", false));
        assert!(matches_search(k, "  ", true));
        assert!(!matches_search(k, "wraith", true));
    }
}
