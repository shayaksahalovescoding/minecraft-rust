use crate::items::{ItemId, ItemRegistry, ItemStack};
use crate::tags::TagRegistry;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CookingKind {
    Smelting,
    Blasting,
    Smoking,
    Campfire,
}

#[derive(Debug, Clone)]
pub enum Ingredient {
    Item(ItemId),
    Choice(Vec<ItemId>),
}

impl Ingredient {
    pub fn matches(&self, item: ItemId) -> bool {
        if item == ItemId::AIR {
            return false;
        }
        match self {
            Self::Item(id) => *id == item,
            Self::Choice(list) => list.contains(&item),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ShapedRecipe {
    pub id: String,
    pub width: usize,
    pub height: usize,
    pub pattern: Vec<Option<Ingredient>>,
    pub result: ItemId,
    pub count: u8,
}

#[derive(Debug, Clone)]
pub struct ShapelessRecipe {
    pub id: String,
    pub ingredients: Vec<Ingredient>,
    pub result: ItemId,
    pub count: u8,
}

#[derive(Debug, Clone)]
pub struct CookingRecipe {
    pub id: String,
    pub kind: CookingKind,
    pub ingredient: Ingredient,
    pub result: ItemId,
    pub experience: f32,
    pub cooking_time: u16,
}

#[derive(Debug, Clone)]
pub struct StonecuttingRecipe {
    pub id: String,
    pub ingredient: Ingredient,
    pub result: ItemId,
    pub count: u8,
}

#[derive(Debug, Clone)]
pub struct SmithingRecipe {
    pub id: String,
    pub template: Option<Ingredient>,
    pub base: Ingredient,
    pub addition: Ingredient,
    pub result: ItemId,
}

#[derive(Debug, Clone)]
pub struct RecipeMatch {
    pub result: ItemStack,
    pub consumed_slots: Vec<usize>,
}

#[derive(Debug)]
pub struct RecipeEngine {
    pub shaped: Vec<ShapedRecipe>,
    pub shapeless: Vec<ShapelessRecipe>,
    pub cooking: Vec<CookingRecipe>,
    pub stonecutting: Vec<StonecuttingRecipe>,
    pub smithing: Vec<SmithingRecipe>,
}

impl RecipeEngine {
    pub fn from_embedded(items: &ItemRegistry, tags: &TagRegistry) -> Self {
        let raw: HashMap<String, Value> =
            serde_json::from_str(include_str!("../../../data/extracted_26_1_2/recipes.json"))
                .expect("valid recipes.json");

        let mut shaped = Vec::new();
        let mut shapeless = Vec::new();
        let mut cooking = Vec::new();
        let mut stonecutting = Vec::new();
        let mut smithing = Vec::new();

        for (id, val) in raw {
            let rtype = val
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .strip_prefix("minecraft:")
                .unwrap_or("");

            match rtype {
                "crafting_shaped" => {
                    if let Some(r) = parse_shaped(&id, &val, items, tags) {
                        shaped.push(r);
                    }
                }
                "crafting_shapeless" => {
                    if let Some(r) = parse_shapeless(&id, &val, items, tags) {
                        shapeless.push(r);
                    }
                }
                "smelting" | "blasting" | "smoking" | "campfire_cooking" => {
                    let kind = match rtype {
                        "smelting" => CookingKind::Smelting,
                        "blasting" => CookingKind::Blasting,
                        "smoking" => CookingKind::Smoking,
                        _ => CookingKind::Campfire,
                    };
                    if let Some(r) = parse_cooking(&id, kind, &val, items, tags) {
                        cooking.push(r);
                    }
                }
                "stonecutting" => {
                    if let Some(r) = parse_stonecutting(&id, &val, items, tags) {
                        stonecutting.push(r);
                    }
                }
                "smithing_transform" => {
                    if let Some(r) = parse_smithing(&id, &val, items, tags) {
                        smithing.push(r);
                    }
                }
                _ => {}
            }
        }

        // Also parse cooking ingredients with tags cleanly
        Self {
            shaped,
            shapeless,
            cooking,
            stonecutting,
            smithing,
        }
    }

    pub fn match_crafting(&self, grid_w: usize, grid_h: usize, grid: &[ItemStack]) -> Option<RecipeMatch> {
        // Compute tight bounding box of non-empty items
        let mut min_x = grid_w;
        let mut min_y = grid_h;
        let mut max_x = 0usize;
        let mut max_y = 0usize;
        let mut non_empty = Vec::new();

        for y in 0..grid_h {
            for x in 0..grid_w {
                let idx = y * grid_w + x;
                if !grid[idx].is_empty() {
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                    non_empty.push(idx);
                }
            }
        }
        if non_empty.is_empty() {
            return None;
        }
        let box_w = max_x - min_x + 1;
        let box_h = max_y - min_y + 1;

        for r in &self.shaped {
            if r.width != box_w || r.height != box_h {
                continue;
            }
            for mirror in [false, true] {
                let mut ok = true;
                for ry in 0..r.height {
                    for rx in 0..r.width {
                        let gx = if mirror { min_x + (r.width - 1 - rx) } else { min_x + rx };
                        let gy = min_y + ry;
                        let item = grid[gy * grid_w + gx].item;
                        match &r.pattern[ry * r.width + rx] {
                            Some(ing) => {
                                if !ing.matches(item) {
                                    ok = false;
                                    break;
                                }
                            }
                            None => {
                                if item != ItemId::AIR {
                                    ok = false;
                                    break;
                                }
                            }
                        }
                    }
                    if !ok {
                        break;
                    }
                }
                if ok {
                    return Some(RecipeMatch {
                        result: ItemStack::new(r.result, r.count),
                        consumed_slots: non_empty,
                    });
                }
            }
        }

        for r in &self.shapeless {
            if r.ingredients.len() != non_empty.len() {
                continue;
            }
            let mut used = vec![false; non_empty.len()];
            if match_shapeless_backtrack(&r.ingredients, 0, &non_empty, grid, &mut used) {
                return Some(RecipeMatch {
                    result: ItemStack::new(r.result, r.count),
                    consumed_slots: non_empty,
                });
            }
        }

        None
    }

    pub fn match_cooking(&self, kind: CookingKind, input: ItemId) -> Option<(ItemId, f32, u16)> {
        if input == ItemId::AIR {
            return None;
        }
        self.cooking
            .iter()
            .find(|r| r.kind == kind && r.ingredient.matches(input))
            .map(|r| (r.result, r.experience, r.cooking_time))
    }

    pub fn stonecutter_results(&self, input: ItemId) -> Vec<(ItemId, u8)> {
        if input == ItemId::AIR {
            return Vec::new();
        }
        self.stonecutting
            .iter()
            .filter(|r| r.ingredient.matches(input))
            .map(|r| (r.result, r.count))
            .collect()
    }

    pub fn match_smithing(&self, template: ItemId, base: ItemId, addition: ItemId) -> Option<ItemId> {
        self.smithing
            .iter()
            .find(|r| {
                r.template.as_ref().is_none_or(|t| t.matches(template))
                    && r.base.matches(base)
                    && r.addition.matches(addition)
            })
            .map(|r| r.result)
    }
}

fn match_shapeless_backtrack(
    ings: &[Ingredient],
    idx: usize,
    slots: &[usize],
    grid: &[ItemStack],
    used: &mut [bool],
) -> bool {
    if idx == ings.len() {
        return true;
    }
    for (i, &slot) in slots.iter().enumerate() {
        if !used[i] && ings[idx].matches(grid[slot].item) {
            used[i] = true;
            if match_shapeless_backtrack(ings, idx + 1, slots, grid, used) {
                return true;
            }
            used[i] = false;
        }
    }
    false
}

fn parse_ingredient(val: &Value, items: &ItemRegistry, tags: &TagRegistry) -> Option<Ingredient> {
    if let Some(s) = val.as_str() {
        if let Some(tag) = s.strip_prefix('#') {
            let members: Vec<ItemId> = tags
                .item_tag_members(tag)
                .iter()
                .filter_map(|n| items.by_name(n))
                .collect();
            return (!members.is_empty()).then_some(Ingredient::Choice(members));
        } else {
            return items.by_name(s).map(Ingredient::Item);
        }
    }
    if let Some(arr) = val.as_array() {
        let mut list = Vec::new();
        for e in arr {
            if let Some(ing) = parse_ingredient(e, items, tags) {
                match ing {
                    Ingredient::Item(id) => list.push(id),
                    Ingredient::Choice(v) => list.extend(v),
                }
            }
        }
        return (!list.is_empty()).then_some(Ingredient::Choice(list));
    }
    if let Some(obj) = val.as_object() {
        if let Some(item) = obj.get("item").and_then(|v| v.as_str()) {
            return items.by_name(item).map(Ingredient::Item);
        }
        if let Some(tag) = obj.get("tag").and_then(|v| v.as_str()) {
            let members: Vec<ItemId> = tags
                .item_tag_members(tag)
                .iter()
                .filter_map(|n| items.by_name(n))
                .collect();
            return (!members.is_empty()).then_some(Ingredient::Choice(members));
        }
    }
    None
}

fn parse_result(val: &Value, items: &ItemRegistry) -> Option<(ItemId, u8)> {
    let res = val.get("result")?;
    let id_str = res.get("id").and_then(|v| v.as_str()).or_else(|| res.as_str())?;
    let item = items.by_name(id_str)?;
    let count = res.get("count").and_then(|v| v.as_u64()).unwrap_or(1) as u8;
    Some((item, count.max(1)))
}

fn parse_shaped(id: &str, val: &Value, items: &ItemRegistry, tags: &TagRegistry) -> Option<ShapedRecipe> {
    let (result, count) = parse_result(val, items)?;
    let pat_arr = val.get("pattern")?.as_array()?;
    let height = pat_arr.len();
    let width = pat_arr.iter().filter_map(|r| r.as_str()).map(|s| s.len()).max().unwrap_or(0);
    if width == 0 || height == 0 {
        return None;
    }
    let key_obj = val.get("key")?.as_object()?;
    let mut key_map = HashMap::new();
    for (ch_str, ing_val) in key_obj {
        if let Some(ch) = ch_str.chars().next() {
            key_map.insert(ch, parse_ingredient(ing_val, items, tags)?);
        }
    }
    let mut pattern = Vec::with_capacity(width * height);
    for row_v in pat_arr {
        let row = row_v.as_str().unwrap_or("");
        for x in 0..width {
            let ch = row.chars().nth(x).unwrap_or(' ');
            if ch == ' ' {
                pattern.push(None);
            } else {
                pattern.push(key_map.get(&ch).cloned());
            }
        }
    }
    Some(ShapedRecipe {
        id: id.to_string(),
        width,
        height,
        pattern,
        result,
        count,
    })
}

fn parse_shapeless(id: &str, val: &Value, items: &ItemRegistry, tags: &TagRegistry) -> Option<ShapelessRecipe> {
    let (result, count) = parse_result(val, items)?;
    let ing_arr = val.get("ingredients")?.as_array()?;
    let mut ingredients = Vec::with_capacity(ing_arr.len());
    for v in ing_arr {
        ingredients.push(parse_ingredient(v, items, tags)?);
    }
    Some(ShapelessRecipe {
        id: id.to_string(),
        ingredients,
        result,
        count,
    })
}

fn parse_cooking(id: &str, kind: CookingKind, val: &Value, items: &ItemRegistry, tags: &TagRegistry) -> Option<CookingRecipe> {
    let (result, _) = parse_result(val, items)?;
    let ing_val = val.get("ingredient")?;
    let ingredient = parse_ingredient(ing_val, items, tags)?;
    let experience = val.get("experience").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
    let cooking_time = val.get("cookingtime").and_then(|v| v.as_u64()).unwrap_or(200) as u16;
    Some(CookingRecipe {
        id: id.to_string(),
        kind,
        ingredient,
        result,
        experience,
        cooking_time,
    })
}

fn parse_stonecutting(id: &str, val: &Value, items: &ItemRegistry, tags: &TagRegistry) -> Option<StonecuttingRecipe> {
    let (result, count) = parse_result(val, items)?;
    let ingredient = parse_ingredient(val.get("ingredient")?, items, tags)?;
    Some(StonecuttingRecipe {
        id: id.to_string(),
        ingredient,
        result,
        count,
    })
}

fn parse_smithing(id: &str, val: &Value, items: &ItemRegistry, tags: &TagRegistry) -> Option<SmithingRecipe> {
    let (result, _) = parse_result(val, items)?;
    let template = val.get("template").and_then(|v| parse_ingredient(v, items, tags));
    let base = parse_ingredient(val.get("base")?, items, tags)?;
    let addition = parse_ingredient(val.get("addition")?, items, tags)?;
    Some(SmithingRecipe {
        id: id.to_string(),
        template,
        base,
        addition,
        result,
    })
}
