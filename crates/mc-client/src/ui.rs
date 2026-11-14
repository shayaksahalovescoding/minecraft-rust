use crate::save::WorldSummary;
use mc_core::{BlockPos, GameMode, SharedConstants};
use mc_data::{GameData, ItemId, ItemStack};
use mc_entities::{EntityKind, EntityManager, Player};
use mc_render::{Renderer, UiBatch};
use mc_world::{BlockEntityData, World};

#[derive(Debug, Clone)]
pub enum Screen {
    Title {
        worlds: Vec<WorldSummary>,
        selected: usize,
        subpage: TitleSubpage,
        new_name: String,
        new_seed: String,
        new_mode: GameMode,
        mp_addr: String,
        mp_status: String,
    },
    InGame,
    Pause,
    Dead,
    Inventory {
        craft: [ItemStack; 4],
        cursor: ItemStack,
    },
    Creative {
        search: String,
        scroll: usize,
        cursor: ItemStack,
    },
    CraftingTable {
        grid: [ItemStack; 9],
        cursor: ItemStack,
    },
    Furnace {
        pos: BlockPos,
        cursor: ItemStack,
    },
    Chest {
        pos: BlockPos,
        cursor: ItemStack,
    },
    Enchanting {
        _pos: BlockPos,
        item: ItemStack,
        lapis: ItemStack,
        cursor: ItemStack,
    },
    Anvil {
        _pos: BlockPos,
        left: ItemStack,
        right: ItemStack,
        cursor: ItemStack,
    },
    BrewingStand {
        pos: BlockPos,
        cursor: ItemStack,
    },
    VillagerTrade {
        entity_id: u64,
        cursor: ItemStack,
    },
    Advancements {
        scroll: usize,
    },
    Chat {
        input: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleSubpage {
    Main,
    Singleplayer,
    CreateWorld,
    Multiplayer,
    Options,
}

#[derive(Debug, Clone)]
pub enum UiAction {
    None,
    Resume,
    StartWorld { name: String, seed: i64, mode: GameMode },
    LoadWorld { summary: WorldSummary },
    SaveAndQuit,
    Respawn,
    SendChat(String),
    PingServer(String),
    ConnectServer(String),
    OpenLan,
    ToggleRenderDist,
    ToggleFov,
    ToggleSound,
}

pub struct HudContext<'a> {
    pub world: Option<&'a World>,
    pub player: Option<&'a Player>,
    pub entities: Option<&'a EntityManager>,
    pub data: &'a GameData,
    pub fps: u32,
    pub loaded_chunks: usize,
    pub show_f3: bool,
    pub render_distance: i32,
    pub fov_deg: f32,
    pub sound_enabled: bool,
    pub chat_log: &'a [(String, f32)],
    pub toast: Option<&'a (String, f32)>,
    pub lan_port: Option<u16>,
}

const COL_BG_DIM: [f32; 4] = [0.04, 0.05, 0.08, 0.78];
const COL_PANEL: [f32; 4] = [0.76, 0.76, 0.76, 0.96];
const COL_PANEL_DARK: [f32; 4] = [0.22, 0.22, 0.24, 0.92];
const COL_SLOT: [f32; 4] = [0.54, 0.54, 0.54, 1.0];
const COL_SLOT_HOVER: [f32; 4] = [1.0, 1.0, 1.0, 0.35];
const COL_BORDER_DARK: [f32; 4] = [0.18, 0.18, 0.18, 1.0];
const COL_WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const COL_GRAY: [f32; 4] = [0.80, 0.80, 0.80, 1.0];
const COL_YELLOW: [f32; 4] = [1.0, 0.92, 0.30, 1.0];
const COL_GREEN: [f32; 4] = [0.35, 0.95, 0.35, 1.0];
const COL_RED: [f32; 4] = [0.95, 0.25, 0.25, 1.0];

pub fn build_ui(
    screen: &Screen,
    ctx: &HudContext<'_>,
    renderer: &Renderer,
    w: f32,
    h: f32,
    mx: f32,
    my: f32,
    batch: &mut UiBatch,
) {
    batch.clear(renderer.atlas.white_uv);

    match screen {
        Screen::Title {
            worlds,
            selected,
            subpage,
            new_name,
            new_seed,
            new_mode,
            mp_addr,
            mp_status,
        } => {
            draw_title_screen(
                batch,
                ctx,
                w,
                h,
                mx,
                my,
                worlds,
                *selected,
                *subpage,
                new_name,
                new_seed,
                *new_mode,
                mp_addr,
                mp_status,
            );
        }
        Screen::InGame => {
            draw_hud(batch, ctx, renderer, w, h);
        }
        Screen::Chat { input } => {
            draw_hud(batch, ctx, renderer, w, h);
            draw_chat_box(batch, ctx, input, w, h);
        }
        Screen::Pause => {
            draw_hud(batch, ctx, renderer, w, h);
            batch.rect(0.0, 0.0, w, h, COL_BG_DIM);
            centered_text(batch, w * 0.5, h * 0.22, 3.0, "Game Menu", COL_WHITE);
            let bw = 260.0;
            let bx = (w - bw) * 0.5;
            draw_button(batch, bx, h * 0.34, bw, 36.0, "Back to Game", mx, my);
            let lan_lbl = match ctx.lan_port {
                Some(p) => format!("LAN Open on :{p}"),
                None => "Open to LAN".to_string(),
            };
            draw_button(batch, bx, h * 0.43, bw, 36.0, &lan_lbl, mx, my);
            draw_button(
                batch,
                bx,
                h * 0.52,
                bw,
                36.0,
                &format!("Render Distance: {} ch", ctx.render_distance),
                mx,
                my,
            );
            draw_button(batch, bx, h * 0.61, bw, 36.0, "Save and Quit to Title", mx, my);
        }
        Screen::Dead => {
            draw_hud(batch, ctx, renderer, w, h);
            batch.rect(0.0, 0.0, w, h, [0.45, 0.04, 0.04, 0.72]);
            centered_text(batch, w * 0.5, h * 0.26, 4.0, "You Died!", COL_WHITE);
            if let Some(p) = ctx.player {
                centered_text(
                    batch,
                    w * 0.5,
                    h * 0.36,
                    2.0,
                    &format!("Score (Total XP): {}", p.total_xp),
                    COL_YELLOW,
                );
            }
            let bw = 240.0;
            let bx = (w - bw) * 0.5;
            draw_button(batch, bx, h * 0.48, bw, 38.0, "Respawn", mx, my);
            draw_button(batch, bx, h * 0.58, bw, 38.0, "Title Screen", mx, my);
        }
        Screen::Inventory { craft, cursor } => {
            draw_hud(batch, ctx, renderer, w, h);
            draw_inventory_screen(batch, ctx, renderer, craft, cursor, w, h, mx, my);
        }
        Screen::Creative { search, scroll, cursor } => {
            draw_hud(batch, ctx, renderer, w, h);
            draw_creative_screen(batch, ctx, renderer, search, *scroll, cursor, w, h, mx, my);
        }
        Screen::CraftingTable { grid, cursor } => {
            draw_hud(batch, ctx, renderer, w, h);
            draw_crafting_table_screen(batch, ctx, renderer, grid, cursor, w, h, mx, my);
        }
        Screen::Furnace { pos, cursor } => {
            draw_hud(batch, ctx, renderer, w, h);
            draw_furnace_screen(batch, ctx, renderer, *pos, cursor, w, h, mx, my);
        }
        Screen::Chest { pos, cursor } => {
            draw_hud(batch, ctx, renderer, w, h);
            draw_chest_screen(batch, ctx, renderer, *pos, cursor, w, h, mx, my);
        }
        Screen::Enchanting { _pos: _, item, lapis, cursor } => {
            draw_hud(batch, ctx, renderer, w, h);
            draw_enchanting_screen(batch, ctx, renderer, item, lapis, cursor, w, h, mx, my);
        }
        Screen::Anvil { _pos: _, left, right, cursor } => {
            draw_hud(batch, ctx, renderer, w, h);
            draw_anvil_screen(batch, ctx, renderer, left, right, cursor, w, h, mx, my);
        }
        Screen::BrewingStand { pos, cursor } => {
            draw_hud(batch, ctx, renderer, w, h);
            draw_brewing_screen(batch, ctx, renderer, *pos, cursor, w, h, mx, my);
        }
        Screen::VillagerTrade { entity_id, cursor } => {
            draw_hud(batch, ctx, renderer, w, h);
            draw_villager_screen(batch, ctx, renderer, *entity_id, cursor, w, h, mx, my);
        }
        Screen::Advancements { scroll } => {
            draw_hud(batch, ctx, renderer, w, h);
            draw_advancements_screen(batch, ctx, *scroll, w, h, mx, my);
        }
    }
}

fn draw_title_screen(
    batch: &mut UiBatch,
    ctx: &HudContext<'_>,
    w: f32,
    h: f32,
    mx: f32,
    my: f32,
    worlds: &[WorldSummary],
    selected: usize,
    subpage: TitleSubpage,
    new_name: &str,
    new_seed: &str,
    new_mode: GameMode,
    mp_addr: &str,
    mp_status: &str,
) {
    // Rich gradient-look tiled backdrop
    batch.rect(0.0, 0.0, w, h * 0.55, [0.16, 0.28, 0.46, 1.0]);
    batch.rect(0.0, h * 0.55, w, h * 0.45, [0.10, 0.14, 0.20, 1.0]);
    batch.rect(0.0, 0.0, w, h, [0.0, 0.0, 0.0, 0.25]);

    centered_text(batch, w * 0.5, 42.0, 5.0, "MINECRAFT", COL_WHITE);
    centered_text(batch, w * 0.5, 92.0, 2.0, "Java 26.1.2 Rust Clean-Room Edition", COL_YELLOW);

    let bw = 300.0;
    let bx = (w - bw) * 0.5;

    match subpage {
        TitleSubpage::Main => {
            draw_button(batch, bx, 175.0, bw, 38.0, "Singleplayer", mx, my);
            draw_button(batch, bx, 225.0, bw, 38.0, "Multiplayer", mx, my);
            draw_button(batch, bx, 275.0, bw, 38.0, "Quick Start Survival", mx, my);
            draw_button(batch, bx, 325.0, bw, 38.0, "Options...", mx, my);
            batch.text_shadow(
                14.0,
                h - 26.0,
                1.6,
                &format!(
                    "Minecraft 26.1.2 (Proto {} / World {})  Blocks: {}  Items: {}  Recipes: {}",
                    SharedConstants::PROTOCOL_VERSION,
                    SharedConstants::WORLD_VERSION,
                    ctx.data.blocks.len(),
                    ctx.data.items.len(),
                    ctx.data.recipes.shaped.len() + ctx.data.recipes.shapeless.len() + ctx.data.recipes.cooking.len()
                ),
                COL_GRAY,
            );
        }
        TitleSubpage::Singleplayer => {
            let pw = 520.0;
            let px = (w - pw) * 0.5;
            batch.rect(px, 130.0, pw, 290.0, COL_PANEL_DARK);
            batch.rect_outline(px, 130.0, pw, 290.0, 2.0, COL_BORDER_DARK);
            centered_text(batch, w * 0.5, 140.0, 2.0, "Select World", COL_WHITE);
            if worlds.is_empty() {
                centered_text(batch, w * 0.5, 250.0, 1.8, "No saved worlds yet - Create New World!", COL_GRAY);
            } else {
                for (i, ws) in worlds.iter().take(5).enumerate() {
                    let ry = 175.0 + i as f32 * 44.0;
                    let col = if i == selected { [0.28, 0.42, 0.25, 0.95] } else { [0.16, 0.16, 0.18, 0.9] };
                    batch.rect(px + 14.0, ry, pw - 28.0, 38.0, col);
                    batch.text_shadow(px + 24.0, ry + 6.0, 1.7, &ws.meta.name, COL_WHITE);
                    batch.text_shadow(
                        px + 24.0,
                        ry + 22.0,
                        1.3,
                        &format!("{}  Seed: {}  Mode: {}", ws.folder_name, ws.meta.seed, ws.meta.game_mode.name()),
                        COL_GRAY,
                    );
                }
            }
            draw_button(batch, px, 434.0, 165.0, 36.0, "Play Selected", mx, my);
            draw_button(batch, px + 177.0, 434.0, 165.0, 36.0, "Create New World", mx, my);
            draw_button(batch, px + 354.0, 434.0, 166.0, 36.0, "Back", mx, my);
        }
        TitleSubpage::CreateWorld => {
            let pw = 420.0;
            let px = (w - pw) * 0.5;
            batch.rect(px, 135.0, pw, 270.0, COL_PANEL_DARK);
            centered_text(batch, w * 0.5, 145.0, 2.2, "Create New World", COL_WHITE);

            batch.text_shadow(px + 20.0, 185.0, 1.6, "World Name (type on keyboard):", COL_GRAY);
            batch.rect(px + 20.0, 206.0, pw - 40.0, 30.0, [0.05, 0.05, 0.06, 1.0]);
            batch.rect_outline(px + 20.0, 206.0, pw - 40.0, 30.0, 1.5, COL_GRAY);
            batch.text_shadow(px + 28.0, 214.0, 1.7, &format!("{new_name}_"), COL_WHITE);

            batch.text_shadow(px + 20.0, 248.0, 1.6, "Seed (optional number):", COL_GRAY);
            batch.rect(px + 20.0, 269.0, pw - 40.0, 30.0, [0.05, 0.05, 0.06, 1.0]);
            batch.rect_outline(px + 20.0, 269.0, pw - 40.0, 30.0, 1.5, COL_GRAY);
            batch.text_shadow(px + 28.0, 277.0, 1.7, new_seed, COL_YELLOW);

            draw_button(
                batch,
                px + 20.0,
                314.0,
                pw - 40.0,
                34.0,
                &format!("Game Mode: {}", new_mode.name()),
                mx,
                my,
            );
            draw_button(batch, px + 20.0, 360.0, 180.0, 36.0, "Create New World", mx, my);
            draw_button(batch, px + 220.0, 360.0, 180.0, 36.0, "Cancel", mx, my);
        }
        TitleSubpage::Multiplayer => {
            let pw = 440.0;
            let px = (w - pw) * 0.5;
            batch.rect(px, 145.0, pw, 250.0, COL_PANEL_DARK);
            centered_text(batch, w * 0.5, 158.0, 2.2, "Multiplayer (Proto 775)", COL_WHITE);
            batch.text_shadow(px + 20.0, 196.0, 1.6, "Server Address (host:port):", COL_GRAY);
            batch.rect(px + 20.0, 218.0, pw - 40.0, 32.0, [0.05, 0.05, 0.06, 1.0]);
            batch.rect_outline(px + 20.0, 218.0, pw - 40.0, 32.0, 1.5, COL_GRAY);
            batch.text_shadow(px + 28.0, 226.0, 1.7, &format!("{mp_addr}_"), COL_WHITE);
            batch.text_shadow(px + 20.0, 264.0, 1.5, mp_status, COL_YELLOW);

            draw_button(batch, px + 20.0, 300.0, 125.0, 36.0, "Ping Status", mx, my);
            draw_button(batch, px + 157.0, 300.0, 130.0, 36.0, "Join Server", mx, my);
            draw_button(batch, px + 299.0, 300.0, 121.0, 36.0, "Back", mx, my);
        }
        TitleSubpage::Options => {
            let pw = 360.0;
            let px = (w - pw) * 0.5;
            batch.rect(px, 150.0, pw, 260.0, COL_PANEL_DARK);
            centered_text(batch, w * 0.5, 162.0, 2.2, "Options", COL_WHITE);
            draw_button(
                batch,
                px + 30.0,
                205.0,
                pw - 60.0,
                34.0,
                &format!("Render Distance: {} chunks", ctx.render_distance),
                mx,
                my,
            );
            draw_button(
                batch,
                px + 30.0,
                250.0,
                pw - 60.0,
                34.0,
                &format!("FOV: {:.0}", ctx.fov_deg),
                mx,
                my,
            );
            draw_button(
                batch,
                px + 30.0,
                295.0,
                pw - 60.0,
                34.0,
                if ctx.sound_enabled { "Audio: ON" } else { "Audio: OFF" },
                mx,
                my,
            );
            draw_button(batch, px + 30.0, 355.0, pw - 60.0, 36.0, "Done", mx, my);
        }
    }
}

fn draw_hud(
    batch: &mut UiBatch,
    ctx: &HudContext<'_>,
    renderer: &Renderer,
    w: f32,
    h: f32,
) {
    let Some(player) = ctx.player else {
        return;
    };

    // Crosshair
    let cx = w * 0.5;
    let cy = h * 0.5;
    batch.rect(cx - 8.0, cy - 1.5, 16.0, 3.0, [1.0, 1.0, 1.0, 0.85]);
    batch.rect(cx - 1.5, cy - 8.0, 3.0, 16.0, [1.0, 1.0, 1.0, 0.85]);

    // Boss Bar for Ender Dragon / Wither
    if let Some(entities) = ctx.entities {
        if let Some(boss) = entities
            .entities
            .iter()
            .find(|e| !e.removed && matches!(e.kind, EntityKind::EnderDragon | EntityKind::Wither))
        {
            let bw = 340.0;
            let bx = (w - bw) * 0.5;
            let name = if boss.kind == EntityKind::EnderDragon {
                "Ender Dragon"
            } else {
                "Wither"
            };
            centered_text(batch, w * 0.5, 10.0, 1.6, name, [0.95, 0.55, 0.95, 1.0]);
            batch.rect(bx, 28.0, bw, 8.0, [0.15, 0.05, 0.18, 0.9]);
            let pct = (boss.health / boss.max_health.max(1.0)).clamp(0.0, 1.0);
            batch.rect(bx + 1.0, 29.0, (bw - 2.0) * pct, 6.0, [0.85, 0.18, 0.88, 1.0]);
        }
    }

    // Hotbar
    let slot_sz = 40.0;
    let hb_w = 9.0 * slot_sz + 8.0;
    let hb_x = (w - hb_w) * 0.5;
    let hb_y = h - slot_sz - 12.0;
    batch.rect(hb_x - 3.0, hb_y - 3.0, hb_w + 6.0, slot_sz + 6.0, [0.12, 0.12, 0.14, 0.85]);
    for i in 0..9 {
        let sx = hb_x + i as f32 * (slot_sz + 1.0);
        let active = player.selected_slot == i;
        draw_slot(batch, renderer, ctx.data, sx, hb_y, slot_sz, &player.inventory[i], false, active);
    }

    // Selected item label
    let held = player.main_hand();
    if !held.is_empty() {
        let label = held
            .custom_name
            .as_deref()
            .unwrap_or(&ctx.data.items.get(held.item).name);
        centered_text(batch, w * 0.5, hb_y - 44.0, 1.6, label, COL_WHITE);
    }

    // Survival Bars (Hearts, Armor, Food, Oxygen, XP)
    if player.game_mode.is_survival_or_adventure() {
        let bar_w = (hb_w - 20.0) * 0.5;
        // XP Bar
        let xp_y = hb_y - 12.0;
        batch.rect(hb_x, xp_y, hb_w, 6.0, [0.1, 0.1, 0.1, 0.85]);
        batch.rect(hb_x + 1.0, xp_y + 1.0, (hb_w - 2.0) * player.xp_progress.clamp(0.0, 1.0), 4.0, COL_GREEN);
        if player.xp_level > 0 {
            centered_text(batch, w * 0.5, xp_y - 14.0, 1.5, &format!("{}", player.xp_level), COL_GREEN);
        }

        // Hearts (10 hearts = 20 HP)
        let hearts_y = xp_y - 16.0;
        for i in 0..10 {
            let hx = hb_x + i as f32 * 16.0;
            batch.rect(hx, hearts_y, 13.0, 12.0, [0.15, 0.03, 0.03, 0.9]);
            let hp = player.health - (i as f32 * 2.0);
            if hp >= 1.5 {
                batch.rect(hx + 1.0, hearts_y + 1.0, 11.0, 10.0, COL_RED);
            } else if hp >= 0.5 {
                batch.rect(hx + 1.0, hearts_y + 1.0, 5.5, 10.0, COL_RED);
            }
        }

        // Armor above hearts
        let (armor_def, _, _) = player.total_armor_defense(ctx.data);
        if armor_def > 0.0 {
            for i in 0..10 {
                let ax = hb_x + i as f32 * 16.0;
                let pts = armor_def - (i as f32 * 2.0);
                let col = if pts >= 1.5 {
                    [0.85, 0.88, 0.92, 1.0]
                } else if pts >= 0.5 {
                    [0.55, 0.58, 0.62, 1.0]
                } else {
                    [0.2, 0.2, 0.2, 0.5]
                };
                batch.rect(ax, hearts_y - 14.0, 12.0, 10.0, col);
            }
        }

        // Food bar on right side
        let food_x0 = hb_x + hb_w - bar_w;
        for i in 0..10 {
            let fx = food_x0 + (9 - i) as f32 * 16.0 + (bar_w - 160.0);
            batch.rect(fx, hearts_y, 13.0, 12.0, [0.18, 0.10, 0.05, 0.9]);
            let f = player.food_level as i32 - (i as i32 * 2);
            if f >= 2 {
                batch.rect(fx + 1.0, hearts_y + 1.0, 11.0, 10.0, [0.85, 0.52, 0.20, 1.0]);
            } else if f == 1 {
                batch.rect(fx + 6.5, hearts_y + 1.0, 5.5, 10.0, [0.85, 0.52, 0.20, 1.0]);
            }
        }

        // Air supply bubbles when underwater
        if player.air_supply < player.max_air {
            for i in 0..10 {
                let threshold = i * 30;
                if player.air_supply > threshold {
                    let ox = food_x0 + (9 - i) as f32 * 16.0 + (bar_w - 160.0);
                    batch.rect(ox, hearts_y - 14.0, 11.0, 11.0, [0.35, 0.80, 1.0, 0.9]);
                }
            }
        }
    }

    // F3 Debug HUD
    if ctx.show_f3 {
        if let Some(world) = ctx.world {
            let bp = mc_core::BlockPos::from_vec3(player.pos);
            let cp = bp.chunk_pos();
            let sky_l = world.get_sky_light(bp);
            let blk_l = world.get_block_light(bp);
            let biome = world
                .chunks
                .get(&cp)
                .map(|c| c.biome_at(bp.local_x(), bp.local_z()))
                .unwrap_or("unknown");
            let lines = [
                format!("Minecraft 26.1.2 (Proto 775 / Data 4790)  {} FPS", ctx.fps),
                format!("XYZ: {:.3} / {:.3} / {:.3}  Mode: {}", player.pos.x, player.pos.y, player.pos.z, player.game_mode.name()),
                format!("Block: {} {} {}  Chunk: {} {}  Loaded: {}", bp.x, bp.y, bp.z, cp.x, cp.z, ctx.loaded_chunks),
                format!("Dim: {}  Biome: {}  DayTime: {}", world.dimension.id_str(), biome, world.day_time),
                format!("Light: {} ({} sky, {} block)  Entities: {}", sky_l.max(blk_l), sky_l, blk_l, ctx.entities.map(|e| e.entities.len()).unwrap_or(0)),
                format!("Health: {:.1}/{:.0}  Hunger: {} ({:.1} sat)  XP Lv: {}", player.health, player.max_health, player.food_level, player.saturation_level, player.xp_level),
            ];
            for (i, line) in lines.iter().enumerate() {
                let lw = UiBatch::text_width(1.5, line) + 10.0;
                batch.rect(6.0, 6.0 + i as f32 * 18.0, lw, 16.0, [0.05, 0.05, 0.07, 0.65]);
                batch.text_shadow(10.0, 8.0 + i as f32 * 18.0, 1.5, line, COL_WHITE);
            }
        }
    }

    // Toast banner (Advancement unlocked!)
    if let Some((title, timer)) = ctx.toast {
        if *timer > 0.0 {
            let tw = 270.0;
            let tx = w - tw - 12.0;
            batch.rect(tx, 12.0, tw, 46.0, [0.12, 0.12, 0.16, 0.94]);
            batch.rect_outline(tx, 12.0, tw, 46.0, 2.0, COL_YELLOW);
            batch.text_shadow(tx + 12.0, 19.0, 1.4, "Advancement Made!", COL_YELLOW);
            batch.text_shadow(tx + 12.0, 36.0, 1.5, title, COL_WHITE);
        }
    }

    // Recent chat messages
    let visible: Vec<&(String, f32)> = ctx.chat_log.iter().rev().take(7).collect();
    for (idx, (msg, age)) in visible.into_iter().rev().enumerate() {
        let alpha = (*age).clamp(0.0, 1.0);
        if alpha > 0.02 {
            let y = h - 106.0 - (6 - idx as i32).max(0) as f32 * 18.0;
            let mw = UiBatch::text_width(1.5, msg) + 10.0;
            batch.rect(10.0, y, mw, 17.0, [0.0, 0.0, 0.0, 0.55 * alpha]);
            batch.text_shadow(14.0, y + 3.0, 1.5, msg, [1.0, 1.0, 1.0, alpha]);
        }
    }
}

fn draw_chat_box(batch: &mut UiBatch, _ctx: &HudContext<'_>, input: &str, w: f32, h: f32) {
    batch.rect(10.0, h - 38.0, w - 20.0, 26.0, [0.05, 0.05, 0.06, 0.85]);
    batch.rect_outline(10.0, h - 38.0, w - 20.0, 26.0, 1.5, COL_GRAY);
    batch.text_shadow(16.0, h - 31.0, 1.7, &format!(">{input}_"), COL_WHITE);
}

fn draw_player_inventory_grid(
    batch: &mut UiBatch,
    renderer: &Renderer,
    data: &GameData,
    player: &Player,
    px: f32,
    py: f32,
    mx: f32,
    my: f32,
) {
    // 27 main slots (9..36) + 9 hotbar slots (0..9)
    for row in 0..3 {
        for col in 0..9 {
            let idx = 9 + row * 9 + col;
            let sx = px + 14.0 + col as f32 * 40.0;
            let sy = py + row as f32 * 40.0;
            let hov = point_in(mx, my, sx, sy, 36.0, 36.0);
            draw_slot(batch, renderer, data, sx, sy, 36.0, &player.inventory[idx], hov, false);
        }
    }
    for col in 0..9 {
        let sx = px + 14.0 + col as f32 * 40.0;
        let sy = py + 128.0;
        let hov = point_in(mx, my, sx, sy, 36.0, 36.0);
        draw_slot(batch, renderer, data, sx, sy, 36.0, &player.inventory[col], hov, false);
    }
}

fn draw_cursor_stack(
    batch: &mut UiBatch,
    renderer: &Renderer,
    cursor: &ItemStack,
    mx: f32,
    my: f32,
) {
    if !cursor.is_empty() {
        let uv = renderer.item_icon_uv(cursor.item);
        batch.textured_rect(mx - 14.0, my - 14.0, 28.0, 28.0, uv, COL_WHITE);
        if cursor.count > 1 {
            batch.text_shadow(mx + 2.0, my + 2.0, 1.4, &format!("{}", cursor.count), COL_WHITE);
        }
    }
}

fn draw_inventory_screen(
    batch: &mut UiBatch,
    ctx: &HudContext<'_>,
    renderer: &Renderer,
    craft: &[ItemStack; 4],
    cursor: &ItemStack,
    w: f32,
    h: f32,
    mx: f32,
    my: f32,
) {
    let Some(player) = ctx.player else {
        return;
    };
    let pw = 384.0;
    let ph = 330.0;
    let px = (w - pw) * 0.5;
    let py = (h - ph) * 0.5;
    batch.rect(0.0, 0.0, w, h, COL_BG_DIM);
    batch.rect(px, py, pw, ph, COL_PANEL);
    batch.rect_outline(px, py, pw, ph, 2.0, COL_BORDER_DARK);
    batch.text(px + 14.0, py + 10.0, 1.7, "Inventory & Crafting (2x2)", COL_BORDER_DARK);

    // 4 Armor slots [Helmet..Boots]
    for i in 0..4 {
        let sx = px + 14.0;
        let sy = py + 34.0 + i as f32 * 28.0;
        let armor_idx = 3 - i;
        draw_slot(batch, renderer, ctx.data, sx, sy, 26.0, &player.armor[armor_idx], point_in(mx, my, sx, sy, 26.0, 26.0), false);
    }
    // Offhand
    draw_slot(batch, renderer, ctx.data, px + 60.0, py + 118.0, 32.0, &player.offhand, point_in(mx, my, px + 60.0, py + 118.0, 32.0, 32.0), false);

    // 2x2 craft grid + output
    for i in 0..4 {
        let sx = px + 150.0 + (i % 2) as f32 * 40.0;
        let sy = py + 44.0 + (i / 2) as f32 * 40.0;
        draw_slot(batch, renderer, ctx.data, sx, sy, 36.0, &craft[i], point_in(mx, my, sx, sy, 36.0, 36.0), false);
    }
    batch.text(px + 240.0, py + 72.0, 2.0, "->", COL_BORDER_DARK);
    let out = ctx
        .data
        .recipes
        .match_crafting(2, 2, craft)
        .map(|m| m.result)
        .unwrap_or(ItemStack::EMPTY);
    draw_slot(batch, renderer, ctx.data, px + 280.0, py + 64.0, 38.0, &out, point_in(mx, my, px + 280.0, py + 64.0, 38.0, 38.0), !out.is_empty());

    draw_player_inventory_grid(batch, renderer, ctx.data, player, px, py + 156.0, mx, my);
    draw_cursor_stack(batch, renderer, cursor, mx, my);
}

fn draw_crafting_table_screen(
    batch: &mut UiBatch,
    ctx: &HudContext<'_>,
    renderer: &Renderer,
    grid: &[ItemStack; 9],
    cursor: &ItemStack,
    w: f32,
    h: f32,
    mx: f32,
    my: f32,
) {
    let Some(player) = ctx.player else { return; };
    let pw = 384.0;
    let ph = 350.0;
    let px = (w - pw) * 0.5;
    let py = (h - ph) * 0.5;
    batch.rect(0.0, 0.0, w, h, COL_BG_DIM);
    batch.rect(px, py, pw, ph, COL_PANEL);
    batch.rect_outline(px, py, pw, ph, 2.0, COL_BORDER_DARK);
    batch.text(px + 14.0, py + 10.0, 1.7, "Crafting (3x3)", COL_BORDER_DARK);

    for i in 0..9 {
        let sx = px + 44.0 + (i % 3) as f32 * 40.0;
        let sy = py + 38.0 + (i / 3) as f32 * 40.0;
        draw_slot(batch, renderer, ctx.data, sx, sy, 36.0, &grid[i], point_in(mx, my, sx, sy, 36.0, 36.0), false);
    }
    batch.text(px + 190.0, py + 84.0, 2.2, "->", COL_BORDER_DARK);
    let out = ctx
        .data
        .recipes
        .match_crafting(3, 3, grid)
        .map(|m| m.result)
        .unwrap_or(ItemStack::EMPTY);
    draw_slot(batch, renderer, ctx.data, px + 245.0, py + 76.0, 42.0, &out, point_in(mx, my, px + 245.0, py + 76.0, 42.0, 42.0), !out.is_empty());

    draw_player_inventory_grid(batch, renderer, ctx.data, player, px, py + 174.0, mx, my);
    draw_cursor_stack(batch, renderer, cursor, mx, my);
}

fn draw_creative_screen(
    batch: &mut UiBatch,
    ctx: &HudContext<'_>,
    renderer: &Renderer,
    search: &str,
    scroll: usize,
    cursor: &ItemStack,
    w: f32,
    h: f32,
    mx: f32,
    my: f32,
) {
    let Some(player) = ctx.player else { return; };
    let pw = 420.0;
    let ph = 390.0;
    let px = (w - pw) * 0.5;
    let py = (h - ph) * 0.5;
    batch.rect(0.0, 0.0, w, h, COL_BG_DIM);
    batch.rect(px, py, pw, ph, COL_PANEL);
    batch.rect_outline(px, py, pw, ph, 2.0, COL_BORDER_DARK);

    batch.text(px + 14.0, py + 10.0, 1.6, "Creative Catalog (type to filter):", COL_BORDER_DARK);
    batch.rect(px + 14.0, py + 30.0, 260.0, 24.0, [0.1, 0.1, 0.12, 1.0]);
    batch.text_shadow(px + 20.0, py + 35.0, 1.5, &format!("{search}_"), COL_WHITE);
    draw_button(batch, px + 286.0, py + 28.0, 56.0, 26.0, "Prev", mx, my);
    draw_button(batch, px + 348.0, py + 28.0, 56.0, 26.0, "Next", mx, my);

    let matches = filtered_creative_items(ctx.data, search);
    let cols = 9;
    let start = scroll * cols;
    for slot_idx in 0..45 {
        let r = slot_idx / cols;
        let c = slot_idx % cols;
        let sx = px + 14.0 + c as f32 * 40.0;
        let sy = py + 64.0 + r as f32 * 40.0;
        let st = matches
            .get(start + slot_idx)
            .map(|id| ItemStack::new(*id, 1))
            .unwrap_or(ItemStack::EMPTY);
        draw_slot(batch, renderer, ctx.data, sx, sy, 36.0, &st, point_in(mx, my, sx, sy, 36.0, 36.0), false);
    }

    // Hotbar row at bottom of creative screen
    for col in 0..9 {
        let sx = px + 14.0 + col as f32 * 40.0;
        let sy = py + 332.0;
        draw_slot(batch, renderer, ctx.data, sx, sy, 36.0, &player.inventory[col], point_in(mx, my, sx, sy, 36.0, 36.0), false);
    }
    draw_cursor_stack(batch, renderer, cursor, mx, my);
}

fn draw_furnace_screen(
    batch: &mut UiBatch,
    ctx: &HudContext<'_>,
    renderer: &Renderer,
    pos: BlockPos,
    cursor: &ItemStack,
    w: f32,
    h: f32,
    mx: f32,
    my: f32,
) {
    let (Some(world), Some(player)) = (ctx.world, ctx.player) else { return; };
    let pw = 384.0;
    let ph = 330.0;
    let px = (w - pw) * 0.5;
    let py = (h - ph) * 0.5;
    batch.rect(0.0, 0.0, w, h, COL_BG_DIM);
    batch.rect(px, py, pw, ph, COL_PANEL);
    batch.text(px + 14.0, py + 10.0, 1.7, "Furnace / Smelting", COL_BORDER_DARK);

    if let Some(BlockEntityData::Furnace {
        input,
        fuel,
        output,
        burn_time,
        burn_duration,
        cook_time,
        cook_total,
        ..
    }) = world.get_block_entity(pos)
    {
        draw_slot(batch, renderer, ctx.data, px + 95.0, py + 40.0, 38.0, input, point_in(mx, my, px + 95.0, py + 40.0, 38.0, 38.0), false);
        draw_slot(batch, renderer, ctx.data, px + 95.0, py + 106.0, 38.0, fuel, point_in(mx, my, px + 95.0, py + 106.0, 38.0, 38.0), false);
        draw_slot(batch, renderer, ctx.data, px + 225.0, py + 72.0, 42.0, output, point_in(mx, my, px + 225.0, py + 72.0, 42.0, 42.0), false);

        let burn_pct = if *burn_duration > 0 {
            *burn_time as f32 / *burn_duration as f32
        } else {
            0.0
        };
        let cook_pct = if *cook_total > 0 {
            *cook_time as f32 / *cook_total as f32
        } else {
            0.0
        };
        batch.rect(px + 148.0, py + 84.0, 62.0, 14.0, COL_SLOT);
        batch.rect(px + 148.0, py + 84.0, 62.0 * cook_pct, 14.0, COL_GREEN);
        batch.rect(px + 104.0, py + 82.0, 20.0, 18.0, COL_SLOT);
        batch.rect(px + 104.0, py + 82.0 + 18.0 * (1.0 - burn_pct), 20.0, 18.0 * burn_pct, [0.95, 0.45, 0.1, 1.0]);
    }
    draw_player_inventory_grid(batch, renderer, ctx.data, player, px, py + 156.0, mx, my);
    draw_cursor_stack(batch, renderer, cursor, mx, my);
}

fn draw_chest_screen(
    batch: &mut UiBatch,
    ctx: &HudContext<'_>,
    renderer: &Renderer,
    pos: BlockPos,
    cursor: &ItemStack,
    w: f32,
    h: f32,
    mx: f32,
    my: f32,
) {
    let (Some(world), Some(player)) = (ctx.world, ctx.player) else { return; };
    let pw = 384.0;
    let ph = 350.0;
    let px = (w - pw) * 0.5;
    let py = (h - ph) * 0.5;
    batch.rect(0.0, 0.0, w, h, COL_BG_DIM);
    batch.rect(px, py, pw, ph, COL_PANEL);
    batch.text(px + 14.0, py + 10.0, 1.7, "Container (27 Slots)", COL_BORDER_DARK);

    if let Some(BlockEntityData::Chest { items } | BlockEntityData::Barrel { items } | BlockEntityData::ShulkerBox { items }) = world.get_block_entity(pos) {
        for r in 0..3 {
            for c in 0..9 {
                let idx = r * 9 + c;
                let sx = px + 14.0 + c as f32 * 40.0;
                let sy = py + 36.0 + r as f32 * 40.0;
                let empty = ItemStack::EMPTY;
                let st = items.get(idx).unwrap_or(&empty);
                draw_slot(batch, renderer, ctx.data, sx, sy, 36.0, st, point_in(mx, my, sx, sy, 36.0, 36.0), false);
            }
        }
    }
    draw_player_inventory_grid(batch, renderer, ctx.data, player, px, py + 174.0, mx, my);
    draw_cursor_stack(batch, renderer, cursor, mx, my);
}

fn draw_enchanting_screen(
    batch: &mut UiBatch,
    ctx: &HudContext<'_>,
    renderer: &Renderer,
    item: &ItemStack,
    lapis: &ItemStack,
    cursor: &ItemStack,
    w: f32,
    h: f32,
    mx: f32,
    my: f32,
) {
    let Some(player) = ctx.player else { return; };
    let pw = 384.0;
    let ph = 330.0;
    let px = (w - pw) * 0.5;
    let py = (h - ph) * 0.5;
    batch.rect(0.0, 0.0, w, h, COL_BG_DIM);
    batch.rect(px, py, pw, ph, COL_PANEL);
    batch.text(px + 14.0, py + 10.0, 1.7, "Enchanting Table", COL_BORDER_DARK);

    draw_slot(batch, renderer, ctx.data, px + 24.0, py + 54.0, 38.0, item, point_in(mx, my, px + 24.0, py + 54.0, 38.0, 38.0), false);
    draw_slot(batch, renderer, ctx.data, px + 70.0, py + 54.0, 38.0, lapis, point_in(mx, my, px + 70.0, py + 54.0, 38.0, 38.0), false);
    draw_button(batch, px + 128.0, py + 36.0, 238.0, 28.0, "Tier I (1 Lv, 1 Lapis)", mx, my);
    draw_button(batch, px + 128.0, py + 70.0, 238.0, 28.0, "Tier II (3 Lv, 2 Lapis)", mx, my);
    draw_button(batch, px + 128.0, py + 104.0, 238.0, 28.0, "Tier III (6 Lv, 3 Lapis)", mx, my);

    draw_player_inventory_grid(batch, renderer, ctx.data, player, px, py + 156.0, mx, my);
    draw_cursor_stack(batch, renderer, cursor, mx, my);
}

fn draw_anvil_screen(
    batch: &mut UiBatch,
    ctx: &HudContext<'_>,
    renderer: &Renderer,
    left: &ItemStack,
    right: &ItemStack,
    cursor: &ItemStack,
    w: f32,
    h: f32,
    mx: f32,
    my: f32,
) {
    let Some(player) = ctx.player else { return; };
    let pw = 384.0;
    let ph = 330.0;
    let px = (w - pw) * 0.5;
    let py = (h - ph) * 0.5;
    batch.rect(0.0, 0.0, w, h, COL_BG_DIM);
    batch.rect(px, py, pw, ph, COL_PANEL);
    batch.text(px + 14.0, py + 10.0, 1.7, "Repair & Combine (Anvil)", COL_BORDER_DARK);

    draw_slot(batch, renderer, ctx.data, px + 44.0, py + 62.0, 38.0, left, point_in(mx, my, px + 44.0, py + 62.0, 38.0, 38.0), false);
    batch.text(px + 94.0, py + 70.0, 2.0, "+", COL_BORDER_DARK);
    draw_slot(batch, renderer, ctx.data, px + 116.0, py + 62.0, 38.0, right, point_in(mx, my, px + 116.0, py + 62.0, 38.0, 38.0), false);
    batch.text(px + 170.0, py + 70.0, 2.0, "->", COL_BORDER_DARK);
    let out = compute_anvil_result(left, right);
    draw_slot(batch, renderer, ctx.data, px + 220.0, py + 62.0, 42.0, &out, point_in(mx, my, px + 220.0, py + 62.0, 42.0, 42.0), !out.is_empty());

    draw_player_inventory_grid(batch, renderer, ctx.data, player, px, py + 156.0, mx, my);
    draw_cursor_stack(batch, renderer, cursor, mx, my);
}

fn draw_brewing_screen(
    batch: &mut UiBatch,
    ctx: &HudContext<'_>,
    renderer: &Renderer,
    pos: BlockPos,
    cursor: &ItemStack,
    w: f32,
    h: f32,
    mx: f32,
    my: f32,
) {
    let (Some(world), Some(player)) = (ctx.world, ctx.player) else { return; };
    let pw = 384.0;
    let ph = 330.0;
    let px = (w - pw) * 0.5;
    let py = (h - ph) * 0.5;
    batch.rect(0.0, 0.0, w, h, COL_BG_DIM);
    batch.rect(px, py, pw, ph, COL_PANEL);
    batch.text(px + 14.0, py + 10.0, 1.7, "Brewing Stand", COL_BORDER_DARK);

    if let Some(BlockEntityData::BrewingStand { bottles, ingredient, fuel, .. }) = world.get_block_entity(pos) {
        draw_slot(batch, renderer, ctx.data, px + 44.0, py + 42.0, 36.0, fuel, point_in(mx, my, px + 44.0, py + 42.0, 36.0, 36.0), false);
        draw_slot(batch, renderer, ctx.data, px + 172.0, py + 36.0, 36.0, ingredient, point_in(mx, my, px + 172.0, py + 36.0, 36.0, 36.0), false);
        for i in 0..3 {
            let sx = px + 116.0 + i as f32 * 56.0;
            let sy = py + 96.0;
            draw_slot(batch, renderer, ctx.data, sx, sy, 36.0, &bottles[i], point_in(mx, my, sx, sy, 36.0, 36.0), false);
        }
    }
    draw_player_inventory_grid(batch, renderer, ctx.data, player, px, py + 156.0, mx, my);
    draw_cursor_stack(batch, renderer, cursor, mx, my);
}

fn draw_villager_screen(
    batch: &mut UiBatch,
    ctx: &HudContext<'_>,
    renderer: &Renderer,
    entity_id: u64,
    cursor: &ItemStack,
    w: f32,
    h: f32,
    mx: f32,
    my: f32,
) {
    let Some(player) = ctx.player else { return; };
    let pw = 420.0;
    let ph = 360.0;
    let px = (w - pw) * 0.5;
    let py = (h - ph) * 0.5;
    batch.rect(0.0, 0.0, w, h, COL_BG_DIM);
    batch.rect(px, py, pw, ph, COL_PANEL);
    batch.text(px + 14.0, py + 10.0, 1.7, "Villager Trading (click offer)", COL_BORDER_DARK);

    if let Some(ent) = ctx.entities.and_then(|em| em.entities.iter().find(|e| e.id == entity_id)) {
        for (i, offer) in ent.villager_offers.iter().take(4).enumerate() {
            let ry = py + 36.0 + i as f32 * 34.0;
            let wn = &ctx.data.items.get(offer.wants.item).name;
            let gn = &ctx.data.items.get(offer.gives.item).name;
            let lbl = format!("{}x {} -> {}x {} ({}/{})", offer.wants.count, wn, offer.gives.count, gn, offer.uses, offer.max_uses);
            draw_button(batch, px + 14.0, ry, pw - 28.0, 30.0, &lbl, mx, my);
        }
    }
    draw_player_inventory_grid(batch, renderer, ctx.data, player, px + 18.0, py + 184.0, mx, my);
    draw_cursor_stack(batch, renderer, cursor, mx, my);
}

fn draw_advancements_screen(
    batch: &mut UiBatch,
    ctx: &HudContext<'_>,
    scroll: usize,
    w: f32,
    h: f32,
    mx: f32,
    my: f32,
) {
    let pw = 520.0;
    let ph = 380.0;
    let px = (w - pw) * 0.5;
    let py = (h - ph) * 0.5;
    batch.rect(0.0, 0.0, w, h, COL_BG_DIM);
    batch.rect(px, py, pw, ph, COL_PANEL_DARK);
    batch.rect_outline(px, py, pw, ph, 2.0, COL_BORDER_DARK);

    let unlocked = ctx.player.map(|p| &p.advancements);
    let total = ctx.data.world.advancements.len();
    let have = unlocked.map(|s| s.len()).unwrap_or(0);
    batch.text_shadow(px + 14.0, py + 12.0, 1.8, &format!("Advancements ({have} / {total})"), COL_YELLOW);

    let mut list: Vec<_> = ctx.data.world.advancements.values().collect();
    list.sort_by(|a, b| a.id.cmp(&b.id));
    for (row, adv) in list.iter().skip(scroll).take(10).enumerate() {
        let ry = py + 42.0 + row as f32 * 28.0;
        let done = unlocked.is_some_and(|s| s.contains(&adv.id));
        let col = if done { COL_GREEN } else { COL_GRAY };
        let title = if adv.title.is_empty() { adv.id.as_str() } else { adv.title.as_str() };
        batch.rect(px + 14.0, ry, pw - 28.0, 24.0, if done { [0.16, 0.30, 0.16, 0.9] } else { [0.12, 0.12, 0.14, 0.85] });
        batch.text_shadow(px + 22.0, ry + 5.0, 1.4, &format!("{} - {}", title, &adv.id), col);
    }
    draw_button(batch, px + 14.0, py + ph - 40.0, 90.0, 28.0, "Up", mx, my);
    draw_button(batch, px + 114.0, py + ph - 40.0, 90.0, 28.0, "Down", mx, my);
    draw_button(batch, px + pw - 110.0, py + ph - 40.0, 96.0, 28.0, "Close", mx, my);
}

fn draw_button(batch: &mut UiBatch, x: f32, y: f32, w: f32, h: f32, label: &str, mx: f32, my: f32) {
    let hov = point_in(mx, my, x, y, w, h);
    let fill = if hov { [0.38, 0.48, 0.68, 0.95] } else { [0.28, 0.28, 0.30, 0.95] };
    batch.rect(x, y, w, h, fill);
    batch.rect_outline(x, y, w, h, 2.0, if hov { COL_WHITE } else { COL_BORDER_DARK });
    centered_text(batch, x + w * 0.5, y + (h - 12.0) * 0.5, 1.6, label, COL_WHITE);
}

fn draw_slot(
    batch: &mut UiBatch,
    renderer: &Renderer,
    data: &GameData,
    x: f32,
    y: f32,
    size: f32,
    stack: &ItemStack,
    hovered: bool,
    active: bool,
) {
    batch.rect(x, y, size, size, COL_SLOT);
    batch.rect_outline(x, y, size, size, if active { 2.5 } else { 1.5 }, if active { COL_WHITE } else { COL_BORDER_DARK });
    if !stack.is_empty() {
        let uv = renderer.item_icon_uv(stack.item);
        let pad = 4.0;
        batch.textured_rect(x + pad, y + pad, size - pad * 2.0, size - pad * 2.0, uv, COL_WHITE);
        if stack.count > 1 {
            batch.text_shadow(x + size - 16.0, y + size - 13.0, 1.3, &format!("{}", stack.count), COL_WHITE);
        }
        let max_d = data.items.get(stack.item).max_durability;
        if max_d > 0 && stack.damage > 0 {
            let pct = 1.0 - (stack.damage as f32 / max_d as f32).clamp(0.0, 1.0);
            batch.rect(x + 3.0, y + size - 4.0, (size - 6.0) * pct, 2.5, COL_GREEN);
        }
    }
    if hovered {
        batch.rect(x + 1.0, y + 1.0, size - 2.0, size - 2.0, COL_SLOT_HOVER);
    }
}

fn centered_text(batch: &mut UiBatch, cx: f32, y: f32, scale: f32, text: &str, color: [f32; 4]) {
    let w = UiBatch::text_width(scale, text);
    batch.text_shadow(cx - w * 0.5, y, scale, text, color);
}

fn point_in(px: f32, py: f32, x: f32, y: f32, w: f32, h: f32) -> bool {
    px >= x && px <= x + w && py >= y && py <= y + h
}

pub fn filtered_creative_items(data: &GameData, query: &str) -> Vec<ItemId> {
    let q = query.trim().to_ascii_lowercase();
    data.items
        .all()
        .iter()
        .filter(|item| item.id != ItemId::AIR && (q.is_empty() || item.name.contains(&q)))
        .map(|item| item.id)
        .collect()
}

pub fn compute_anvil_result(left: &ItemStack, right: &ItemStack) -> ItemStack {
    if left.is_empty() {
        return ItemStack::EMPTY;
    }
    if right.is_empty() {
        return ItemStack::EMPTY;
    }
    let mut out = left.clone();
    if left.item == right.item {
        out.damage = out.damage.saturating_sub(50);
    }
    for (eid, lvl) in &right.enchantments {
        if let Some(ex) = out.enchantments.iter_mut().find(|(k, _)| k == eid) {
            if ex.1 == *lvl {
                ex.1 = (*lvl + 1).min(5);
            } else {
                ex.1 = ex.1.max(*lvl);
            }
        } else {
            out.enchantments.push((eid.clone(), *lvl));
        }
    }
    if out == *left {
        ItemStack::EMPTY
    } else {
        out
    }
}

pub fn handle_slot_click(slot: &mut ItemStack, cursor: &mut ItemStack, right_click: bool, data: &GameData) {
    if cursor.is_empty() {
        if slot.is_empty() {
            return;
        }
        if right_click && slot.count > 1 {
            let take = (slot.count + 1) / 2;
            let mut picked = slot.clone();
            picked.count = take;
            slot.count -= take;
            *cursor = picked;
        } else {
            *cursor = std::mem::take(slot);
        }
    } else if slot.is_empty() {
        if right_click {
            let mut one = cursor.clone();
            one.count = 1;
            *slot = one;
            cursor.count -= 1;
            if cursor.count == 0 {
                *cursor = ItemStack::EMPTY;
            }
        } else {
            *slot = std::mem::take(cursor);
        }
    } else if slot.item == cursor.item && slot.enchantments == cursor.enchantments && slot.damage == cursor.damage {
        let max_s = data.items.get(slot.item).max_stack;
        let put = if right_click { 1 } else { cursor.count };
        let add = (max_s - slot.count).min(put);
        slot.count += add;
        cursor.count -= add;
        if cursor.count == 0 {
            *cursor = ItemStack::EMPTY;
        }
    } else {
        std::mem::swap(slot, cursor);
    }
}

pub fn handle_ui_click(
    screen: &mut Screen,
    world: &mut Option<World>,
    player: &mut Option<Player>,
    entities: &mut Option<EntityManager>,
    data: &GameData,
    w: f32,
    h: f32,
    mx: f32,
    my: f32,
    right_click: bool,
) -> UiAction {
    match screen {
        Screen::Title {
            worlds,
            selected,
            subpage,
            new_name,
            new_seed,
            new_mode,
            mp_addr,
            mp_status: _,
        } => {
            let bw = 300.0;
            let bx = (w - bw) * 0.5;
            match *subpage {
                TitleSubpage::Main => {
                    if point_in(mx, my, bx, 175.0, bw, 38.0) {
                        *subpage = TitleSubpage::Singleplayer;
                    } else if point_in(mx, my, bx, 225.0, bw, 38.0) {
                        *subpage = TitleSubpage::Multiplayer;
                    } else if point_in(mx, my, bx, 275.0, bw, 38.0) {
                        return UiAction::StartWorld {
                            name: "Survival World".into(),
                            seed: 2612026,
                            mode: GameMode::Survival,
                        };
                    } else if point_in(mx, my, bx, 325.0, bw, 38.0) {
                        *subpage = TitleSubpage::Options;
                    }
                }
                TitleSubpage::Singleplayer => {
                    let pw = 520.0;
                    let px = (w - pw) * 0.5;
                    for i in 0..worlds.len().min(5) {
                        let ry = 175.0 + i as f32 * 44.0;
                        if point_in(mx, my, px + 14.0, ry, pw - 28.0, 38.0) {
                            *selected = i;
                        }
                    }
                    if point_in(mx, my, px, 434.0, 165.0, 36.0) {
                        if let Some(ws) = worlds.get(*selected).cloned() {
                            return UiAction::LoadWorld { summary: ws };
                        }
                    } else if point_in(mx, my, px + 177.0, 434.0, 165.0, 36.0) {
                        *subpage = TitleSubpage::CreateWorld;
                    } else if point_in(mx, my, px + 354.0, 434.0, 166.0, 36.0) {
                        *subpage = TitleSubpage::Main;
                    }
                }
                TitleSubpage::CreateWorld => {
                    let pw = 420.0;
                    let px = (w - pw) * 0.5;
                    if point_in(mx, my, px + 20.0, 314.0, pw - 40.0, 34.0) {
                        *new_mode = match *new_mode {
                            GameMode::Survival => GameMode::Creative,
                            GameMode::Creative => GameMode::Adventure,
                            GameMode::Adventure => GameMode::Spectator,
                            GameMode::Spectator => GameMode::Survival,
                        };
                    } else if point_in(mx, my, px + 20.0, 360.0, 180.0, 36.0) {
                        let seed = new_seed.trim().parse::<i64>().unwrap_or(424242);
                        return UiAction::StartWorld {
                            name: new_name.clone(),
                            seed,
                            mode: *new_mode,
                        };
                    } else if point_in(mx, my, px + 220.0, 360.0, 180.0, 36.0) {
                        *subpage = TitleSubpage::Singleplayer;
                    }
                }
                TitleSubpage::Multiplayer => {
                    let pw = 440.0;
                    let px = (w - pw) * 0.5;
                    if point_in(mx, my, px + 20.0, 300.0, 125.0, 36.0) {
                        return UiAction::PingServer(mp_addr.clone());
                    } else if point_in(mx, my, px + 157.0, 300.0, 130.0, 36.0) {
                        return UiAction::ConnectServer(mp_addr.clone());
                    } else if point_in(mx, my, px + 299.0, 300.0, 121.0, 36.0) {
                        *subpage = TitleSubpage::Main;
                    }
                }
                TitleSubpage::Options => {
                    let pw = 360.0;
                    let px = (w - pw) * 0.5;
                    if point_in(mx, my, px + 30.0, 205.0, pw - 60.0, 34.0) {
                        return UiAction::ToggleRenderDist;
                    } else if point_in(mx, my, px + 30.0, 250.0, pw - 60.0, 34.0) {
                        return UiAction::ToggleFov;
                    } else if point_in(mx, my, px + 30.0, 295.0, pw - 60.0, 34.0) {
                        return UiAction::ToggleSound;
                    } else if point_in(mx, my, px + 30.0, 355.0, pw - 60.0, 36.0) {
                        *subpage = TitleSubpage::Main;
                    }
                }
            }
        }
        Screen::Pause => {
            let bw = 260.0;
            let bx = (w - bw) * 0.5;
            if point_in(mx, my, bx, h * 0.34, bw, 36.0) {
                return UiAction::Resume;
            } else if point_in(mx, my, bx, h * 0.43, bw, 36.0) {
                return UiAction::OpenLan;
            } else if point_in(mx, my, bx, h * 0.52, bw, 36.0) {
                return UiAction::ToggleRenderDist;
            } else if point_in(mx, my, bx, h * 0.61, bw, 36.0) {
                return UiAction::SaveAndQuit;
            }
        }
        Screen::Dead => {
            let bw = 240.0;
            let bx = (w - bw) * 0.5;
            if point_in(mx, my, bx, h * 0.48, bw, 38.0) {
                return UiAction::Respawn;
            } else if point_in(mx, my, bx, h * 0.58, bw, 38.0) {
                return UiAction::SaveAndQuit;
            }
        }
        Screen::Inventory { craft, cursor } => {
            if let Some(p) = player.as_mut() {
                let pw = 384.0;
                let ph = 330.0;
                let px = (w - pw) * 0.5;
                let py = (h - ph) * 0.5;
                for i in 0..4 {
                    if point_in(mx, my, px + 14.0, py + 34.0 + i as f32 * 28.0, 26.0, 26.0) {
                        handle_slot_click(&mut p.armor[3 - i], cursor, right_click, data);
                    }
                }
                if point_in(mx, my, px + 60.0, py + 118.0, 32.0, 32.0) {
                    handle_slot_click(&mut p.offhand, cursor, right_click, data);
                }
                for i in 0..4 {
                    let sx = px + 150.0 + (i % 2) as f32 * 40.0;
                    let sy = py + 44.0 + (i / 2) as f32 * 40.0;
                    if point_in(mx, my, sx, sy, 36.0, 36.0) {
                        handle_slot_click(&mut craft[i], cursor, right_click, data);
                    }
                }
                if point_in(mx, my, px + 280.0, py + 64.0, 38.0, 38.0) {
                    craft_take_output(craft, 2, 2, cursor, p, data);
                }
                click_player_inventory_grid(p, cursor, data, px, py + 156.0, mx, my, right_click);
            }
        }
        Screen::CraftingTable { grid, cursor } => {
            if let Some(p) = player.as_mut() {
                let pw = 384.0;
                let ph = 350.0;
                let px = (w - pw) * 0.5;
                let py = (h - ph) * 0.5;
                for i in 0..9 {
                    let sx = px + 44.0 + (i % 3) as f32 * 40.0;
                    let sy = py + 38.0 + (i / 3) as f32 * 40.0;
                    if point_in(mx, my, sx, sy, 36.0, 36.0) {
                        handle_slot_click(&mut grid[i], cursor, right_click, data);
                    }
                }
                if point_in(mx, my, px + 245.0, py + 76.0, 42.0, 42.0) {
                    craft_take_output(grid, 3, 3, cursor, p, data);
                }
                click_player_inventory_grid(p, cursor, data, px, py + 174.0, mx, my, right_click);
            }
        }
        Screen::Creative { search, scroll, cursor } => {
            if let Some(p) = player.as_mut() {
                let pw = 420.0;
                let ph = 390.0;
                let px = (w - pw) * 0.5;
                let py = (h - ph) * 0.5;
                if point_in(mx, my, px + 286.0, py + 28.0, 56.0, 26.0) {
                    *scroll = scroll.saturating_sub(5);
                } else if point_in(mx, my, px + 348.0, py + 28.0, 56.0, 26.0) {
                    *scroll += 5;
                }
                let matches = filtered_creative_items(data, search);
                let start = *scroll * 9;
                for slot_idx in 0..45 {
                    let sx = px + 14.0 + (slot_idx % 9) as f32 * 40.0;
                    let sy = py + 64.0 + (slot_idx / 9) as f32 * 40.0;
                    if point_in(mx, my, sx, sy, 36.0, 36.0) {
                        if let Some(&item) = matches.get(start + slot_idx) {
                            let count = if right_click { 1 } else { data.items.get(item).max_stack };
                            *cursor = ItemStack::new(item, count);
                        }
                    }
                }
                for col in 0..9 {
                    let sx = px + 14.0 + col as f32 * 40.0;
                    let sy = py + 332.0;
                    if point_in(mx, my, sx, sy, 36.0, 36.0) {
                        handle_slot_click(&mut p.inventory[col], cursor, right_click, data);
                    }
                }
            }
        }
        Screen::Furnace { pos, cursor } => {
            if let (Some(w_ref), Some(p)) = (world.as_mut(), player.as_mut()) {
                let pw = 384.0;
                let ph = 330.0;
                let px = (w - pw) * 0.5;
                let py = (h - ph) * 0.5;
                if let Some(BlockEntityData::Furnace { input, fuel, output, .. }) = w_ref.get_block_entity_mut(*pos) {
                    if point_in(mx, my, px + 95.0, py + 40.0, 38.0, 38.0) {
                        handle_slot_click(input, cursor, right_click, data);
                    } else if point_in(mx, my, px + 95.0, py + 106.0, 38.0, 38.0) {
                        handle_slot_click(fuel, cursor, right_click, data);
                    } else if point_in(mx, my, px + 225.0, py + 72.0, 42.0, 42.0) {
                        handle_slot_click(output, cursor, right_click, data);
                    }
                }
                click_player_inventory_grid(p, cursor, data, px, py + 156.0, mx, my, right_click);
            }
        }
        Screen::Chest { pos, cursor } => {
            if let (Some(w_ref), Some(p)) = (world.as_mut(), player.as_mut()) {
                let pw = 384.0;
                let ph = 350.0;
                let px = (w - pw) * 0.5;
                let py = (h - ph) * 0.5;
                if let Some(BlockEntityData::Chest { items } | BlockEntityData::Barrel { items } | BlockEntityData::ShulkerBox { items }) = w_ref.get_block_entity_mut(*pos) {
                    for r in 0..3 {
                        for c in 0..9 {
                            let sx = px + 14.0 + c as f32 * 40.0;
                            let sy = py + 36.0 + r as f32 * 40.0;
                            if point_in(mx, my, sx, sy, 36.0, 36.0) {
                                if let Some(slot) = items.get_mut(r * 9 + c) {
                                    handle_slot_click(slot, cursor, right_click, data);
                                }
                            }
                        }
                    }
                }
                click_player_inventory_grid(p, cursor, data, px, py + 174.0, mx, my, right_click);
            }
        }
        Screen::Enchanting { _pos: _, item, lapis, cursor } => {
            if let Some(p) = player.as_mut() {
                let pw = 384.0;
                let ph = 330.0;
                let px = (w - pw) * 0.5;
                let py = (h - ph) * 0.5;
                if point_in(mx, my, px + 24.0, py + 54.0, 38.0, 38.0) {
                    handle_slot_click(item, cursor, right_click, data);
                } else if point_in(mx, my, px + 70.0, py + 54.0, 38.0, 38.0) {
                    handle_slot_click(lapis, cursor, right_click, data);
                } else {
                    for (tier, (lvl_cost, lapis_cost, power)) in [(1u32, 1u8, 1u8), (3, 2, 2), (6, 3, 3)].into_iter().enumerate() {
                        if point_in(mx, my, px + 128.0, py + 36.0 + tier as f32 * 34.0, 238.0, 28.0)
                            && !item.is_empty()
                            && item.enchantments.is_empty()
                            && (p.xp_level >= lvl_cost || p.game_mode.insta_build())
                            && (lapis.count >= lapis_cost || p.game_mode.insta_build())
                        {
                            if !p.game_mode.insta_build() {
                                p.xp_level -= lvl_cost;
                                lapis.count = lapis.count.saturating_sub(lapis_cost);
                                if lapis.count == 0 {
                                    *lapis = ItemStack::EMPTY;
                                }
                            }
                            let idef = data.items.get(item.item);
                            let eid = if idef.armor_slot.is_some() {
                                "protection"
                            } else if idef.tool_kind == mc_data::ToolKind::Sword {
                                "sharpness"
                            } else {
                                "efficiency"
                            };
                            item.enchantments.push((eid.into(), power));
                            p.advancements.insert("minecraft:story/enchant_item".into());
                        }
                    }
                }
                click_player_inventory_grid(p, cursor, data, px, py + 156.0, mx, my, right_click);
            }
        }
        Screen::Anvil { _pos: _, left, right, cursor } => {
            if let Some(p) = player.as_mut() {
                let pw = 384.0;
                let ph = 330.0;
                let px = (w - pw) * 0.5;
                let py = (h - ph) * 0.5;
                if point_in(mx, my, px + 44.0, py + 62.0, 38.0, 38.0) {
                    handle_slot_click(left, cursor, right_click, data);
                } else if point_in(mx, my, px + 116.0, py + 62.0, 38.0, 38.0) {
                    handle_slot_click(right, cursor, right_click, data);
                } else if point_in(mx, my, px + 220.0, py + 62.0, 42.0, 42.0) && cursor.is_empty() {
                    let res = compute_anvil_result(left, right);
                    if !res.is_empty() {
                        *cursor = res;
                        *left = ItemStack::EMPTY;
                        *right = ItemStack::EMPTY;
                    }
                }
                click_player_inventory_grid(p, cursor, data, px, py + 156.0, mx, my, right_click);
            }
        }
        Screen::BrewingStand { pos, cursor } => {
            if let (Some(w_ref), Some(p)) = (world.as_mut(), player.as_mut()) {
                let pw = 384.0;
                let ph = 330.0;
                let px = (w - pw) * 0.5;
                let py = (h - ph) * 0.5;
                if let Some(BlockEntityData::BrewingStand { bottles, ingredient, fuel, .. }) = w_ref.get_block_entity_mut(*pos) {
                    if point_in(mx, my, px + 44.0, py + 42.0, 36.0, 36.0) {
                        handle_slot_click(fuel, cursor, right_click, data);
                    } else if point_in(mx, my, px + 172.0, py + 36.0, 36.0, 36.0) {
                        handle_slot_click(ingredient, cursor, right_click, data);
                    } else {
                        for i in 0..3 {
                            if point_in(mx, my, px + 116.0 + i as f32 * 56.0, py + 96.0, 36.0, 36.0) {
                                handle_slot_click(&mut bottles[i], cursor, right_click, data);
                            }
                        }
                    }
                }
                click_player_inventory_grid(p, cursor, data, px, py + 156.0, mx, my, right_click);
            }
        }
        Screen::VillagerTrade { entity_id, cursor } => {
            if let (Some(em), Some(p)) = (entities.as_mut(), player.as_mut()) {
                let pw = 420.0;
                let ph = 360.0;
                let px = (w - pw) * 0.5;
                let py = (h - ph) * 0.5;
                if let Some(ent) = em.entities.iter_mut().find(|e| e.id == *entity_id) {
                    for (i, offer) in ent.villager_offers.iter_mut().take(4).enumerate() {
                        let ry = py + 36.0 + i as f32 * 34.0;
                        if point_in(mx, my, px + 14.0, ry, pw - 28.0, 30.0) && offer.uses < offer.max_uses {
                            let have: u32 = p.inventory.iter().filter(|s| s.item == offer.wants.item).map(|s| s.count as u32).sum();
                            if have >= offer.wants.count as u32 {
                                let mut rem = offer.wants.count;
                                for slot in p.inventory.iter_mut() {
                                    if slot.item == offer.wants.item && rem > 0 {
                                        let take = slot.count.min(rem);
                                        slot.count -= take;
                                        rem -= take;
                                        if slot.count == 0 {
                                            *slot = ItemStack::EMPTY;
                                        }
                                    }
                                }
                                p.give_item(offer.gives.clone(), data);
                                offer.uses += 1;
                                p.advancements.insert("minecraft:adventure/trade".into());
                            }
                        }
                    }
                }
                click_player_inventory_grid(p, cursor, data, px + 18.0, py + 184.0, mx, my, right_click);
            }
        }
        Screen::Advancements { scroll } => {
            let pw = 520.0;
            let ph = 380.0;
            let px = (w - pw) * 0.5;
            let py = (h - ph) * 0.5;
            if point_in(mx, my, px + 14.0, py + ph - 40.0, 90.0, 28.0) {
                *scroll = scroll.saturating_sub(10);
            } else if point_in(mx, my, px + 114.0, py + ph - 40.0, 90.0, 28.0) {
                *scroll += 10;
            } else if point_in(mx, my, px + pw - 110.0, py + ph - 40.0, 96.0, 28.0) {
                return UiAction::Resume;
            }
        }
        Screen::InGame | Screen::Chat { .. } => {}
    }

    UiAction::None
}

fn craft_take_output(
    grid: &mut [ItemStack],
    gw: usize,
    gh: usize,
    cursor: &mut ItemStack,
    player: &mut Player,
    data: &GameData,
) {
    let Some(m) = data.recipes.match_crafting(gw, gh, grid) else {
        return;
    };
    let can_stack = cursor.is_empty()
        || (cursor.item == m.result.item
            && cursor.count + m.result.count <= data.items.get(m.result.item).max_stack);
    if !can_stack {
        return;
    }
    if cursor.is_empty() {
        *cursor = m.result.clone();
    } else {
        cursor.count += m.result.count;
    }
    for slot_idx in m.consumed_slots {
        grid[slot_idx].count = grid[slot_idx].count.saturating_sub(1);
        if grid[slot_idx].count == 0 {
            grid[slot_idx] = ItemStack::EMPTY;
        }
    }
    if data.items.get(m.result.item).name == "crafting_table" {
        player.advancements.insert("minecraft:story/root".into());
    }
}

fn click_player_inventory_grid(
    player: &mut Player,
    cursor: &mut ItemStack,
    data: &GameData,
    px: f32,
    py: f32,
    mx: f32,
    my: f32,
    right_click: bool,
) {
    for row in 0..3 {
        for col in 0..9 {
            let sx = px + 14.0 + col as f32 * 40.0;
            let sy = py + row as f32 * 40.0;
            if point_in(mx, my, sx, sy, 36.0, 36.0) {
                handle_slot_click(&mut player.inventory[9 + row * 9 + col], cursor, right_click, data);
            }
        }
    }
    for col in 0..9 {
        let sx = px + 14.0 + col as f32 * 40.0;
        let sy = py + 128.0;
        if point_in(mx, my, sx, sy, 36.0, 36.0) {
            handle_slot_click(&mut player.inventory[col], cursor, right_click, data);
        }
    }
}
