//! BufferedMetadataProvider - A mutable metadata provider for encoding hints.
//!
//! This module provides a simple, thread-safe implementation of the MetadataProvider
//! trait that allows programmatic setting of key-value pairs. It's primarily used
//! for passing encoding hints (IMODE, IC, NPPBH, NPPBV, COMRAT) to the dataset writer.

use std::collections::HashMap;
use std::sync::RwLock;

use serde_json::Value;

use crate::traits::MetadataProvider;

/// A mutable metadata provider that stores JSON key-value pairs.
///
/// This provider allows programmatic setting of metadata values, making it useful
/// for creating assets with custom encoding hints. It implements the MetadataProvider
/// trait, allowing it to be used anywhere a MetadataProvider is expected.
///
/// # Repeated keys
///
/// Each key maps to an *ordered list* of values rather than a single value, because
/// some formats let a key repeat within one container — a NITF subheader may hold a
/// sequence of tagged record extensions and repeat a CETAG (STDI-0002 Volume 1 §2;
/// JBP §5.9.2). Ordinary keys hold exactly one element, so that list is the single
/// source of truth for the whole dictionary surface: [`Self::get_value`],
/// [`MetadataProvider::entries`], [`MetadataProvider::keys`], and
/// [`MetadataProvider::len`] all project instance 0, which is what makes
/// `get_value(key) == get_all(key)[0]` hold by construction.
///
/// [`Self::set`] and [`Self::remove`] are whole-slot operations: assigning to a key
/// that holds several values discards all of them. [`Self::set_all`] and
/// [`Self::append`] are the mutators that author more than one.
///
/// # Thread Safety
///
/// BufferedMetadataProvider is thread-safe (Send + Sync) and uses RwLock for
/// concurrent read access with exclusive write access.
pub struct BufferedMetadataProvider {
    data: RwLock<HashMap<String, Vec<Value>>>,
}

impl BufferedMetadataProvider {
    /// Create a new empty BufferedMetadataProvider.
    pub fn new() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
        }
    }

    /// Create a BufferedMetadataProvider initialized from an existing MetadataProvider.
    ///
    /// Every key is copied through [`MetadataProvider::get_all`], so repeated values
    /// survive with no special case. `get_all` is total over
    /// [`MetadataProvider::keys`] — one element for an ordinary key, one per instance
    /// for a repeated one — which is what lets this be a single uniform loop. An
    /// earlier version partitioned the keys and copied part of them through
    /// `entries()`, where the first-value-only projection made the copy order-sensitive
    /// and silently lossy if it ran the wrong way round; there is nothing left to get
    /// in the wrong order.
    pub fn from_provider(source: &dyn MetadataProvider) -> Self {
        let data: HashMap<String, Vec<Value>> = source
            .keys()
            .into_iter()
            .map(|key| {
                let instances = source.get_all(&key);
                (key, instances)
            })
            .filter(|(_, instances)| !instances.is_empty())
            .collect();

        Self {
            data: RwLock::new(data),
        }
    }

    /// Set a value for the given key. Accepts any `serde_json::Value`.
    ///
    /// This replaces the **whole slot**: if the key held several values, all of them
    /// are discarded. Returns the number of instances that were stored under the key
    /// beforehand, so callers can warn when an assignment silently narrows a repeated
    /// key to one value.
    pub fn set(&self, key: &str, value: Value) -> usize {
        self.set_all(key, vec![value])
    }

    /// Replace every value stored under `key` with `instances`, in order.
    ///
    /// Order is load-bearing when instances form a sequence — `CSEPHA` instances are
    /// defined in time-sequence order (STDI-0002 Vol 1 App D) and `BCHIPA`
    /// instances form a UUID-linked series (Vol 1 App AR) — so the sequence given
    /// here is the sequence the writer emits. An empty `instances` removes the key.
    /// Returns the previous instance count.
    pub fn set_all(&self, key: &str, instances: Vec<Value>) -> usize {
        let mut data = self.data.write().unwrap();
        let previous = if instances.is_empty() {
            data.remove(key)
        } else {
            data.insert(key.to_string(), instances)
        };
        previous.map_or(0, |instances| instances.len())
    }

    /// Append one more value under `key`, keeping any already stored.
    ///
    /// Returns the new instance count.
    pub fn append(&self, key: &str, instance: Value) -> usize {
        let mut data = self.data.write().unwrap();
        let instances = data.entry(key.to_string()).or_default();
        instances.push(instance);
        instances.len()
    }

    /// Remove a key and every instance under it, returning the first instance.
    pub fn remove(&self, key: &str) -> Option<Value> {
        let mut data = self.data.write().unwrap();
        data.remove(key)
            .and_then(|instances| instances.into_iter().next())
    }

    /// Bulk insert from a map, overwriting any existing keys.
    ///
    /// This is bulk [`Self::set`] and obeys the same whole-slot rule. Returns
    /// `(key, previous instance count)` for every key whose replacement discarded
    /// more than one instance, sorted by key.
    pub fn update(&self, entries: HashMap<String, Value>) -> Vec<(String, usize)> {
        let mut data = self.data.write().unwrap();
        let mut narrowed = Vec::new();

        for (key, value) in entries {
            match data.insert(key.clone(), vec![value]) {
                Some(previous) if previous.len() > 1 => narrowed.push((key, previous.len())),
                _ => {}
            }
        }

        narrowed.sort_unstable();
        narrowed
    }

    /// Clear all stored metadata.
    pub fn clear(&self) {
        let mut data = self.data.write().unwrap();
        data.clear();
    }
}

impl Default for BufferedMetadataProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MetadataProvider for BufferedMetadataProvider {
    fn raw(&self) -> &[u8] {
        &[]
    }

    /// The first instance stored under `key`, matching `get_all(key)[0]`.
    fn get_value(&self, key: &str) -> Option<Value> {
        let data = self.data.read().unwrap();
        data.get(key)
            .and_then(|instances| instances.first().cloned())
    }

    fn contains_key(&self, key: &str) -> bool {
        let data = self.data.read().unwrap();
        data.contains_key(key)
    }

    /// One entry per key — a repeated key counts once.
    fn len(&self) -> usize {
        let data = self.data.read().unwrap();
        data.len()
    }

    /// All keys, sorted.
    ///
    /// Sorted rather than in `HashMap` order because this order is load-bearing on the
    /// write path: the NITF writer classifies these keys and emits extensions in the
    /// order it walks them, so an unsorted answer would make envelope bytes differ
    /// between runs of the same program.
    fn keys(&self) -> Vec<String> {
        let data = self.data.read().unwrap();
        let mut keys: Vec<String> = data.keys().cloned().collect();
        keys.sort_unstable();
        keys
    }

    fn entries(&self, prefix: Option<&str>) -> HashMap<String, Value> {
        let data = self.data.read().unwrap();

        data.iter()
            .filter(|(key, _)| prefix.is_none_or(|p| key.starts_with(p)))
            .filter_map(|(key, instances)| {
                instances.first().map(|first| (key.clone(), first.clone()))
            })
            .collect()
    }

    fn get_all(&self, key: &str) -> Vec<Value> {
        let data = self.data.read().unwrap();
        data.get(key).cloned().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn new_creates_empty_provider() {
        let provider = BufferedMetadataProvider::new();
        assert!(provider.is_empty());
        assert_eq!(provider.len(), 0);
    }

    #[test]
    fn set_and_get_value() {
        let provider = BufferedMetadataProvider::new();
        provider.set("imode", json!("B"));
        assert_eq!(provider.get_value("imode"), Some(json!("B")));
    }

    #[test]
    fn set_overwrites_existing_value() {
        let provider = BufferedMetadataProvider::new();
        provider.set("imode", json!("B"));
        provider.set("imode", json!("P"));
        assert_eq!(provider.get_value("imode"), Some(json!("P")));
    }

    #[test]
    fn get_value_nonexistent_key_returns_none() {
        let provider = BufferedMetadataProvider::new();
        assert_eq!(provider.get_value("nonexistent"), None);
    }

    #[test]
    fn contains_key_reports_correctly() {
        let provider = BufferedMetadataProvider::new();
        provider.set("imode", json!("B"));
        assert!(provider.contains_key("imode"));
        assert!(!provider.contains_key("nonexistent"));
    }

    #[test]
    fn len_tracks_entries() {
        let provider = BufferedMetadataProvider::new();
        assert_eq!(provider.len(), 0);
        provider.set("a", json!("1"));
        assert_eq!(provider.len(), 1);
        provider.set("b", json!("2"));
        assert_eq!(provider.len(), 2);
        provider.set("a", json!("3")); // overwrite, no length change
        assert_eq!(provider.len(), 2);
    }

    #[test]
    fn is_empty_reflects_state() {
        let provider = BufferedMetadataProvider::new();
        assert!(provider.is_empty());
        provider.set("k", json!("v"));
        assert!(!provider.is_empty());
    }

    #[test]
    fn keys_returns_all_keys() {
        let provider = BufferedMetadataProvider::new();
        provider.set("imode", json!("B"));
        provider.set("ic", json!("NC"));
        let mut keys = provider.keys();
        keys.sort();
        assert_eq!(keys, vec!["ic", "imode"]);
    }

    #[test]
    fn remove_returns_previous_value() {
        let provider = BufferedMetadataProvider::new();
        provider.set("imode", json!("B"));
        let removed = provider.remove("imode");
        assert_eq!(removed, Some(json!("B")));
        assert_eq!(provider.get_value("imode"), None);
    }

    #[test]
    fn remove_nonexistent_returns_none() {
        let provider = BufferedMetadataProvider::new();
        assert_eq!(provider.remove("nonexistent"), None);
    }

    #[test]
    fn update_merges_entries() {
        let provider = BufferedMetadataProvider::new();
        provider.set("existing", json!("keep"));

        let mut new_entries = HashMap::new();
        new_entries.insert("a".to_string(), json!("1"));
        new_entries.insert("b".to_string(), json!("2"));
        provider.update(new_entries);

        assert_eq!(provider.len(), 3);
        assert_eq!(provider.get_value("existing"), Some(json!("keep")));
        assert_eq!(provider.get_value("a"), Some(json!("1")));
        assert_eq!(provider.get_value("b"), Some(json!("2")));
    }

    #[test]
    fn update_overwrites_existing_keys() {
        let provider = BufferedMetadataProvider::new();
        provider.set("imode", json!("B"));

        let mut new_entries = HashMap::new();
        new_entries.insert("imode".to_string(), json!("P"));
        provider.update(new_entries);

        assert_eq!(provider.get_value("imode"), Some(json!("P")));
    }

    #[test]
    fn clear_removes_all_keys() {
        let provider = BufferedMetadataProvider::new();
        provider.set("imode", json!("B"));
        provider.set("ic", json!("NC"));
        provider.set("nppbh", json!("256"));

        provider.clear();

        assert!(provider.is_empty());
        assert_eq!(provider.len(), 0);
    }

    #[test]
    fn raw_returns_empty_bytes() {
        let provider = BufferedMetadataProvider::new();
        provider.set("imode", json!("B"));
        assert!(provider.raw().is_empty());
    }

    #[test]
    fn entries_none_returns_all_pairs() {
        let provider = BufferedMetadataProvider::new();
        provider.set("imode", json!("B"));
        provider.set("ic", json!("NC"));
        provider.set("nppbh", json!("256"));

        let dict = provider.entries(None);

        assert_eq!(dict.len(), 3);
        assert_eq!(dict.get("imode"), Some(&json!("B")));
        assert_eq!(dict.get("ic"), Some(&json!("NC")));
        assert_eq!(dict.get("nppbh"), Some(&json!("256")));
    }

    #[test]
    fn entries_with_prefix_filters_correctly() {
        let provider = BufferedMetadataProvider::new();
        provider.set("imode", json!("B"));
        provider.set("ic", json!("NC"));
        provider.set("nppbh", json!("256"));
        provider.set("nppbv", json!("256"));
        provider.set("comrat", json!("01.0"));

        let dict = provider.entries(Some("npp"));

        assert_eq!(dict.len(), 2);
        assert!(dict.contains_key("nppbh"));
        assert!(dict.contains_key("nppbv"));
        assert!(!dict.contains_key("imode"));
    }

    #[test]
    fn entries_with_nonmatching_prefix_returns_empty() {
        let provider = BufferedMetadataProvider::new();
        provider.set("imode", json!("B"));
        provider.set("ic", json!("NC"));

        let dict = provider.entries(Some("xyz"));
        assert!(dict.is_empty());
    }

    #[test]
    fn default_creates_empty_provider() {
        let provider = BufferedMetadataProvider::default();
        assert!(provider.is_empty());
    }

    #[test]
    fn from_provider_copies_all_pairs() {
        let source = BufferedMetadataProvider::new();
        source.set("imode", json!("B"));
        source.set("ic", json!("NC"));
        source.set("nppbh", json!("256"));

        let copied = BufferedMetadataProvider::from_provider(&source);

        assert_eq!(copied.len(), 3);
        assert_eq!(copied.get_value("imode"), Some(json!("B")));
        assert_eq!(copied.get_value("ic"), Some(json!("NC")));
        assert_eq!(copied.get_value("nppbh"), Some(json!("256")));
    }

    #[test]
    fn from_provider_allows_modification_without_affecting_source() {
        let source = BufferedMetadataProvider::new();
        source.set("imode", json!("B"));

        let copied = BufferedMetadataProvider::from_provider(&source);
        copied.set("imode", json!("P"));
        copied.set("new_key", json!("NEW_VALUE"));

        assert_eq!(source.get_value("imode"), Some(json!("B")));
        assert_eq!(source.get_value("new_key"), None);

        assert_eq!(copied.get_value("imode"), Some(json!("P")));
        assert_eq!(copied.get_value("new_key"), Some(json!("NEW_VALUE")));
    }

    #[test]
    fn set_all_stores_instances_in_order() {
        let provider = BufferedMetadataProvider::new();
        provider.set_all(
            "PIAPEA",
            vec![json!({"LASTNME": "DURHAM"}), json!({"LASTNME": "DAILEY"})],
        );

        let instances = provider.get_all("PIAPEA");
        assert_eq!(instances.len(), 2);
        assert_eq!(instances[0], json!({"LASTNME": "DURHAM"}));
        assert_eq!(instances[1], json!({"LASTNME": "DAILEY"}));
    }

    #[test]
    fn append_adds_without_discarding() {
        let provider = BufferedMetadataProvider::new();
        provider.set_all("PIAPEA", vec![json!({"LASTNME": "DURHAM"})]);
        assert_eq!(provider.append("PIAPEA", json!({"LASTNME": "DAILEY"})), 2);
        assert_eq!(provider.append("PIAPEA", json!({"LASTNME": "WEBB"})), 3);

        let instances = provider.get_all("PIAPEA");
        let names: Vec<&str> = instances
            .iter()
            .map(|i| i["LASTNME"].as_str().unwrap())
            .collect();
        assert_eq!(names, vec!["DURHAM", "DAILEY", "WEBB"]);
    }

    #[test]
    fn append_creates_the_slot_when_absent() {
        let provider = BufferedMetadataProvider::new();
        assert_eq!(provider.append("SECURA", json!({"SECLEN": "001"})), 1);
        assert_eq!(provider.get_all("SECURA").len(), 1);
        assert_eq!(provider.get_value("SECURA"), Some(json!({"SECLEN": "001"})));
    }

    #[test]
    fn set_replaces_the_whole_slot_and_reports_the_discarded_count() {
        let provider = BufferedMetadataProvider::new();
        provider.set_all("PIAPEA", vec![json!({"N": "A"}), json!({"N": "B"})]);

        assert_eq!(provider.set("PIAPEA", json!({"N": "C"})), 2);
        assert_eq!(provider.get_all("PIAPEA"), vec![json!({"N": "C"})]);
    }

    #[test]
    fn set_reports_zero_when_the_key_is_new() {
        let provider = BufferedMetadataProvider::new();
        assert_eq!(provider.set("IMODE", json!("B")), 0);
        assert_eq!(provider.set("IMODE", json!("P")), 1);
    }

    #[test]
    fn set_all_with_no_instances_removes_the_key() {
        let provider = BufferedMetadataProvider::new();
        provider.set_all("PIAPEA", vec![json!({"N": "A"})]);
        assert_eq!(provider.set_all("PIAPEA", Vec::new()), 1);

        assert!(!provider.contains_key("PIAPEA"));
        assert!(provider.get_all("PIAPEA").is_empty());
    }

    #[test]
    fn remove_discards_every_instance() {
        let provider = BufferedMetadataProvider::new();
        provider.set_all("PIAPEA", vec![json!({"N": "A"}), json!({"N": "B"})]);

        assert_eq!(provider.remove("PIAPEA"), Some(json!({"N": "A"})));
        assert!(provider.get_all("PIAPEA").is_empty());
        assert!(!provider.contains_key("PIAPEA"));
    }

    #[test]
    fn update_obeys_the_whole_slot_rule_and_names_narrowed_keys() {
        let provider = BufferedMetadataProvider::new();
        provider.set_all("PIAPEA", vec![json!({"N": "A"}), json!({"N": "B"})]);
        provider.set("ICAT", json!("VIS"));

        let mut entries = HashMap::new();
        entries.insert("PIAPEA".to_string(), json!({"N": "C"}));
        entries.insert("ICAT".to_string(), json!("MS"));
        entries.insert("IMODE".to_string(), json!("B"));

        assert_eq!(provider.update(entries), vec![("PIAPEA".to_string(), 2)]);
        assert_eq!(provider.get_all("PIAPEA"), vec![json!({"N": "C"})]);
    }

    #[test]
    fn dictionary_surface_projects_the_first_instance() {
        let provider = BufferedMetadataProvider::new();
        provider.set("ICAT", json!("VIS"));
        provider.set_all(
            "PIAPEA",
            vec![json!({"N": "A"}), json!({"N": "B"}), json!({"N": "C"})],
        );

        // One key per tag regardless of instance count.
        assert_eq!(provider.len(), 2);
        let mut keys = provider.keys();
        keys.sort();
        assert_eq!(keys, vec!["ICAT", "PIAPEA"]);

        // The invariant the whole design rests on.
        assert_eq!(
            provider.get_value("PIAPEA"),
            Some(provider.get_all("PIAPEA")[0].clone())
        );
        assert_eq!(provider.get_value("PIAPEA"), Some(json!({"N": "A"})));
        assert_eq!(
            provider.entries(None).get("PIAPEA"),
            Some(&json!({"N": "A"}))
        );
    }

    #[test]
    fn keys_are_sorted_so_the_write_path_is_reproducible() {
        let provider = BufferedMetadataProvider::new();
        provider.set("RPC00B", json!({"SUCCESS": "1"}));
        provider.set("ICAT", json!("VIS"));
        provider.set_all("PIAPEA", vec![json!({"N": "A"}), json!({"N": "B"})]);

        // The writer emits extensions in the order it walks these keys, so a
        // `HashMap` order would make envelope bytes differ between runs.
        assert_eq!(provider.keys(), vec!["ICAT", "PIAPEA", "RPC00B"]);
    }

    #[test]
    fn keys_and_get_all_enumerate_every_value() {
        let provider = BufferedMetadataProvider::new();
        provider.set("ICAT", json!("VIS"));
        provider.set("IMAGE_INFO", json!([{"LISH": "439"}]));
        provider.set_all("PIAPEA", vec![json!({"N": "A"}), json!({"N": "B"})]);

        // `get_all` is total over `keys()`: one element for an ordinary key — an
        // array value included, wrapped rather than spread — and one per instance
        // for a repeated key. That totality is what makes this a complete walk.
        let walked: Vec<(String, usize)> = provider
            .keys()
            .into_iter()
            .map(|key| {
                let count = provider.get_all(&key).len();
                (key, count)
            })
            .collect();

        assert_eq!(
            walked,
            vec![
                ("ICAT".to_string(), 1),
                ("IMAGE_INFO".to_string(), 1),
                ("PIAPEA".to_string(), 2),
            ]
        );
        assert_eq!(
            provider.get_all("IMAGE_INFO"),
            vec![json!([{"LISH": "439"}])]
        );
    }

    #[test]
    fn from_provider_preserves_repeated_instances() {
        let source = BufferedMetadataProvider::new();
        source.set("ICAT", json!("VIS"));
        source.set_all(
            "PIAPEA",
            vec![json!({"N": "A"}), json!({"N": "B"}), json!({"N": "C"})],
        );

        let copied = BufferedMetadataProvider::from_provider(&source);

        assert_eq!(copied.get_all("PIAPEA").len(), 3);
        assert_eq!(copied.get_all("PIAPEA"), source.get_all("PIAPEA"));
        assert_eq!(copied.get_value("ICAT"), Some(json!("VIS")));
        assert_eq!(copied.len(), 2);
    }

    #[test]
    fn from_provider_copies_no_key_twice() {
        // A TRE key must arrive through `get_all()` only; if it also came through
        // `entries()` the later write would decide the outcome.
        let source = BufferedMetadataProvider::new();
        source.set_all("PIAPEA", vec![json!({"N": "A"}), json!({"N": "B"})]);

        let copied = BufferedMetadataProvider::from_provider(&source);

        assert_eq!(copied.get_all("PIAPEA").len(), 2);
        assert_eq!(copied.get_value("PIAPEA"), Some(json!({"N": "A"})));
    }

    #[test]
    fn set_accepts_all_json_types() {
        let provider = BufferedMetadataProvider::new();
        provider.set("str", json!("hello"));
        provider.set("int", json!(42));
        provider.set("float", json!(2.5));
        provider.set("bool", json!(true));
        provider.set("null", Value::Null);
        provider.set("array", json!([1, 2, 3]));
        provider.set("object", json!({"a": "b"}));

        assert_eq!(provider.get_value("str"), Some(json!("hello")));
        assert_eq!(provider.get_value("int"), Some(json!(42)));
        assert_eq!(provider.get_value("float"), Some(json!(2.5)));
        assert_eq!(provider.get_value("bool"), Some(json!(true)));
        assert_eq!(provider.get_value("null"), Some(Value::Null));
        assert_eq!(provider.get_value("array"), Some(json!([1, 2, 3])));
        assert_eq!(provider.get_value("object"), Some(json!({"a": "b"})));
    }
}
