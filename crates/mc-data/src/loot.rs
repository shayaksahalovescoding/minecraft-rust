use crate::items::{ItemId, ItemRegistry, ItemStack};
use crate::tags::TagRegistry;
use rand::Rng;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct LootContext {
    pub silk_touch: bool,
    pub fortune: u8,
    pub explosion_survived: bool,
}

impl Default for LootContext {
    fn default() -> Self {
        Self {
            silk_touch: false,
            fortune: 0,
            explosion_survived: true,
        }
    }
}

#[derive(Debug)]
pub struct LootEngine {
    tables: HashMap<String, Value>,
    items_by_name: HashMap<String, ItemId>,
    item_tags: HashMap<String, Vec<ItemId>>,
}

impl LootEngine {
    pub fn from_embedded(items: &ItemRegistry, tags: &TagRegistry) -> Self {
        let tables: HashMap<String, Value> = serde_json::from_str(include_str!(
            "../../../data/extracted_26_1_2/loot_tables.json"
        ))
        .expect("valid loot_tables.json");

        let mut items_by_name = HashMap::with_capacity(items.len());
        for def in items.all() {
            items_by_name.insert(def.name.clone(), def.id);
        }

        let _ = tags;
        Self {
            tables,
            items_by_name,
            item_tags: HashMap::new(),
        }
    }

    pub fn table_count(&self) -> usize {
        self.tables.len()
    }

    pub fn roll_block(&self, block_name: &str, ctx: &LootContext, rng: &mut impl Rng) -> Vec<ItemStack> {
        let key = format!("blocks/{}", block_name.strip_prefix("minecraft:").unwrap_or(block_name));
        self.roll_table(&key, ctx, rng)
    }

    pub fn roll_entity(&self, entity_name: &str, ctx: &LootContext, rng: &mut impl Rng) -> Vec<ItemStack> {
        let key = format!("entities/{}", entity_name.strip_prefix("minecraft:").unwrap_or(entity_name));
        self.roll_table(&key, ctx, rng)
    }

    pub fn roll_chest(&self, chest_name: &str, ctx: &LootContext, rng: &mut impl Rng) -> Vec<ItemStack> {
        let key = if chest_name.starts_with("chests/") {
            chest_name.to_string()
        } else {
            format!("chests/{chest_name}")
        };
        self.roll_table(&key, ctx, rng)
    }

    pub fn roll_table(&self, key: &str, ctx: &LootContext, rng: &mut impl Rng) -> Vec<ItemStack> {
        let Some(table) = self.tables.get(key) else {
            return Vec::new();
        };
        let mut drops = Vec::new();
        let Some(pools) = table.get("pools").and_then(|v| v.as_array()) else {
            return drops;
        };
        for pool in pools {
            if !check_conditions(pool.get("conditions"), ctx, rng) {
                continue;
            }
            let rolls = eval_number(pool.get("rolls"), rng).max(0.0).round() as usize;
            let Some(entries) = pool.get("entries").and_then(|v| v.as_array()) else {
                continue;
            };
            for _ in 0..rolls {
                for entry in entries {
                    if self.eval_entry(entry, ctx, rng, &mut drops) {
                        break;
                    }
                }
            }
        }
        drops
    }

    fn eval_entry(&self, entry: &Value, ctx: &LootContext, rng: &mut impl Rng, out: &mut Vec<ItemStack>) -> bool {
        if !check_conditions(entry.get("conditions"), ctx, rng) {
            return false;
        }
        let etype = entry
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .strip_prefix("minecraft:")
            .unwrap_or("");

        match etype {
            "alternatives" => {
                if let Some(children) = entry.get("children").and_then(|v| v.as_array()) {
                    for child in children {
                        if self.eval_entry(child, ctx, rng, out) {
                            return true;
                        }
                    }
                }
                false
            }
            "group" | "sequence" => {
                let mut any = false;
                if let Some(children) = entry.get("children").and_then(|v| v.as_array()) {
                    for child in children {
                        if self.eval_entry(child, ctx, rng, out) {
                            any = true;
                        }
                    }
                }
                any
            }
            "item" => {
                let Some(name) = entry.get("name").and_then(|v| v.as_str()) else {
                    return false;
                };
                let short = name.strip_prefix("minecraft:").unwrap_or(name);
                let Some(&item_id) = self.items_by_name.get(short) else {
                    return false;
                };
                let mut count: i32 = 1;
                if let Some(funcs) = entry.get("functions").and_then(|v| v.as_array()) {
                    for f in funcs {
                        let ftype = f
                            .get("function")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .strip_prefix("minecraft:")
                            .unwrap_or("");
                        if ftype == "set_count" {
                            count = eval_number(f.get("count"), rng).round() as i32;
                        } else if ftype == "apply_bonus" && ctx.fortune > 0 {
                            let bonus = rng.gen_range(0..=(ctx.fortune as i32));
                            count += bonus;
                        }
                    }
                }
                if count > 0 {
                    out.push(ItemStack::new(item_id, count.min(64) as u8));
                }
                true
            }
            "loot_table" => {
                if let Some(val) = entry.get("value").and_then(|v| v.as_str()) {
                    let k = val.strip_prefix("minecraft:").unwrap_or(val);
                    out.extend(self.roll_table(k, ctx, rng));
                    return true;
                }
                false
            }
            _ => {
                let _ = &self.item_tags;
                false
            }
        }
    }
}

fn check_conditions(conds: Option<&Value>, ctx: &LootContext, rng: &mut impl Rng) -> bool {
    let Some(arr) = conds.and_then(|v| v.as_array()) else {
        return true;
    };
    for c in arr {
        let ctype = c
            .get("condition")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .strip_prefix("minecraft:")
            .unwrap_or("");
        match ctype {
            "survives_explosion" => {
                if !ctx.explosion_survived {
                    return false;
                }
            }
            "random_chance" => {
                let ch = c.get("chance").and_then(|v| v.as_f64()).unwrap_or(1.0);
                if rng.r#gen::<f64>() >= ch {
                    return false;
                }
            }
            "match_tool" => {
                if let Some(pred) = c.get("predicate") {
                    let s = pred.to_string();
                    if s.contains("silk_touch") && !ctx.silk_touch {
                        return false;
                    }
                }
            }
            "inverted" => {
                if let Some(term) = c.get("term") {
                    let single = Value::Array(vec![term.clone()]);
                    if check_conditions(Some(&single), ctx, rng) {
                        return false;
                    }
                }
            }
            _ => {}
        }
    }
    true
}

fn eval_number(val: Option<&Value>, rng: &mut impl Rng) -> f64 {
    let Some(v) = val else {
        return 1.0;
    };
    if let Some(n) = v.as_f64() {
        return n;
    }
    if let Some(obj) = v.as_object() {
        let min = obj.get("min").and_then(|x| x.as_f64()).unwrap_or(1.0);
        let max = obj.get("max").and_then(|x| x.as_f64()).unwrap_or(min);
        if max <= min {
            return min;
        }
        return rng.gen_range(min..=max);
    }
    1.0
}
