use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default)]
pub struct TagRegistry {
    block_tags: HashMap<String, Vec<String>>,
    item_tags: HashMap<String, Vec<String>>,
    biome_tags: HashMap<String, Vec<String>>,
    entity_tags: HashMap<String, Vec<String>>,
}

impl TagRegistry {
    pub fn from_embedded() -> Self {
        let raw: HashMap<String, Value> =
            serde_json::from_str(include_str!("../../../data/extracted_26_1_2/tags.json"))
                .expect("valid tags.json");

        let mut raw_block = HashMap::new();
        let mut raw_item = HashMap::new();
        let mut raw_biome = HashMap::new();
        let mut raw_entity = HashMap::new();

        for (k, v) in raw {
            let Some(arr) = v.get("values").and_then(|x| x.as_array()) else {
                continue;
            };
            let vals: Vec<String> = arr
                .iter()
                .filter_map(|e| {
                    if let Some(s) = e.as_str() {
                        Some(s.to_string())
                    } else {
                        e.get("id").and_then(|id| id.as_str()).map(|s| s.to_string())
                    }
                })
                .collect();

            if let Some(rest) = k.strip_prefix("block/") {
                raw_block.insert(rest.to_string(), vals);
            } else if let Some(rest) = k.strip_prefix("item/") {
                raw_item.insert(rest.to_string(), vals);
            } else if let Some(rest) = k.strip_prefix("worldgen/biome/") {
                raw_biome.insert(rest.to_string(), vals);
            } else if let Some(rest) = k.strip_prefix("entity_type/") {
                raw_entity.insert(rest.to_string(), vals);
            }
        }

        Self {
            block_tags: resolve_all(&raw_block),
            item_tags: resolve_all(&raw_item),
            biome_tags: resolve_all(&raw_biome),
            entity_tags: resolve_all(&raw_entity),
        }
    }

    pub fn block_has_tag(&self, block_short: &str, tag: &str) -> bool {
        let key = strip_mc(tag);
        self.block_tags
            .get(key)
            .is_some_and(|list| list.iter().any(|x| x == block_short))
    }

    pub fn block_tag_members(&self, tag: &str) -> &[String] {
        let key = strip_mc(tag.trim_start_matches('#'));
        self.block_tags.get(key).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn item_tag_members(&self, tag: &str) -> &[String] {
        let key = strip_mc(tag.trim_start_matches('#'));
        self.item_tags.get(key).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn biome_tag_members(&self, tag: &str) -> &[String] {
        let key = strip_mc(tag.trim_start_matches('#'));
        self.biome_tags.get(key).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn entity_tag_members(&self, tag: &str) -> &[String] {
        let key = strip_mc(tag.trim_start_matches('#'));
        self.entity_tags.get(key).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

fn strip_mc(s: &str) -> &str {
    s.strip_prefix("minecraft:").unwrap_or(s)
}

fn resolve_all(raw: &HashMap<String, Vec<String>>) -> HashMap<String, Vec<String>> {
    let mut out = HashMap::new();
    for key in raw.keys() {
        let mut visited = HashSet::new();
        let mut acc = Vec::new();
        resolve_recursive(key, raw, &mut visited, &mut acc);
        out.insert(key.clone(), acc);
    }
    out
}

fn resolve_recursive(
    key: &str,
    raw: &HashMap<String, Vec<String>>,
    visited: &mut HashSet<String>,
    acc: &mut Vec<String>,
) {
    if !visited.insert(key.to_string()) {
        return;
    }
    let Some(entries) = raw.get(key) else {
        return;
    };
    for e in entries {
        if let Some(sub) = e.strip_prefix('#') {
            resolve_recursive(strip_mc(sub), raw, visited, acc);
        } else {
            let item = strip_mc(e).to_string();
            if !acc.contains(&item) {
                acc.push(item);
            }
        }
    }
}
