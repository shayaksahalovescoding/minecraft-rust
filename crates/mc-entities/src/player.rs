use glam::DVec3;
use mc_core::{
    BlockPos, Difficulty, DimensionId, GameMode, SharedConstants, AABB,
};
use mc_data::{ArmorSlot, BlockState, GameData, ItemId, ItemStack, ToolKind};
use mc_world::World;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusEffect {
    pub id: String,
    pub amplifier: u8,
    pub duration_ticks: u32,
}

#[derive(Debug, Clone, Default)]
pub struct PlayerInput {
    pub forward: f32,
    pub strafe: f32,
    pub jump: bool,
    pub sneak: bool,
    pub sprint: bool,
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub name: String,
    pub pos: DVec3,
    pub vel: DVec3,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub is_in_water: bool,
    pub is_in_lava: bool,
    pub is_sprinting: bool,
    pub is_sneaking: bool,
    pub is_flying: bool,
    pub is_fall_flying: bool,
    pub game_mode: GameMode,
    pub dimension: DimensionId,
    pub health: f32,
    pub max_health: f32,
    pub absorption: f32,
    pub food_level: u8,
    pub saturation_level: f32,
    pub exhaustion_level: f32,
    pub food_tick_timer: u32,
    pub air_supply: i32,
    pub max_air: i32,
    pub fall_distance: f32,
    pub fire_ticks: i32,
    pub invulnerable_ticks: i32,
    pub attack_cooldown_ticks: u32,
    pub xp_level: u32,
    pub xp_progress: f32,
    pub total_xp: u32,
    pub selected_slot: usize,
    pub inventory: Vec<ItemStack>,
    pub armor: [ItemStack; 4], // [boots, leggings, chestplate, helmet]
    pub offhand: ItemStack,
    pub ender_chest: Vec<ItemStack>,
    pub effects: Vec<StatusEffect>,
    pub advancements: HashSet<String>,
    pub spawn_pos: BlockPos,
    pub spawn_dim: DimensionId,
    pub portal_cooldown: u32,
    pub dead: bool,
}

impl Player {
    pub fn new(name: impl Into<String>, spawn: BlockPos, game_mode: GameMode) -> Self {
        Self {
            name: name.into(),
            pos: DVec3::new(spawn.x as f64 + 0.5, spawn.y as f64 + 0.1, spawn.z as f64 + 0.5),
            vel: DVec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: false,
            is_in_water: false,
            is_in_lava: false,
            is_sprinting: false,
            is_sneaking: false,
            is_flying: false,
            is_fall_flying: false,
            game_mode,
            dimension: DimensionId::Overworld,
            health: 20.0,
            max_health: 20.0,
            absorption: 0.0,
            food_level: 20,
            saturation_level: 5.0,
            exhaustion_level: 0.0,
            food_tick_timer: 0,
            air_supply: 300,
            max_air: 300,
            fall_distance: 0.0,
            fire_ticks: 0,
            invulnerable_ticks: 0,
            attack_cooldown_ticks: 0,
            xp_level: 0,
            xp_progress: 0.0,
            total_xp: 0,
            selected_slot: 0,
            inventory: vec![ItemStack::EMPTY; 36],
            armor: [
                ItemStack::EMPTY,
                ItemStack::EMPTY,
                ItemStack::EMPTY,
                ItemStack::EMPTY,
            ],
            offhand: ItemStack::EMPTY,
            ender_chest: vec![ItemStack::EMPTY; 27],
            effects: Vec::new(),
            advancements: HashSet::new(),
            spawn_pos: spawn,
            spawn_dim: DimensionId::Overworld,
            portal_cooldown: 0,
            dead: false,
        }
    }

    pub fn eye_height(&self) -> f64 {
        if self.is_sneaking {
            SharedConstants::PLAYER_CROUCH_EYE_HEIGHT
        } else {
            SharedConstants::PLAYER_EYE_HEIGHT
        }
    }

    pub fn eye_pos(&self) -> DVec3 {
        self.pos + DVec3::new(0.0, self.eye_height(), 0.0)
    }

    pub fn look_dir(&self) -> DVec3 {
        let cy = (self.yaw as f64).cos();
        let sy = (self.yaw as f64).sin();
        let cp = (self.pitch as f64).cos();
        let sp = (self.pitch as f64).sin();
        DVec3::new(-sy * cp, -sp, cy * cp).normalize()
    }

    pub fn aabb(&self) -> AABB {
        let h = if self.is_sneaking {
            SharedConstants::PLAYER_CROUCH_HEIGHT
        } else {
            SharedConstants::PLAYER_HEIGHT
        };
        AABB::from_feet(self.pos, SharedConstants::PLAYER_WIDTH, h)
    }

    pub fn main_hand(&self) -> &ItemStack {
        &self.inventory[self.selected_slot.min(8)]
    }

    pub fn main_hand_mut(&mut self) -> &mut ItemStack {
        let idx = self.selected_slot.min(8);
        &mut self.inventory[idx]
    }

    pub fn effect_level(&self, id: &str) -> u8 {
        self.effects
            .iter()
            .find(|e| e.id == id)
            .map(|e| e.amplifier + 1)
            .unwrap_or(0)
    }

    pub fn add_effect(&mut self, id: impl Into<String>, amplifier: u8, duration_ticks: u32) {
        let id = id.into();
        if let Some(ex) = self.effects.iter_mut().find(|e| e.id == id) {
            if amplifier >= ex.amplifier {
                ex.amplifier = amplifier;
                ex.duration_ticks = ex.duration_ticks.max(duration_ticks);
            }
        } else {
            self.effects.push(StatusEffect {
                id,
                amplifier,
                duration_ticks,
            });
        }
    }

    pub fn total_armor_defense(&self, data: &GameData) -> (f32, f32, f32) {
        let mut def = 0.0;
        let mut tough = 0.0;
        let mut epf = 0.0;
        for piece in &self.armor {
            if !piece.is_empty() {
                let d = data.items.get(piece.item);
                def += d.armor_defense;
                tough += d.armor_toughness;
                epf += piece.enchantment_level("protection") as f32;
            }
        }
        (def, tough, epf)
    }

    pub fn give_item(&mut self, mut stack: ItemStack, data: &GameData) -> bool {
        if stack.is_empty() {
            return true;
        }
        let max_stack = data.items.get(stack.item).max_stack;
        if max_stack > 1 && stack.enchantments.is_empty() && stack.damage == 0 {
            for slot in self.inventory.iter_mut() {
                if slot.item == stack.item && slot.count < max_stack && slot.enchantments.is_empty()
                {
                    let add = (max_stack - slot.count).min(stack.count);
                    slot.count += add;
                    stack.count -= add;
                    if stack.count == 0 {
                        return true;
                    }
                }
            }
        }
        for slot in self.inventory.iter_mut() {
            if slot.is_empty() {
                *slot = stack;
                return true;
            }
        }
        false
    }

    pub fn add_xp(&mut self, amount: u32) {
        self.total_xp += amount;
        let mut rem = amount as f32;
        while rem > 0.0 {
            let needed = self.xp_to_next_level() as f32;
            let space = needed * (1.0 - self.xp_progress);
            if rem >= space {
                rem -= space;
                self.xp_level += 1;
                self.xp_progress = 0.0;
            } else {
                self.xp_progress += rem / needed;
                rem = 0.0;
            }
        }
    }

    pub fn xp_to_next_level(&self) -> u32 {
        if self.xp_level >= 30 {
            112 + (self.xp_level - 30) * 9
        } else if self.xp_level >= 15 {
            37 + (self.xp_level - 15) * 5
        } else {
            7 + self.xp_level * 2
        }
    }

    pub fn eat_food(&mut self, data: &GameData) -> bool {
        let item_id = self.main_hand().item;
        if item_id == ItemId::AIR {
            return false;
        }
        let def = data.items.get(item_id);
        if def.food_nutrition == 0 || self.food_level >= 20 {
            return false;
        }
        self.food_level = (self.food_level + def.food_nutrition).min(20);
        self.saturation_level = (self.saturation_level + def.food_saturation).min(self.food_level as f32);
        let name = def.name.clone();
        if self.game_mode.is_survival_or_adventure() {
            let slot = self.main_hand_mut();
            slot.count -= 1;
            if slot.count == 0 {
                *slot = ItemStack::EMPTY;
            }
        }
        if name == "golden_apple" {
            self.add_effect("regeneration", 1, 100);
            self.add_effect("absorption", 0, 2400);
        } else if name == "enchanted_golden_apple" {
            self.add_effect("regeneration", 1, 400);
            self.add_effect("resistance", 0, 6000);
            self.add_effect("fire_resistance", 0, 6000);
            self.add_effect("absorption", 3, 2400);
        }
        true
    }

    pub fn try_equip_armor_from_hand(&mut self, data: &GameData) -> bool {
        let held = self.main_hand().clone();
        if held.is_empty() {
            return false;
        }
        let Some(slot) = data.items.get(held.item).armor_slot else {
            return false;
        };
        let idx = match slot {
            ArmorSlot::Boots => 0,
            ArmorSlot::Leggings => 1,
            ArmorSlot::Chestplate => 2,
            ArmorSlot::Helmet => 3,
        };
        let prev = std::mem::replace(&mut self.armor[idx], held);
        *self.main_hand_mut() = prev;
        true
    }

    /// Block breaking speed matching `Player.getDestroySpeed`.
    pub fn block_break_ticks(&self, state: BlockState, data: &GameData) -> Option<u32> {
        let bdef = data.blocks.get(state.id());
        if bdef.hardness < 0.0 {
            return None; // Unbreakable (bedrock, end portal frame)
        }
        if self.game_mode.insta_build() {
            return Some(1);
        }
        if bdef.hardness == 0.0 {
            return Some(1);
        }

        let held = self.main_hand();
        let idef = data.items.get(held.item);
        let correct_tool = idef.tool_kind == bdef.preferred_tool
            && bdef.preferred_tool != ToolKind::None
            && idef.tool_tier >= bdef.min_tool_tier;

        let mut speed = if idef.tool_kind == bdef.preferred_tool && bdef.preferred_tool != ToolKind::None {
            idef.mining_speed
        } else {
            1.0
        };

        if speed > 1.0 {
            let eff = held.enchantment_level("efficiency");
            if eff > 0 {
                speed += (eff as f32).powi(2) + 1.0;
            }
        }
        let haste = self.effect_level("haste");
        if haste > 0 {
            speed *= 1.0 + 0.2 * haste as f32;
        }
        let fatigue = self.effect_level("mining_fatigue");
        if fatigue > 0 {
            speed *= 0.3f32.powi(fatigue as i32);
        }
        if self.is_in_water {
            speed *= 0.2;
        }
        if !self.on_ground && !self.is_flying {
            speed *= 0.2;
        }

        let can_harvest = !bdef.requires_tool || correct_tool;
        let mult = if can_harvest { 30.0 } else { 100.0 };
        let dmg_per_tick = speed / bdef.hardness / mult;
        Some((1.0 / dmg_per_tick.max(0.0001)).ceil() as u32)
    }

    pub fn damage(&mut self, amount: f32, bypass_armor: bool, data: &GameData) -> bool {
        if self.dead || !self.game_mode.is_survival_or_adventure() || self.invulnerable_ticks > 0 {
            return false;
        }
        let mut dmg = amount;
        if !bypass_armor {
            let (def, tough, epf) = self.total_armor_defense(data);
            dmg = CombatRules::get_damage_after_absorb(dmg, def, tough);
            dmg = CombatRules::get_damage_after_magic_absorb(dmg, epf);
        }
        let res = self.effect_level("resistance");
        if res > 0 {
            dmg *= (1.0 - 0.2 * res as f32).max(0.0);
        }
        if self.absorption > 0.0 {
            let absorb = self.absorption.min(dmg);
            self.absorption -= absorb;
            dmg -= absorb;
        }
        self.health = (self.health - dmg).max(0.0);
        self.invulnerable_ticks = SharedConstants::INVULNERABLE_DURATION;
        if self.health <= 0.0 {
            self.dead = true;
        }
        true
    }

    pub fn respawn(&mut self) {
        self.dead = false;
        self.health = self.max_health;
        self.food_level = 20;
        self.saturation_level = 5.0;
        self.exhaustion_level = 0.0;
        self.air_supply = self.max_air;
        self.fire_ticks = 0;
        self.fall_distance = 0.0;
        self.vel = DVec3::ZERO;
        self.dimension = self.spawn_dim;
        self.pos = DVec3::new(
            self.spawn_pos.x as f64 + 0.5,
            self.spawn_pos.y as f64 + 0.1,
            self.spawn_pos.z as f64 + 0.5,
        );
    }

    /// Tick movement, collision, fluid physics, hunger (`FoodData`), oxygen, fire, and status effects.
    pub fn tick(&mut self, input: &PlayerInput, world: &World, data: &GameData) {
        if self.dead {
            return;
        }
        self.yaw = input.yaw;
        self.pitch = input.pitch;

        if self.invulnerable_ticks > 0 {
            self.invulnerable_ticks -= 1;
        }
        if self.attack_cooldown_ticks > 0 {
            self.attack_cooldown_ticks -= 1;
        }
        if self.portal_cooldown > 0 {
            self.portal_cooldown -= 1;
        }

        // Tick active status effects
        for eff in &mut self.effects {
            eff.duration_ticks = eff.duration_ticks.saturating_sub(1);
            if eff.id == "regeneration" && eff.duration_ticks % 25 == 0 {
                self.health = (self.health + 1.0).min(self.max_health);
            } else if eff.id == "poison" && eff.duration_ticks % 25 == 0 && self.health > 1.0 {
                self.health = (self.health - 1.0).max(1.0);
            }
        }
        self.effects.retain(|e| e.duration_ticks > 0);

        let aabb = self.aabb();
        self.is_in_water = world.is_in_fluid(aabb, data.blocks.water);
        self.is_in_lava = world.is_in_fluid(aabb, data.blocks.lava);

        self.is_sneaking = input.sneak && !self.is_flying;
        self.is_sprinting = input.sprint && input.forward > 0.0 && !self.is_sneaking && self.food_level > 6;

        let forward_vec = DVec3::new(-(self.yaw as f64).sin(), 0.0, (self.yaw as f64).cos());
        let right_vec = DVec3::new(-(self.yaw as f64).cos(), 0.0, -(self.yaw as f64).sin());
        let wish_dir = (forward_vec * input.forward as f64 + right_vec * input.strafe as f64)
            .normalize_or_zero();

        if self.is_flying && self.game_mode.can_fly() {
            let fly_spd = SharedConstants::PLAYER_FLY_SPEED * if self.is_sprinting { 2.0 } else { 1.0 };
            self.vel.x = self.vel.x * 0.91 + wish_dir.x * fly_spd;
            self.vel.z = self.vel.z * 0.91 + wish_dir.z * fly_spd;
            let vy_wish = if input.jump { 1.0 } else if input.sneak { -1.0 } else { 0.0 };
            self.vel.y = self.vel.y * 0.6 + vy_wish * fly_spd * 3.0;
            self.fall_distance = 0.0;
        } else if self.is_fall_flying {
            let look = self.look_dir();
            let horiz_speed = (self.vel.x * self.vel.x + self.vel.z * self.vel.z).sqrt();
            let look_horiz = (look.x * look.x + look.z * look.z).sqrt();
            let cos_p = look_horiz;
            self.vel.y += SharedConstants::GRAVITY * (-1.0 + cos_p * cos_p * 0.75);
            if self.vel.y < 0.0 && look_horiz > 0.0 {
                let d = self.vel.y * -0.1 * cos_p * cos_p;
                self.vel.y += d;
                self.vel.x += look.x * d / look_horiz;
                self.vel.z += look.z * d / look_horiz;
            }
            if self.pitch < 0.0 && look_horiz > 0.0 {
                let d = horiz_speed * (-(self.pitch as f64).sin()) * 0.04;
                self.vel.y += d * 3.2;
                self.vel.x -= look.x * d / look_horiz;
                self.vel.z -= look.z * d / look_horiz;
            }
            self.vel.x *= 0.99;
            self.vel.y *= 0.98;
            self.vel.z *= 0.99;
        } else if self.is_in_water {
            self.vel.x = self.vel.x * 0.8 + wish_dir.x * 0.025;
            self.vel.z = self.vel.z * 0.8 + wish_dir.z * 0.025;
            self.vel.y -= 0.02;
            if input.jump {
                self.vel.y += SharedConstants::WATER_FLOAT_IMPULSE;
            }
            self.vel.y *= 0.8;
            self.fall_distance = 0.0;
        } else if self.is_in_lava {
            self.vel.x = self.vel.x * 0.5 + wish_dir.x * 0.02;
            self.vel.z = self.vel.z * 0.5 + wish_dir.z * 0.02;
            self.vel.y = (self.vel.y - 0.02) * 0.5;
            if input.jump {
                self.vel.y += 0.04;
            }
            self.fall_distance = 0.0;
        } else {
            // Normal ground & air movement matching LivingEntity.travel
            let feet_block = world.get_block(BlockPos::from_vec3(self.pos - DVec3::new(0.0, 0.5, 0.0)));
            let slip = if self.on_ground {
                data.blocks.get(feet_block.id()).slipperiness as f64
            } else {
                1.0
            };
            let drag = if self.on_ground { slip * 0.91 } else { 0.91 };

            let speed_lvl = self.effect_level("speed") as f64;
            let base_speed = 0.1
                * (1.0 + 0.2 * speed_lvl)
                * if self.is_sprinting {
                    1.3
                } else if self.is_sneaking {
                    0.3
                } else {
                    1.0
                };

            let accel = if self.on_ground {
                base_speed * (0.21600002 / (slip * slip * slip))
            } else if self.is_sprinting {
                0.026
            } else {
                0.02
            };

            self.vel.x += wish_dir.x * accel;
            self.vel.z += wish_dir.z * accel;

            // Climb ladders
            let body_block = world.get_block(BlockPos::from_vec3(self.pos)).id();
            if body_block == data.blocks.ladder {
                self.fall_distance = 0.0;
                if input.jump || input.forward > 0.0 {
                    self.vel.y = 0.2;
                } else if input.sneak {
                    self.vel.y = 0.0;
                } else {
                    self.vel.y = self.vel.y.max(-0.15);
                }
            } else if self.on_ground && input.jump {
                let jump_boost = self.effect_level("jump_boost") as f64 * 0.1;
                self.vel.y = SharedConstants::BASE_JUMP_POWER as f64 + jump_boost;
                if self.is_sprinting {
                    self.vel.x += forward_vec.x * 0.2;
                    self.vel.z += forward_vec.z * 0.2;
                    self.exhaustion_level += 0.2;
                } else {
                    self.exhaustion_level += 0.05;
                }
            }

            // Gravity & air drag
            self.vel.y = (self.vel.y - SharedConstants::GRAVITY) * SharedConstants::INPUT_FRICTION as f64;
            self.vel.x *= drag;
            self.vel.z *= drag;
        }

        if self.game_mode == GameMode::Spectator {
            self.pos += self.vel;
            self.on_ground = false;
        } else {
            let (actual, grounded, _) = world.move_aabb(aabb, self.vel, 0.6, &data.blocks);
            if actual.y < 0.0 {
                self.fall_distance += (-actual.y) as f32;
            }
            if grounded {
                if self.fall_distance > 3.0 && !self.is_in_water && world.game_rules.fall_damage {
                    let dmg = self.fall_distance - 3.0;
                    self.damage(dmg, true, data);
                }
                self.fall_distance = 0.0;
                self.is_fall_flying = false;
                self.vel.y = 0.0;
            } else if (actual.y - self.vel.y).abs() > 1e-6 && self.vel.y > 0.0 {
                self.vel.y = 0.0;
            }
            if (actual.x - self.vel.x).abs() > 1e-6 {
                self.vel.x = 0.0;
            }
            if (actual.z - self.vel.z).abs() > 1e-6 {
                self.vel.z = 0.0;
            }
            self.pos += actual;
            self.on_ground = grounded;
        }

        // Exhaustion from sprinting / swimming
        let horiz_dist = (self.vel.x * self.vel.x + self.vel.z * self.vel.z).sqrt() as f32;
        if self.is_sprinting {
            self.exhaustion_level += 0.1 * horiz_dist;
        } else if self.is_in_water {
            self.exhaustion_level += 0.01 * horiz_dist;
        }

        // Oxygen / Drowning
        let head_submerged = world
            .get_block(BlockPos::from_vec3(self.eye_pos()))
            .id()
            == data.blocks.water;
        if head_submerged && self.effect_level("water_breathing") == 0 {
            self.air_supply -= 1;
            if self.air_supply <= -20 {
                self.air_supply = 0;
                if world.game_rules.drowning_damage {
                    self.damage(2.0, true, data);
                }
            }
        } else {
            self.air_supply = (self.air_supply + 4).min(self.max_air);
        }

        // Fire / Lava / Void damage
        if self.is_in_lava {
            self.fire_ticks = 300;
            if self.effect_level("fire_resistance") == 0 && world.game_rules.fire_damage {
                self.damage(4.0, false, data);
            }
        } else if self.is_in_water {
            self.fire_ticks = 0;
        }
        if self.fire_ticks > 0 {
            self.fire_ticks -= 1;
            if self.fire_ticks % 20 == 0
                && self.effect_level("fire_resistance") == 0
                && world.game_rules.fire_damage
            {
                self.damage(1.0, false, data);
            }
        }
        if self.pos.y < (world.dimension.min_y() - 64) as f64 {
            self.damage(4.0, true, data);
        }

        self.tick_food_data(world.difficulty, world.game_rules.natural_regeneration);
    }

    /// Exact `FoodData.tick` rules reverse-engineered from `FoodData.class`.
    fn tick_food_data(&mut self, difficulty: Difficulty, natural_regen: bool) {
        if !self.game_mode.is_survival_or_adventure() {
            return;
        }
        if self.exhaustion_level > 4.0 {
            self.exhaustion_level -= 4.0;
            if self.saturation_level > 0.0 {
                self.saturation_level = (self.saturation_level - 1.0).max(0.0);
            } else if difficulty != Difficulty::Peaceful {
                self.food_level = self.food_level.saturating_sub(1);
            }
        }

        if natural_regen && self.food_level >= 20 && self.saturation_level > 0.0 && self.health < self.max_health {
            self.food_tick_timer += 1;
            if self.food_tick_timer >= 10 {
                self.health = (self.health + 1.0).min(self.max_health);
                self.exhaustion_level += 6.0;
                self.food_tick_timer = 0;
            }
        } else if natural_regen && self.food_level >= 18 && self.health < self.max_health {
            self.food_tick_timer += 1;
            if self.food_tick_timer >= 80 {
                self.health = (self.health + 1.0).min(self.max_health);
                self.exhaustion_level += 6.0;
                self.food_tick_timer = 0;
            }
        } else if self.food_level == 0 {
            self.food_tick_timer += 1;
            if self.food_tick_timer >= 80 {
                let can_starve = match difficulty {
                    Difficulty::Peaceful => false,
                    Difficulty::Easy => self.health > 10.0,
                    Difficulty::Normal => self.health > 1.0,
                    Difficulty::Hard => true,
                };
                if can_starve {
                    self.health = (self.health - 1.0).max(0.0);
                    if self.health <= 0.0 {
                        self.dead = true;
                    }
                }
                self.food_tick_timer = 0;
            }
        } else {
            self.food_tick_timer = 0;
        }
    }
}

/// Exact `net.minecraft.world.damagesource.CombatRules`.
pub struct CombatRules;

impl CombatRules {
    pub fn get_damage_after_absorb(damage: f32, armor: f32, armor_toughness: f32) -> f32 {
        let f = 2.0 + armor_toughness / 4.0;
        let eff = (armor - damage / f).clamp(armor * 0.2, 20.0);
        damage * (1.0 - eff / 25.0)
    }

    pub fn get_damage_after_magic_absorb(damage: f32, enchantment_epf: f32) -> f32 {
        let clamped = enchantment_epf.clamp(0.0, 20.0);
        damage * (1.0 - clamped / 25.0)
    }
}
