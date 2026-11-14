#!/usr/bin/env python3
"""Reverse-engineer Minecraft 26.1.2 registries, data JSONs, and model texture mappings from Fabric 26.1.2.jar."""

import json
import os
import subprocess
import sys
import zipfile
from pathlib import Path


def run_javap_fields(jar_path: str, class_name: str) -> list[str]:
    out = subprocess.check_output(
        ["javap", "-cp", jar_path, class_name],
        text=True,
        stderr=subprocess.STDOUT,
    )
    fields = []
    for line in out.splitlines():
        line = line.strip()
        if line.startswith("public static final "):
            name = line.rstrip(";").split()[-1]
            fields.append(name.lower())
    return fields


def strip_ns(val: str) -> str:
    return val[len("minecraft:"):] if val.startswith("minecraft:") else val


def resolve_model_textures(model_name: str, models: dict, cache: dict, depth: int = 0) -> dict:
    if depth > 16:
        return {}
    key = strip_ns(model_name)
    if key in cache:
        return cache[key]
    m = models.get(key)
    if not m:
        return {}
    tex = {}
    if "parent" in m:
        tex.update(resolve_model_textures(m["parent"], models, cache, depth + 1))
    if "textures" in m:
        tex.update(m["textures"])
    # Resolve #var references
    resolved = {}
    for k, v in tex.items():
        cur = v
        hops = 0
        while isinstance(cur, str) and cur.startswith("#") and hops < 8:
            cur = tex.get(cur[1:], cur)
            hops += 1
        if isinstance(cur, str) and not cur.startswith("#"):
            resolved[k] = strip_ns(cur)
    cache[key] = resolved
    return resolved


def pick_first_model(bs_obj: dict) -> str | None:
    if "variants" in bs_obj:
        variants = bs_obj["variants"]
        # Prefer default-like variant if available
        for pref in ["", "normal", "snowy=false", "facing=north", "axis=y", "half=bottom,shape=straight,facing=north"]:
            if pref in variants:
                v = variants[pref]
                if isinstance(v, list) and v:
                    return v[0].get("model")
                if isinstance(v, dict):
                    return v.get("model")
        for v in variants.values():
            if isinstance(v, list) and v:
                return v[0].get("model")
            if isinstance(v, dict):
                return v.get("model")
    if "multipart" in bs_obj:
        for part in bs_obj["multipart"]:
            app = part.get("apply")
            if isinstance(app, list) and app:
                return app[0].get("model")
            if isinstance(app, dict):
                return app.get("model")
    return None


def extract_item_model_name(obj: dict) -> str | None:
    if not isinstance(obj, dict):
        return None
    if "model" in obj and isinstance(obj["model"], str):
        return obj["model"]
    if "model" in obj and isinstance(obj["model"], dict):
        r = extract_item_model_name(obj["model"])
        if r:
            return r
    for key in ("on_true", "on_false", "fallback"):
        if key in obj:
            r = extract_item_model_name(obj[key])
            if r:
                return r
    for key in ("cases", "entries", "models"):
        if key in obj and isinstance(obj[key], list):
            for entry in obj[key]:
                r = extract_item_model_name(entry)
                if r:
                    return r
    return None


def main() -> int:
    appdata = os.environ.get("APPDATA", "")
    jar_path = Path(appdata) / ".minecraft" / "versions" / "Fabric 26.1.2" / "Fabric 26.1.2.jar"
    if not jar_path.exists():
        print(f"ERROR: JAR not found at {jar_path}", file=sys.stderr)
        return 1

    out_dir = Path("data/extracted_26_1_2")
    out_dir.mkdir(parents=True, exist_ok=True)

    blocks_cls = run_javap_fields(str(jar_path), "net.minecraft.world.level.block.Blocks")
    items_cls = run_javap_fields(str(jar_path), "net.minecraft.world.item.Items")
    entities_cls = run_javap_fields(str(jar_path), "net.minecraft.world.entity.EntityType")
    effects_cls = run_javap_fields(str(jar_path), "net.minecraft.world.effect.MobEffects")
    particles_cls = run_javap_fields(str(jar_path), "net.minecraft.core.particles.ParticleTypes")

    with zipfile.ZipFile(jar_path, "r") as z:
        names = z.namelist()
        biomes = {}
        enchantments = {}
        damage_types = {}
        dimension_types = {}
        recipes = {}
        loot_tables = {}
        tags = {}
        blockstates = {}
        models = {}
        item_defs = {}
        villager_trades = {}
        trade_sets = {}
        advancements = {}

        for name in names:
            if name.startswith("data/minecraft/worldgen/biome/") and name.endswith(".json"):
                biomes[name[len("data/minecraft/worldgen/biome/"):-5]] = json.loads(z.read(name))
            elif name.startswith("data/minecraft/enchantment/") and name.endswith(".json"):
                enchantments[name[len("data/minecraft/enchantment/"):-5]] = json.loads(z.read(name))
            elif name.startswith("data/minecraft/damage_type/") and name.endswith(".json"):
                damage_types[name[len("data/minecraft/damage_type/"):-5]] = json.loads(z.read(name))
            elif name.startswith("data/minecraft/dimension_type/") and name.endswith(".json"):
                dimension_types[name[len("data/minecraft/dimension_type/"):-5]] = json.loads(z.read(name))
            elif name.startswith("data/minecraft/recipe/") and name.endswith(".json"):
                recipes[name[len("data/minecraft/recipe/"):-5]] = json.loads(z.read(name))
            elif name.startswith("data/minecraft/loot_table/") and name.endswith(".json"):
                loot_tables[name[len("data/minecraft/loot_table/"):-5]] = json.loads(z.read(name))
            elif name.startswith("data/minecraft/tags/") and name.endswith(".json"):
                tags[name[len("data/minecraft/tags/"):-5]] = json.loads(z.read(name))
            elif name.startswith("data/minecraft/villager_trade/") and name.endswith(".json"):
                villager_trades[name[len("data/minecraft/villager_trade/"):-5]] = json.loads(z.read(name))
            elif name.startswith("data/minecraft/trade_set/") and name.endswith(".json"):
                trade_sets[name[len("data/minecraft/trade_set/"):-5]] = json.loads(z.read(name))
            elif name.startswith("data/minecraft/advancement/") and name.endswith(".json"):
                advancements[name[len("data/minecraft/advancement/"):-5]] = json.loads(z.read(name))
            elif name.startswith("assets/minecraft/blockstates/") and name.endswith(".json"):
                blockstates[name[len("assets/minecraft/blockstates/"):-5]] = json.loads(z.read(name))
            elif name.startswith("assets/minecraft/models/") and name.endswith(".json"):
                models[name[len("assets/minecraft/models/"):-5]] = json.loads(z.read(name))
            elif name.startswith("assets/minecraft/items/") and name.endswith(".json"):
                item_defs[name[len("assets/minecraft/items/"):-5]] = json.loads(z.read(name))

    tex_cache = {}
    block_textures = {}
    for bname, bs in blockstates.items():
        mname = pick_first_model(bs)
        if mname:
            t = resolve_model_textures(mname, models, tex_cache)
            top = t.get("top") or t.get("up") or t.get("end") or t.get("all") or t.get("texture") or t.get("cross") or t.get("plant") or t.get("particle")
            bottom = t.get("bottom") or t.get("down") or t.get("end") or t.get("all") or top
            side = t.get("side") or t.get("front") or t.get("north") or t.get("all") or t.get("wall") or top
            front = t.get("front") or side
            block_textures[bname] = {
                "top": top,
                "bottom": bottom,
                "side": side,
                "front": front,
                "overlay": t.get("overlay"),
                "particle": t.get("particle") or side,
            }

    item_textures = {}
    for iname, idef in item_defs.items():
        mname = extract_item_model_name(idef)
        if mname:
            t = resolve_model_textures(mname, models, tex_cache)
            tex = t.get("layer0") or t.get("all") or t.get("top") or t.get("side") or t.get("front") or t.get("particle")
            if tex:
                item_textures[iname] = tex

    manifest = {
        "version": "26.1.2",
        "world_version": 4790,
        "protocol_version": 775,
        "counts": {
            "blocks_class_fields": len(blocks_cls),
            "items_class_fields": len(items_cls),
            "entities_class_fields": len(entities_cls),
            "effects_class_fields": len(effects_cls),
            "particles_class_fields": len(particles_cls),
            "biomes": len(biomes),
            "enchantments": len(enchantments),
            "damage_types": len(damage_types),
            "dimension_types": len(dimension_types),
            "recipes": len(recipes),
            "loot_tables": len(loot_tables),
            "tags": len(tags),
            "villager_trades": len(villager_trades),
            "trade_sets": len(trade_sets),
            "advancements": len(advancements),
            "blockstates": len(blockstates),
            "resolved_block_textures": len(block_textures),
            "resolved_item_textures": len(item_textures),
        },
        "blocks": blocks_cls,
        "items": items_cls,
        "entities": entities_cls,
        "effects": effects_cls,
        "particles": particles_cls,
    }

    (out_dir / "registries.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    (out_dir / "biomes.json").write_text(json.dumps(biomes, indent=2), encoding="utf-8")
    (out_dir / "enchantments.json").write_text(json.dumps(enchantments, indent=2), encoding="utf-8")
    (out_dir / "damage_types.json").write_text(json.dumps(damage_types, indent=2), encoding="utf-8")
    (out_dir / "dimension_types.json").write_text(json.dumps(dimension_types, indent=2), encoding="utf-8")
    (out_dir / "recipes.json").write_text(json.dumps(recipes), encoding="utf-8")
    (out_dir / "loot_tables.json").write_text(json.dumps(loot_tables), encoding="utf-8")
    (out_dir / "tags.json").write_text(json.dumps(tags), encoding="utf-8")
    (out_dir / "villager_trades.json").write_text(json.dumps(villager_trades), encoding="utf-8")
    (out_dir / "trade_sets.json").write_text(json.dumps(trade_sets), encoding="utf-8")
    (out_dir / "advancements.json").write_text(json.dumps(advancements), encoding="utf-8")
    (out_dir / "block_textures.json").write_text(json.dumps(block_textures), encoding="utf-8")
    (out_dir / "item_textures.json").write_text(json.dumps(item_textures), encoding="utf-8")

    print(json.dumps(manifest["counts"], indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
