#!/usr/bin/env python3
"""Generate the pixel-art prompts in docs/art-prompts/ (one self-contained prompt per asset).

The shared style block (palette, grid, outline, light, delivery rules) is defined once here and
copied into every prompt, so all assets come out consistent no matter which prompt is used.
Edit the tables below and re-run:  python3 scripts/art_prompts.py
"""
import os
import textwrap

OUT = os.path.join(os.path.dirname(__file__), '..', 'docs', 'art-prompts')

# ------------------------------------------------------------------------------------------------
# Shared style
# ------------------------------------------------------------------------------------------------

PALETTE = [
    ('Outline', ['#1B1420']),
    ('Deep shadow', ['#2E2433']),
    ('Greys / stone / steel', ['#4A4553', '#6E6977', '#9A96A1', '#C9C6CC', '#F2F0EB']),
    ('Browns / wood / earth', ['#3D2A1E', '#5C3F28', '#7D5A3A', '#A67C52', '#D1A574']),
    ('Human skin', ['#8C5A3C', '#D99A6C', '#F2C8A0']),
    ('Orc skin', ['#2E4A22', '#4F7A30', '#7FA848']),
    ('Nature greens', ['#1F3D24', '#2F5A2E', '#4A7D3A', '#6FA848', '#A8D46A']),
    ('Gold / yellow', ['#7A5A1E', '#C9962E', '#F2D35E']),
    ('Reds / fire', ['#5E1F1F', '#9C2F2A', '#D8503C', '#F28A5E', '#FFD27A']),
    ('Blues / water / ice', ['#1E2E5C', '#2F55A0', '#4A86D8', '#8EC3F2', '#D6EEFA']),
    ('Elf violet', ['#3A2A5C', '#6A4AA8', '#A88AE8']),
    ('Magic teal', ['#1F5E5E', '#2E9A8A', '#7AE8D8']),
    ('TEAM COLOUR (magenta key)', ['#660066', '#990099', '#CC00CC', '#FF00FF']),
]


def palette_table():
    rows = ['| Group | Colours |', '| --- | --- |']
    for name, cols in PALETTE:
        rows.append(f'| {name} | {" ".join("`" + c + "`" for c in cols)} |')
    return '\n'.join(rows)


STYLE = f"""
## Art direction (identical for every asset in this pack)

You are the pixel artist for **Knight Engine**, a hex-based fantasy strategy/RPG engine. The
look is **classic 16-bit fantasy strategy pixel art** (think the SNES/early-PC era of
Warcraft II / Heroes of Might and Magic II): chunky readable shapes, bold silhouettes,
slightly exaggerated proportions (big heads, hands, weapons and shoulders), bright but earthy
colours. Factions are **Humans** (blue-grey stone, steel, gold trim, tidy and fortified),
**Orcs** (green skin, rust, bone, leather, spikes, rough timber) and **Elves** (moss green,
violet, silver, pale wood, curves, leaves, teal magic glow). All designs are **original** — take
inspiration from the genre, never copy characters, buildings or emblems from existing games.

### Pixel rules (must follow exactly)

1. **True pixel art.** Every art pixel is a single solid colour square. No anti-aliasing, no
   blur, no gradients, no soft brushes, no noise, no JPEG artefacts, no painted texture.
2. **Grid:** the engine uses **16 art pixels per world unit**. One hex is **28 px wide and 32 px
   tall** seen from above. A normal person-sized unit is **24–28 px tall**, a hero **28–32 px**.
   Never change this scale.
3. **Palette:** use **only** the colours in the master palette below (plus full transparency).
   Most assets need 12–20 of them. Do not invent new colours, even similar ones.
4. **Outline:** every sprite (character, building, tree, prop) has a **1-pixel outline in
   `#1B1420`** around its outer silhouette. Inner lines use the darkest shade of the local
   material, not the outline colour. Terrain tiles have **no** outline.
5. **Light:** one light from the **upper left**. Each material uses a ramp of 3 tones plus at
   most 1 highlight pixel cluster. Shadows fall to the lower right. No cast shadows on the ground
   (the engine draws those).
6. **Transparency:** pixels are either fully opaque or fully transparent. No semi-transparent
   pixels, no glow halos, no drop shadows.
7. **Dithering:** none on characters and buildings. Terrain may use sparse single-pixel
   dithering between two neighbouring tones only.
8. **Team colour:** parts that should show the player's colour (banners, tabards, cloaks, shield
   faces, roof trims, sashes) are painted **only** with the 4 magenta shades
   (`#660066` `#990099` `#CC00CC` `#FF00FF`). Nothing else may use these colours. The engine
   swaps them for each player's colour.
9. **No text, letters, numbers, UI, borders, frames, signatures or watermarks.**

### Master palette

{palette_table()}

### Delivery rules

- **PNG**. Either at **native size** (the pixel sizes in this prompt) or **scaled up by an exact
  whole number** (×2, ×4, ×8) with nearest-neighbour — every art pixel a perfect N×N block aligned
  to the image's top-left corner. Never a fractional or smoothed scale.
- **Background:** transparent. If your tool cannot output transparency, fill the background
  with flat **`#00FF00`** (it is not in the palette; the engine removes it). Never white, never a
  checkerboard, never a scene.
- **Sprite sheets:** a strict grid, cells exactly the given size, no gaps, no gutters, no
  labels. Unused cells stay empty (transparent).
- **Characters and props face RIGHT** (east, turned slightly towards the viewer), seen from a
  **3/4 top-down view (~50° above the horizon)**. The engine mirrors them for facing left.
- **Feet / foundation on the baseline:** in every cell the lowest opaque pixel touches the
  **bottom row of the cell**, horizontally centred. The character must not drift up/down or
  left/right between frames (except deliberate motion like a lunge).
- **File name:** exactly as given in "Deliverable".
""".strip()

CONSISTENCY = """
## Consistency with the rest of the pack

- Attach the approved **style reference** (`style_reference.png`) {extra}and match it exactly:
  same pixel density, outline colour, palette, light direction, proportions and level of detail.
- If a detail conflicts with the reference, the reference wins; if the reference conflicts with
  the pixel rules, the pixel rules win.
""".strip()

CHECK = """
## Final check before you deliver (fix anything that fails)

1. Image size is exactly the native size (or an exact integer multiple) — cells line up with the
   grid, nothing crosses a cell border.
2. Only master-palette colours (+ transparency / `#00FF00`) are used; no smoothing, no blur.
3. Magenta appears only on team-coloured parts.
4. Outline rule, upper-left light and 3/4 view are respected; sprites face right; feet on the
   bottom row of every cell.
5. No text, frames, shadows, watermarks or extra objects.
""".strip()


STYLE_REF_CONSISTENCY = """
## Consistency with the rest of the pack

This sheet **is** the reference: every other asset will be drawn to match it. Make it the
cleanest possible example of the pixel rules above — exact palette, 1-pixel `#1B1420` outlines,
upper-left light, correct scale (hex 28×32, footman ~26 px tall).
""".strip()


FLAT_CHECK = """
## Final check before you deliver (fix anything that fails)

1. Image size is exactly the native size (or an exact integer multiple) — cells line up with the
   grid, nothing crosses a cell border.
2. Only master-palette colours (+ transparency / `#00FF00`) are used; no smoothing, no blur.
3. Magenta appears only on team-coloured parts.
4. Outline rule and upper-left light are respected; each item is centred in its cell (standing
   objects such as flags and vehicles keep their base on the bottom row).
5. No text, letters, digits, frames, drop shadows, watermarks or extra objects.
""".strip()


def doc(title, body, extra_refs='', consistency=None, check=None):
    extra = f'and {extra_refs} ' if extra_refs else ''
    return '\n\n'.join([
        f'# {title}',
        '> Copy everything below this line into the image AI. Attach the reference images listed in '
        '"Consistency". Deliver the PNG(s) with the exact file name(s).',
        '---',
        STYLE,
        textwrap.dedent(body).strip(),
        consistency or CONSISTENCY.format(extra=extra),
        check or CHECK,
    ]) + '\n'


def write(path, text):
    full = os.path.join(OUT, path)
    os.makedirs(os.path.dirname(full), exist_ok=True)
    with open(full, 'w') as f:
        f.write(text)


# ------------------------------------------------------------------------------------------------
# 00: style reference
# ------------------------------------------------------------------------------------------------

STYLE_REF = """
## This asset: the style reference sheet

A single sheet that defines the look of the whole pack. Every later prompt attaches it.

**Native size: 256 × 128 px** (deliver ×4 = 1024 × 512 if you upscale). Transparent background.
Layout (each box is a region of the sheet, no borders drawn):

```
x:  0        32       64       96       128      160      192      224    256
y:0 +--------+--------+--------+--------+--------+--------+--------+--------+
    | grass  | sand   | rock   | water  | human  | orc    | elf    | human  |
    | tile   | tile   | tile   | tile   | foot-  | grunt  | sentinel| house |
    | 32x32  | 32x32  | 32x32  | 32x32  | man    | 48x48  | 48x48  | 48x64  |
 32 +--------+--------+--------+--------+ 48x48  |        |        |        |
    | oak tree 32x48  | pine tree 32x48 |        |        |        |        |
    |        |        |        |        |        |        |        |        |
 80 +--------+--------+--------+--------+--------+--------+--------+--------+
    | palette swatches: every master-palette colour as an 8x8 square,         |
    | in the palette table order, left to right, wrapping to a second row    |
128 +------------------------------------------------------------------------+
```

- The 4 terrain tiles are seamless top-down textures (see the terrain rules: no outline).
- Footman (human swordsman with kite shield), grunt (green orc with big axe), sentinel (elf
  warrior woman with a crescent glaive) and the house are in their idle pose/state, facing right,
  feet on the bottom of their 48×48 (house 48×64) box, team colour in magenta.
- The two trees are 32×48, roots on the bottom of their box.
- This sheet sets proportions and detail for everything: make it the best example of the style.

## Deliverable

`style_reference.png`
"""

# ------------------------------------------------------------------------------------------------
# Terrain
# ------------------------------------------------------------------------------------------------

TERRAIN = [
    # name, description, cliff description, animated
    ('grass', 'lush green grass: short blade clumps in 2–3 greens, a few single-pixel flowers (white/yellow), tiny darker patches',
     'brown earth with small roots and pebbles, a grass lip on the top 2 rows', False),
    ('meadow', 'grass densely dotted with small flowers (white, yellow, light blue, violet) and a few taller clumps',
     'brown earth with roots, a flowery grass lip on the top 2 rows', False),
    ('forest_floor', 'dark mossy ground, fallen needles and leaves, small roots and mushrooms caps as 1–2 px dots',
     'dark earth with thick roots, moss lip on top', False),
    ('dirt', 'packed brown earth with scattered pebbles and a few cracks', 'layered brown earth and stones', False),
    ('sand', 'pale warm beach sand with gentle ripples and a few shell specks', 'eroded sand layers, lighter at the top', False),
    ('desert', 'orange desert sand with wind ripples', 'orange sandstone strata', False),
    ('rock', 'flat grey stone slabs with cracks and a little grit', 'grey rock face with horizontal strata and cracks', False),
    ('snow', 'soft white snow with gentle drifts, blue-grey shadows in the dips', 'icy grey rock with snow on the ledges and top', False),
    ('ice', 'cracked blue-white ice with light streaks', 'blue ice wall with vertical cracks', False),
    ('swamp', 'murky green-brown bog with algae patches and tiny dark puddles', 'wet dark mud with roots, algae lip on top', False),
    ('farmland', 'ploughed brown furrows running diagonally, a few green sprouts', 'brown earth, tidy', False),
    ('ash', 'burnt black-grey ground, grey ash patches, a few single orange ember pixels', 'charred earth, black and grey', False),
    ('road_cobble', 'grey cobblestones of uniform size in a staggered pattern, dark mortar gaps', 'fitted grey stone blocks (built road embankment)', False),
    ('road_dirt', 'worn light-brown dirt path with two faint wheel ruts running diagonally and pebbles', 'packed earth embankment', False),
    ('elf_moss', 'lush teal-green moss with a few glowing teal spore pixels and tiny violet flowers', 'intertwined pale roots and vines', False),
    ('orc_wasteland', 'cracked red-brown dirt, scattered small bones and scorch marks', 'red-brown clay with bones', False),
    ('seabed', 'sandy lake bottom with small stones and a few weeds (seen through water)', 'wet sand and stones', False),
    ('water', 'clear blue water: soft ripple lines and sparkle pixels, readable as water from above', 'wet dark earth riverbank', True),
    ('lava', 'cooled black crust plates with bright orange/yellow cracks of molten rock', 'black volcanic rock with glowing cracks', True),
]

TERRAIN_STATIC = """
## This asset: terrain "{title}"

Terrain is drawn **straight from above** (orthographic, no perspective) onto the top of each hex
column, and the **cliff** texture is drawn on the vertical sides of raised columns. The engine
repeats the textures across the whole map, so they must be **seamless** and must not contain
any single eye-catching feature that would visibly repeat.

**Look:** {desc}.
**Cliff side:** {cliff}.

Terrain rules: no outline, evenly lit (no light direction on the top tiles), sparse dithering
allowed, 3–5 palette colours for the ground plus up to 3 accent colours.

## Deliverable

`terrain_{name}.png` — **native 128 × 48 px** (×4 = 512 × 192), opaque (no transparency needed).

```
x: 0        32       64       96      128
y:0 +--------+--------+--------+--------+
    | top A  | top B  | top C  | top D  |   4 variants, each 32x32, each seamless on all 4
    |        |        |        |        |   edges AND seamless next to each other in any order
 32 +--------+--------+--------+--------+
    | cliff A| cliff B| cliff C| cliff D|   4 cliff tiles, each 32x16, each seamless left/right
 48 +--------+--------+--------+--------+   and top/bottom, and next to each other
```

- Variants A–D are the same material with different arrangement of details (A plain, D the
  busiest), so a map of random variants looks natural.
- Cliff tiles: vertical face seen from the front, lit from the upper left; strata run
  horizontally.
"""

TERRAIN_ANIM = """
## This asset: animated terrain "{title}"

Terrain is drawn **straight from above** (orthographic, no perspective) onto the top of each hex
column; the engine repeats it across the map, so every frame must be **seamless**. This terrain
is **animated**: 4 frames that loop (1→2→3→4→1) at about 4 frames per second.

**Look:** {desc}.
**Cliff side:** {cliff}.

Terrain rules: no outline, evenly lit, sparse dithering allowed. Motion should be gentle
(ripples shifting 1–2 px per frame, sparkles appearing/disappearing), never a sliding pattern.

## Deliverable

`terrain_{name}.png` — **native 128 × 80 px** (×4 = 512 × 320), opaque.

```
x: 0        32       64       96      128
y:0 +--------+--------+--------+--------+
    | frame1 | frame2 | frame3 | frame4 |   animation of variant A (32x32 each, seamless)
 32 +--------+--------+--------+--------+
    | frame1 | frame2 | frame3 | frame4 |   animation of variant B (different ripple layout)
 64 +--------+--------+--------+--------+
    | cliff A| cliff B| cliff C| cliff D|   32x16 side tiles, seamless (static)
 80 +--------+--------+--------+--------+
```
"""

PROPS = [
    ('trees_oak', 'Round-crowned oak trees (3 shapes, different sizes) and one burnt oak (black charred trunk, no leaves)', 32, 48),
    ('trees_pine', 'Tall dark-green pine/fir trees (3 shapes) and one burnt pine', 32, 48),
    ('trees_birch', 'Slim white-barked birches with light-green crowns (3 shapes) and one burnt birch', 32, 48),
    ('trees_snow', 'Pines covered in snow (3 shapes) and one bare dead snowy tree', 32, 48),
    ('trees_palm', 'Palm trees for beaches (3 shapes) and one burnt palm', 32, 48),
    ('trees_dead', 'Dead leafless twisted trees (3 shapes) and one fallen log', 32, 48),
    ('trees_elf', 'Graceful elven trees with violet and teal leaves and a faint glowing teal pixel here and there (3 shapes) and one withered elven tree', 32, 48),
    ('rocks', 'Four boulders: small grey rock, round mossy rock, large cracked rock, teal glowing crystal cluster', 32, 32),
    ('mountains', 'Four mountain peaks that sit on one hex and tower over it: rocky grey peak, green-sloped peak, snow-capped peak, volcanic black peak with a lava crack', 64, 64),
    ('bushes_flowers', 'Four small props: green bush, berry bush with red berries, reeds for water edges, cluster of wildflowers', 32, 32),
]

PROP_BODY = """
## This asset: nature props "{title}"

Upright props that stand on hexes (drawn as billboards), seen from the **3/4 view (~50°)**, lit
from the upper left, **1-pixel `#1B1420` outline**, roots/base on the bottom row of each cell,
horizontally centred.

**Content:** {desc}.

## Deliverable

`props_{name}.png` — **native {w4} × {h} px** (×4 = {w16} × {h4}), transparent.

```
+--------+--------+--------+--------+
|   1    |   2    |   3    |   4    |   4 cells, each {w}x{h}, one prop per cell
+--------+--------+--------+--------+
```
"""

FIRE = """
## This asset: fire and smoke effects

Animated effect sprites. **No outline** on flames and smoke (they are light), but keep the pixel
rules (solid pixels, palette only — use the Reds / fire and Greys groups).

## Deliverable

`fx_fire.png` — **native 256 × 112 px** (×4 = 1024 × 448), transparent.

```
x: 0    16   32   48   64   80   96  112  128 ... 256
y:0 +----+----+----+----+----+----+----+----+
    | f1 | f2 | f3 | f4 | f5 | f6 | f7 | f8 |      small flame, 8 frames, each 16x24
 24 +----+----+----+----+----+----+----+----+-----------------------------------+
    |  F1   |  F2   |  F3   |  F4   |  F5   |  F6   |  F7   |  F8   |           big flame (burning tree/house),
    |       |       |       |       |       |       |       |       |           8 frames, each 32x48
 72 +-------+-------+-------+-------+-------+-------+-------+-------+
    | s1 | s2 | s3 | s4 | e1 | e2 | e3 | e4 |  ...                              smoke puff 4 frames 16x16 (grows and
 88 +----+----+----+----+----+----+----+----+                                    fades by losing pixels), embers 4 frames
    |  scorch mark 32x16 (flat, top-down)  |  burnt rubble 32x16  |               8x8-ish centred in 16x16 cells
112 +------------------------------------------------------------+
```

- Flames: base on the bottom row of the cell, bright yellow core (`#FFD27A`), orange middle,
  red tips, flickering shape that loops smoothly from frame 8 back to frame 1.
- Smoke: grey puffs. Embers: 1–3 orange/yellow pixels drifting up.
- Scorch mark and rubble are top-down ground decals (no outline).
"""

# ------------------------------------------------------------------------------------------------
# Characters
# ------------------------------------------------------------------------------------------------

CHAR_SHEET = """
## Deliverable

`{file}.png` — **native {sw} × {sh} px** (×4 = {sw4} × {sh4}), transparent.
A sprite sheet of **{cw} × {ch} px cells**, 6 columns × 6 rows, one animation per row, frames left
to right. Unused cells stay transparent.

```
         col1    col2    col3    col4    col5    col6
row 1   idle1   idle2   idle3   idle4   -       -        IDLE   (breathing, weapon sway)  loop
row 2   walk1   walk2   walk3   walk4   walk5   walk6    WALK   (full stride cycle)       loop
row 3   atk1    atk2    atk3    atk4    atk5    atk6     ATTACK {attack}
row 4   hurt1   hurt2   -       -       -       -        HURT   (recoil, flinch back)
row 5   die1    die2    die3    die4    die5    die6     DEATH  (falls, last frame lies on the ground)
row 6   {special_row}
```

Animation notes:
- The character stays in place (walk is on the spot); feet on the **bottom row of every cell**,
  body centred in the cell (a mount may extend towards the cell edges).
- Keep the silhouette, colours and size identical across frames; only the moving parts change.
- Death frame 6 is the corpse lying on the ground, still inside the cell, baseline unchanged.
- {size_note}

**If you cannot produce the whole sheet in one image**, deliver one image per row instead, each a
single row of {cw} × {ch} cells (e.g. 6 cells = {sw} × {ch} px native), named
`{file}_idle.png`, `{file}_walk.png`, `{file}_attack.png`, `{file}_hurt.png`, `{file}_death.png`,
`{file}_{special}.png`. Same cell size, same baseline, same character in every file.
"""

SPECIAL_ROWS = {
    'melee': 'atk2-1  atk2-2  atk2-3  atk2-4  atk2-5  atk2-6   ATTACK 2 (a second, heavier swing / overhead chop)',
    'ranged': 'shot1   shot2   shot3   shot4   shot5   shot6    SHOOT  (draw/aim → release; projectile NOT included)',
    'caster': 'cast1   cast2   cast3   cast4   cast5   cast6    CAST   (raise hands/staff, glow builds in teal/violet, release)',
    'worker': 'work1   work2   work3   work4   -       -        WORK   (chopping / mining swing, loop)',
}

ATTACK_TEXT = {
    'melee': '(wind-up → strike → recover)',
    'ranged': '(melee jab with the bow/weapon if enemies are adjacent)',
    'caster': '(staff swing / melee strike)',
    'worker': '(small tool swing used to fight)',
}

HEROES = [
    # file, faction, title, description, kind, cell size
    ('hero_human_paladin', 'Humans', 'Paladin (hero)',
     'a heavily armoured holy knight on foot: plate armour in steel greys with gold trim, a tabard and short cape in team magenta, a large glowing warhammer (small gold/white glow pixels on the head), square jaw, short blond hair, no helmet',
     'melee', 48),
    ('hero_human_archmage', 'Humans', 'Archmage (hero)',
     'an old wizard with a long white beard, pointed hood, flowing robe in blue with team-magenta trim, a tall staff topped with a floating teal crystal',
     'caster', 48),
    ('hero_human_ranger', 'Humans', 'Ranger Lord (hero)',
     'a hooded master archer: green hooded cloak with team-magenta lining, leather armour, a large ornate longbow, quiver on the back',
     'ranged', 48),
    ('hero_orc_warlord', 'Orcs', 'Warlord (hero)',
     'a lean, fast orc blademaster: green skin, black topknot, bare muscular chest with scars, a long curved two-handed blade, team-magenta sash and loincloth, bone armband',
     'melee', 48),
    ('hero_orc_farseer', 'Orcs', 'Farseer (hero, mounted)',
     'an orc shaman riding a large grey wolf: wolf-skull helmet, fur mantle, glowing blue-teal eyes, a staff with feathers and a lightning-blue gem, team-magenta cloth on the wolf harness',
     'caster', 64),
    ('hero_orc_chieftain', 'Orcs', 'Blood Chieftain (hero)',
     'a massive orc war chief: huge double-bladed axe, spiked iron pauldrons, war drum strapped to the back, tusks with gold rings, team-magenta war banner on a pole attached to the back',
     'melee', 48),
    ('hero_elf_priestess', 'Elves', 'Moon Priestess (hero, mounted)',
     'an elven archer priestess riding a great white panther: silver armour, long white hair, crescent-moon headpiece, elegant bow, team-magenta cape',
     'ranged', 64),
    ('hero_elf_warden', 'Elves', 'Warden (hero)',
     'a hooded elven assassin: dark violet hooded cloak, silver mask over the lower face, a crescent-shaped bladed weapon, team-magenta sash',
     'melee', 48),
    ('hero_elf_keeper', 'Elves', 'Keeper of the Grove (hero)',
     'a tall antlered nature sage: pale green skin, deer-like lower body (four hooves), leafy mantle, staff of living wood with a teal glow, team-magenta leaf-cloth sash',
     'caster', 64),
]

UNITS = [
    # Humans
    ('unit_human_peasant', 'Humans', 'Peasant (worker)', 'a simple villager with a straw hat, brown tunic with a team-magenta patch, carrying a pickaxe', 'worker', 48),
    ('unit_human_footman', 'Humans', 'Footman', 'a sturdy swordsman in chainmail and a steel helmet, kite shield with a team-magenta face and a gold emblem shape (no letters), short broadsword', 'melee', 48),
    ('unit_human_archer', 'Humans', 'Archer', 'a light archer in a leather jerkin and team-magenta hood, longbow, quiver', 'ranged', 48),
    ('unit_human_rifleman', 'Humans', 'Rifleman', 'a short stocky bearded gunner (dwarf-like) with a long musket, leather cap, team-magenta scarf', 'ranged', 48),
    ('unit_human_knight', 'Humans', 'Knight (mounted)', 'an armoured knight on a barded white horse, lance and shield, team-magenta horse caparison', 'melee', 64),
    ('unit_human_priest', 'Humans', 'Priest', 'a robed healer in white and gold with a team-magenta stole, holding a short staff with a gold sun symbol', 'caster', 48),
    ('unit_human_sorceress', 'Humans', 'Sorceress', 'a young mage woman in a long robe with a team-magenta collar, pointed hat, glowing hand', 'caster', 48),
    ('unit_human_griffon', 'Humans', 'Griffon Rider (flying)', 'a rider on a griffon (eagle head, lion body, wings spread), hammer in hand, team-magenta saddle cloth. It flies: keep a 6-pixel gap between the creature and the bottom row, but its lowest point is still centred', 'melee', 64),
    # Orcs
    ('unit_orc_peon', 'Orcs', 'Peon (worker)', 'a hunched orc labourer with a woodcutting axe, loincloth in team magenta', 'worker', 48),
    ('unit_orc_grunt', 'Orcs', 'Grunt', 'a broad green orc warrior with a huge axe, spiked shoulder pads, team-magenta loincloth and armband', 'melee', 48),
    ('unit_orc_axethrower', 'Orcs', 'Axe Thrower', 'a tall thin troll-like thrower with long tusks and blue-grey skin, mohawk, throwing axes on the belt, team-magenta headband', 'ranged', 48),
    ('unit_orc_wolfrider', 'Orcs', 'Wolf Rider (mounted)', 'an orc raider on a big grey wolf, spear and net, team-magenta cloth on the saddle', 'melee', 64),
    ('unit_orc_shaman', 'Orcs', 'Shaman', 'an old orc with a bone staff, feathers and beads, crackling blue lightning on the hand, team-magenta shawl', 'caster', 48),
    ('unit_orc_witchdoctor', 'Orcs', 'Witch Doctor', 'a masked troll-like mystic with a totem staff topped by a skull, team-magenta mask paint', 'caster', 48),
    ('unit_orc_ogre', 'Orcs', 'Ogre Brute (large)', 'a huge ogre with a spiked club, small head, big belly, team-magenta loincloth; about 40 px tall', 'melee', 64),
    ('unit_orc_wyvern', 'Orcs', 'Wyvern Rider (flying)', 'an orc on a green-brown wyvern with bat-like wings, spear, team-magenta saddle banner. It flies: keep a 6-pixel gap below', 'ranged', 64),
    # Elves
    ('unit_elf_wisp', 'Elves', 'Wisp (worker)', 'a floating spirit: a round teal-white light core with small orbiting violet sparks; hovers 4 px above the bottom row (keep its lowest pixel centred); its "work" pulses brighter', 'worker', 48),
    ('unit_elf_sentinel', 'Elves', 'Sentinel', 'an elf warrior woman with a three-bladed crescent glaive, silver armour, long violet hair, team-magenta cloak', 'melee', 48),
    ('unit_elf_archer', 'Elves', 'Archer', 'a hooded elf archer with an elegant recurve bow, leaf-pattern leather, team-magenta hood lining', 'ranged', 48),
    ('unit_elf_dryad', 'Elves', 'Dryad (mounted form)', 'a deer-bodied forest spirit (centaur-like, deer lower body), green skin, leaves in the hair, spear, team-magenta leaf sash', 'ranged', 64),
    ('unit_elf_bear', 'Elves', 'Druid Bear', 'a druid transformed into a big brown bear with glowing teal markings and a team-magenta collar of leaves', 'melee', 64),
    ('unit_elf_treant', 'Elves', 'Treant (large)', 'a walking tree giant with root legs and branch arms, bark face, moss and violet flowers, team-magenta ribbon tied on a branch; about 44 px tall', 'melee', 64),
    ('unit_elf_hippogryph', 'Elves', 'Hippogryph Rider (flying)', 'an elf archer riding a hippogryph (eagle front, horse back, wings), team-magenta saddle cloth. It flies: keep a 6-pixel gap below', 'ranged', 64),
    # Monsters (neutral, no team colour)
    ('monster_wolf', 'Neutral monsters', 'Wolf', 'a grey wolf with yellow eyes, raised hackles', 'melee', 48),
    ('monster_bear', 'Neutral monsters', 'Bear', 'a big brown bear', 'melee', 48),
    ('monster_spider', 'Neutral monsters', 'Giant Spider', 'a black and dark-violet giant spider with red eyes', 'melee', 48),
    ('monster_skeleton', 'Neutral monsters', 'Skeleton Warrior', 'a skeleton with a rusty sword and a broken round shield', 'melee', 48),
    ('monster_skeleton_archer', 'Neutral monsters', 'Skeleton Archer', 'a skeleton with a cracked bow', 'ranged', 48),
    ('monster_zombie', 'Neutral monsters', 'Zombie', 'a shambling grey-green zombie in rags', 'melee', 48),
    ('monster_troll', 'Neutral monsters', 'Forest Troll', 'a tall blue-green troll with a club and tusks', 'melee', 48),
    ('monster_bandit', 'Neutral monsters', 'Bandit', 'a masked human bandit with a dagger and brown hood', 'melee', 48),
    ('monster_harpy', 'Neutral monsters', 'Harpy', 'a winged bird-woman with talons, flying: keep a 6-pixel gap below', 'melee', 48),
    ('monster_fishman', 'Neutral monsters', 'Fishman', 'an original frog-fish creature with a spear, fins and big eyes', 'melee', 48),
    ('monster_slime', 'Neutral monsters', 'Slime', 'a green blob with a face that squashes and stretches', 'melee', 48),
    ('monster_golem', 'Neutral monsters', 'Rock Golem (large)', 'a hulking golem of grey boulders with teal glowing cracks, about 44 px tall', 'melee', 64),
    ('monster_dragon', 'Neutral monsters', 'Young Dragon (boss)', 'a red dragon with wings half-spread, breathes fire in the attack row (fire drawn with the fire palette), about 56 px tall', 'melee', 96),
]

CHAR_BODY = """
## This asset: {group} — {title}

**Faction:** {faction}. **Design:** {desc}.

{faction_note}
"""

FACTION_NOTES = {
    'Humans': 'Human style: disciplined, clean armour in steel greys, blue and cream cloth, gold trim; team colour on tabards/shields/cloaks.',
    'Orcs': 'Orc style: green skin (orc skin ramp), rusty iron, bone, leather and fur, spikes, tusks, crude but sturdy gear; team colour on loincloths, sashes, banners.',
    'Elves': 'Elf style: slender and graceful, silver and pale wood, moss green and violet cloth, crescent and leaf shapes, small teal glow accents; team colour on cloaks, sashes, linings.',
    'Neutral monsters': 'Neutral creature: **no team colour at all** (no magenta).',
}

# ------------------------------------------------------------------------------------------------
# Structures
# ------------------------------------------------------------------------------------------------

STRUCT_SIZES = {
    'small': (48, 64, 'one hex: its base (foundation) is about 28–32 px wide'),
    'medium': (64, 64, 'one hex, a bit bigger: base about 32–40 px wide'),
    'large': (96, 96, 'a landmark covering about 3 hexes: base about 60–70 px wide'),
}

STRUCTURES = [
    # file, faction, title, size, description, animation idea
    ('building_human_townhall', 'Humans', 'Town Hall', 'large', 'a stone keep with a central tower, blue slate roofs, gold spires, team-magenta banners, wooden gate', 'banners gently waving'),
    ('building_human_castle', 'Humans', 'Castle (upgraded town hall)', 'large', 'a bigger fortified castle with three towers and walls, blue roofs, gold trim, many team-magenta banners', 'banners waving'),
    ('building_human_house', 'Humans', 'House', 'small', 'a cosy timber-framed cottage with a red-brown tiled roof, chimney, team-magenta door awning', 'chimney smoke puffs'),
    ('building_human_farm', 'Humans', 'Farm', 'medium', 'a farmhouse with a small fenced field of wheat and a haystack, team-magenta flag on the roof', 'wheat swaying'),
    ('building_human_barracks', 'Humans', 'Barracks', 'medium', 'a sturdy stone hall with a training yard post, crossed swords sign (no letters), team-magenta banners', 'banner waving'),
    ('building_human_blacksmith', 'Humans', 'Blacksmith', 'medium', 'a stone forge with a glowing furnace, anvil outside, chimney', 'furnace glow flicker and smoke'),
    ('building_human_lumbermill', 'Humans', 'Lumber Mill', 'medium', 'a wooden mill with a saw wheel and stacked logs', 'saw wheel turning'),
    ('building_human_church', 'Humans', 'Church', 'medium', 'a white stone chapel with a bell tower and stained-glass window (coloured blocks, no images of people), team-magenta banner', 'bell swinging'),
    ('building_human_magetower', 'Humans', 'Mage Tower', 'medium', 'a tall slender blue-roofed tower with floating teal crystals around the top', 'crystals bobbing and glowing'),
    ('building_human_guardtower', 'Humans', 'Guard Tower', 'small', 'a round stone watchtower with a crenellated top and a team-magenta pennant', 'pennant waving'),
    ('building_human_windmill', 'Humans', 'Windmill', 'medium', 'a stone windmill with four cloth sails', 'sails rotating (4 frames = a quarter turn so it loops)'),
    ('building_orc_greathall', 'Orcs', 'Great Hall', 'large', 'a huge hall of timber and hides with tusk decorations, spiked roof, braziers, team-magenta war banners', 'brazier fires flickering'),
    ('building_orc_stronghold', 'Orcs', 'Stronghold (upgraded great hall)', 'large', 'a bigger fortress of iron, stone and timber with spiked walls and skull totems, team-magenta banners', 'fires flickering'),
    ('building_orc_hut', 'Orcs', 'Hut', 'small', 'a round hide tent on a timber frame with tusk horns on top and a team-magenta cloth door', 'smoke from the top hole'),
    ('building_orc_burrow', 'Orcs', 'Burrow', 'small', 'a fortified earth mound with a spiked wooden roof and arrow slits, team-magenta pennant', 'pennant waving'),
    ('building_orc_barracks', 'Orcs', 'Barracks', 'medium', 'a rough timber longhouse with weapon racks and a skull on the door, team-magenta banner', 'banner waving'),
    ('building_orc_warmill', 'Orcs', 'War Mill', 'medium', 'a crude sawmill and workshop with a spiked wheel and log piles', 'wheel turning'),
    ('building_orc_forge', 'Orcs', 'Forge', 'medium', 'a stone and iron forge with a big bellows and glowing coals', 'coals glowing, smoke'),
    ('building_orc_spiritlodge', 'Orcs', 'Spirit Lodge', 'medium', 'a hide lodge with spirit totems, feathers and blue-teal glowing runes (shapes, not letters)', 'runes pulsing'),
    ('building_orc_watchtower', 'Orcs', 'Watch Tower', 'small', 'a tall timber tower with a spiked lookout and a team-magenta pennant', 'pennant waving'),
    ('building_elf_treeoflife', 'Elves', 'Tree of Life', 'large', 'a gigantic living tree whose trunk forms a hall with a glowing doorway, violet-teal canopy, team-magenta ribbons in the branches', 'leaves rustling, doorway glow pulsing'),
    ('building_elf_moonwell', 'Elves', 'Moon Well', 'small', 'a ring of pale carved stones around a pool of glowing teal water', 'water glow pulsing'),
    ('building_elf_house', 'Elves', 'Elven House', 'small', 'an elegant house of pale wood with a curved leaf-shaped violet roof, team-magenta curtain', 'lantern glow flicker'),
    ('building_elf_ancientofwar', 'Elves', 'Ancient of War', 'large', 'a huge sentient tree with a face in the bark, branch arms and root legs (it is a building), team-magenta ribbons', 'eyes blinking, branches swaying'),
    ('building_elf_huntershall', 'Elves', "Hunter's Hall", 'medium', 'a long pale-wood hall with antler decorations and hanging bows, team-magenta banner', 'banner waving'),
    ('building_elf_sanctum', 'Elves', 'Arcane Sanctum', 'medium', 'a domed pale-stone temple with a floating violet crystal above the roof', 'crystal floating and glowing'),
    ('building_elf_tower', 'Elves', 'Ancient Tower', 'small', 'a slim living tree tower with a glowing teal eye-like window', 'glow pulsing'),
    ('building_neutral_goldmine', 'Neutral', 'Gold Mine', 'medium', 'a mine entrance in a rocky outcrop with wooden supports, a cart of gold nuggets, no team colour', 'gold glints sparkling'),
    ('building_neutral_sawmill', 'Neutral', 'Sawmill', 'medium', 'a wooden sawmill with a water wheel and log pile, no team colour — plus a team-magenta flag that appears only when captured: put the flag ONLY in row 2 frames', 'water wheel turning'),
    ('building_neutral_quarry', 'Neutral', 'Stone Quarry', 'medium', 'cut stone blocks, a wooden crane and a rock face', 'crane rope swaying'),
    ('building_neutral_tavern', 'Neutral', 'Tavern', 'medium', 'a two-storey inn with a hanging sign showing a mug shape (no letters), warm window light', 'window light flicker, chimney smoke'),
    ('building_neutral_market', 'Neutral', 'Market', 'medium', 'market stalls with striped awnings (red/cream) and crates of goods', 'awnings flapping'),
    ('building_neutral_shrine', 'Neutral', 'Shrine', 'small', 'a small stone shrine with a glowing teal orb on a pedestal', 'orb glow pulsing'),
    ('building_neutral_portal', 'Neutral', 'Portal', 'medium', 'a stone archway with a swirling violet-teal portal', 'swirl rotating'),
    ('building_neutral_village', 'Neutral', 'Village Houses', 'medium', 'a cluster of two small thatched cottages with a fence', 'chimney smoke'),
    ('building_neutral_ruins', 'Neutral', 'Ruins', 'medium', 'crumbling stone walls and a broken column overgrown with ivy', 'none (repeat the same frame)'),
    ('building_neutral_chest', 'Neutral', 'Treasure Chest', 'small', 'a wooden chest with gold bands; row 1 closed with a glint, row 2 cell 1 open and empty, cell 2 open with gold', 'glint sparkle on the lid'),
    ('building_neutral_well', 'Neutral', 'Well', 'small', 'a stone well with a wooden roof and a bucket', 'bucket swinging'),
    ('building_neutral_station', 'Neutral', 'Train Station', 'medium', 'a small brick railway station with a platform canopy on iron posts, a clock shape (no digits) above the door, benches and luggage crates; the platform edge runs left to right along the front', 'steam wisps drifting past the canopy, window light flicker'),
]

STRUCT_BODY = """
## This asset: {faction} building — {title}

**Design:** {desc}.

{faction_note}

Buildings are drawn from the **3/4 view (~50°)**, lit from the upper left, with the **1-pixel
`#1B1420` outline**. The front of the building faces the viewer and slightly right. The
foundation sits on the **bottom row of each cell**, horizontally centred. Size: {size_note}.

## Deliverable

`{file}.png` — **native {sw} × {sh} px** (×4 = {sw4} × {sh4}), transparent.
A sheet of **{cw} × {ch} px cells**, 4 columns × 2 rows:

```
         col1         col2          col3         col4
row 1    anim 1       anim 2        anim 3       anim 4       NORMAL, looping idle animation: {anim}
row 2    building     damaged       ruined       -            STATES
         (under       (cracks,      (collapsed,
         construction: small fires  no team
         scaffolding, drawn with    colour)
         half-built)  fire palette)
```

- The 4 animation frames differ only in the animated part; the building itself never moves.
- The construction, damaged and ruined states keep the same footprint and baseline.

**If you cannot produce the sheet in one image**, deliver two images of 4 cells each:
`{file}_anim.png` (row 1) and `{file}_states.png` (row 2), same cell size and baseline.
"""


# ------------------------------------------------------------------------------------------------
# Extras: everything the engine's demos still draw with placeholder art
# ------------------------------------------------------------------------------------------------

ICON_RULES = """
Icons are small, bold and readable at native size **and** at ×2 on a dark UI panel. Draw each as
a chunky object seen slightly from above (3/4), lit from the upper left, with the **1-pixel
`#1B1420` outline** around the silhouette and 1–2 pixels of transparent margin inside the cell.
No background, no frame, no text.
"""

EXTRAS = [
    ('icons/icons_resources', 'Icons · resources', 'resources', f"""
## This asset: resource icons

Used in the resource bar of the UI **and** as pickups lying on the map (drawn as small
billboards on a hex), so they must also read on grass, sand and snow.
{{rules}}
## Deliverable

`icons_resources.png` — **native 128 × 16 px** (×4 = 512 × 64), transparent. 8 cells of 16 × 16:

```
+------+------+------+------+------+------+------+------+
| gold | wood | ore  |stone | food | mana | gems |people|
+------+------+------+------+------+------+------+------+
```

1. **gold:** a small pile of gold coins (Gold / yellow group, one `#F2D35E` glint).
2. **wood:** three stacked logs, cut ends showing rings.
3. **ore:** a lump of dark iron ore with rusty-orange and grey flecks.
4. **stone:** two cut grey stone blocks.
5. **food:** a wheat sheaf with a round bread loaf.
6. **mana:** a violet-teal crystal with a glow pixel.
7. **gems:** three cut gems (red, blue, teal).
8. **people:** two small head-and-shoulders silhouettes (population / army size).
"""),
    ('icons/icons_spells', 'Icons · spells and abilities', 'spells', f"""
## This asset: spell and ability icons

Shown on action buttons (spells, abilities) at 16 × 16 native, usually drawn at ×2 or ×3.
Each icon sits on a **square rounded badge** 14 × 14 px, centred, with the outline around the
badge; the symbol inside is bright and simple. Badge colour by school: damage = Reds, arcane =
Elf violet, nature/holy = Magic teal or Gold, control = Blues.
{{rules}}
## Deliverable

`icons_spells.png` — **native 128 × 32 px** (×4 = 512 × 128), transparent. 16 cells of 16 × 16:

```
row 1: lightning | magic arrow | fireball | frost bolt | heal   | bless  | shield | haste
row 2: slow      | poison      | fear     | summon     | teleport | sight | rally | build
```

- lightning: a jagged yellow bolt; magic arrow: a violet glowing arrow; fireball: an orange ball
  with a flame tail; frost bolt: a pale-blue ice shard; heal: a teal plus with sparkles; bless:
  a golden halo over a small sword; shield: a steel kite shield with a teal glow; haste: a
  winged boot; slow: a snail shell spiral; poison: a green drop with a skull shape (no text);
  fear: a violet screaming mask; summon: a teal circle with rising motes; teleport: a violet
  swirl; sight: an open eye; rally: a banner on a pole (team-magenta cloth); build: a hammer
  crossing a saw.
"""),
    ('icons/emotes', 'Icons · emotes', 'emotes', f"""
## This asset: chat emotes

Shown in speech bubbles above heroes in multiplayer (at ×2), often many at once, so each must be
recognisable from its silhouette and main colour alone.
{{rules}}
## Deliverable

`emotes.png` — **native 128 × 32 px** (×4 = 512 × 128), transparent. 16 cells of 16 × 16.
Row 1 is the emote; row 2 is a **second frame** of the same emote for a 2-frame bounce/blink
animation (same position and size, only the animated detail changes).

```
        col1   col2   col3   col4   col5   col6   col7   col8
row 1   love   smile  laugh  wow    what   angry  sleep  cheer
row 2   (same eight, second frame)
```

- love: a red heart (second frame: slightly bigger, a sparkle); smile: a yellow round face
  smiling (blink); laugh: face with closed eyes and open mouth (tears of joy on frame 2); wow:
  a yellow exclamation mark (tilted on frame 2); what: a pale-blue question mark (tilted);
  angry: a red face with frowning brows (a puff of steam on frame 2); sleep: pale-blue "Z Z"
  shapes drawn as zig-zag strokes, **not letters** (shifted up 1 px); cheer: a yellow star
  burst / raised fist (sparkles on frame 2).
"""),
    ('fx/fx_combat', 'Effects · combat and magic', 'combat effects', """
## This asset: projectiles, impacts and spell effects

Animated effect sprites for battles. Projectiles are drawn **pointing right** (the engine rotates
and flips them), centred in their cell. Physical objects (arrow, axe, bullet) have the
`#1B1420` outline; light effects (magic, fire, lightning, sparks, heal) have **no outline** and
use the Reds / fire, Magic teal, Elf violet and Gold groups. Frames loop unless noted.

## Deliverable

`fx_combat.png` — **native 256 × 80 px** (×4 = 1024 × 320), transparent.

```
x: 0    16   32   48   64   80   96  112  128  144  160  176  192  208  224  240  256
y:0 +----+----+----+----+----+----+----+----+----+----+----+----+----+----+----+----+
    |arrw|bllt|axe1|axe2|axe3|axe4|mag1|mag2|mag3|mag4|fir1|fir2|fir3|fir4|jav |rock|  16x16 projectiles
 16 +----+----+----+----+----+----+----+----+----+----+----+----+----+----+----+----+
    |spk1|spk2|spk3|spk4|mzl1|mzl2|mzl3|hea1|hea2|hea3|hea4|dst1|dst2|dst3|dst4|    |  16x16 impacts
 32 +----+----+----+----+-------+-------+-------+-------+-------+-------+
    |lgt1|lgt2|lgt3|lgt4|  ex1  |  ex2  |  ex3  |  ex4  |  ex5  |  ex6  |  lightning 16x48 ×4, explosion 32x32 ×6
    |    |    |    |    +-------+-------+-------+-------+-------+-------+  (explosion cells are the top
    |    |    |    |    |           (empty, transparent)                |   32 px; the rest stays empty)
 80 +----+----+----+----+-----------------------------------------------+
```

- **arrw** arrow (wood shaft, steel head, white fletching), **bllt** musket ball with a short
  motion streak, **axe1–4** thrown axe spinning a quarter turn per frame, **mag1–4** violet-teal
  magic bolt pulsing, **fir1–4** fireball with a flickering tail, **jav** thrown spear/javelin,
  **rock** a thrown boulder (ogre, catapult).
- **spk1–4** melee hit spark (white-yellow star that bursts and fades, plays once), **mzl1–3**
  muzzle flash (plays once), **hea1–4** heal sparkles rising (teal and white), **dst1–4** dust
  puff at the feet (plays once).
- **lgt1–4** lightning strike from the top of its 16 × 48 cell down to the bottom row (the
  ground), plays once; **ex1–6** explosion: flash, fireball, smoke ring, fading embers (plays once).
"""),
    ('fx/flags', 'Props · flags and markers', 'flags', """
## This asset: flags, banners and map markers

Stand on hexes as billboards (base on the **bottom row** of the cell). Team parts in the four
magenta shades so the engine recolours them per player. Waving animations loop (4 frames).

## Deliverable

`flags.png` — **native 128 × 64 px** (×4 = 512 × 256), transparent. 16 cells of 16 × 32:

```
        col1-4                      col5-8
row 1   team flag waving ×4         hero standard waving ×4
row 2   neutral flag waving ×4      destination marker bobbing ×4
```

- **team flag:** a wooden pole with a small magenta pennant (ownership of mines, towns).
- **hero standard:** a taller pole with a square magenta banner and a gold finial, carried next
  to heroes on the adventure map.
- **neutral flag:** the team flag in greys/white (unclaimed).
- **destination marker:** a gold downward arrow above a small ring on the ground, bobbing
  1–2 px up and down (where a unit is going).
"""),
    ('vehicles/vehicles_rail', 'Vehicles · railway', 'rail vehicles', """
## This asset: railway locomotive and wagons

For transport games (routes between towns). Drawn **from the side, facing right**, in the same
3/4 view as buildings (you see the right side and a little of the top), lit from the upper left,
with the `#1B1420` outline. Wheels sit on the **bottom row**. Scale: a wagon is about 2 hexes
long (≈ 40 px). Team colour on the locomotive's stripe and the wagons' side panels.

## Deliverable

`vehicles_rail.png` — **native 192 × 64 px** (×4 = 768 × 256), transparent. 8 cells of 48 × 32:

```
        col1          col2          col3          col4
row 1   loco 1        loco 2        loco 3        loco 4       steam locomotive: wheels turning a
                                                               quarter turn per frame, chimney puff
row 2   coal tender   passenger     cargo wagon   ore wagon    static wagons
                      car           (crates)      (ore heap)
```

- Locomotive: black-iron boiler, red cab, brass details (Gold group), magenta stripe.
- Passenger car: dark wood with cream windows; cargo wagon: open wagon with crates and sacks;
  ore wagon: open steel tub with grey ore and a gold nugget glint.
- All cars have the same coupler height so they line up in a train.
"""),
    ('vehicles/rails_track', 'Terrain decals · railway track', 'railway track', """
## This asset: railway track decals

Top-down decals laid on the ground along a route (the engine stretches them along the path), so
they are drawn **straight from above** like terrain: no outline around the tile, no light
direction. The track runs **left to right**.

## Deliverable

`rails_track.png` — **native 128 × 16 px** (×4 = 512 × 64), transparent. 4 cells of 32 × 16:

```
+----------+----------+----------+----------+
| track A  | track B  | bridge   | end stop |
+----------+----------+----------+----------+
```

- **track A / B:** two steel rails (Greys group, 1 px highlight) on dark wooden sleepers every
  5–6 px over a thin gravel bed; each tile is **seamless left–right**, and A and B are
  interchangeable. Transparent above and below the gravel bed.
- **bridge:** the same track on wooden planks with side beams (for crossing water).
- **end stop:** the track ending in a wooden buffer on the right.
"""),
    ('ui/ui_kit', 'UI · panels, buttons and cursors (optional)', 'UI kit', """
## This asset: UI kit (optional; the engine draws plain panels without it)

A pixel-art skin for the engine's immediate-mode UI. Panels and buttons are **9-slice**: the
engine stretches the middle and repeats nothing, so the centre areas must be flat or subtly
dithered, and the corners must work at any size. Dark, readable, fantasy-strategy look: dark
slate-violet panels (`#2E2433`, `#3A2A5C`) with a gold (`#C9962E`) or steel (`#9A96A1`) rim.

## Deliverable

`ui_kit.png` — **native 128 × 96 px** (×4 = 512 × 384), transparent.

```
x: 0              48              96        128
y:0 +---------------+---------------+---------+
    | panel         | panel (dark,  | cursors |  48x48 each; 9-slice with 8 px borders
    | (gold rim)    |  steel rim)   | 16x16 ×2 |  cursors: pointer, attack (sword),
 48 +---------------+---------------+  ×4      |  move (boot), forbidden, build (hammer),
    | button normal | button hover  |         |  grab (hand), target (crosshair),
    | 48x16         | 48x16         |         |  wait (hourglass)  — 2 columns × 4 rows
 64 +---------------+---------------+---------+
    | btn pressed   | btn disabled  | checkbox off/on 16x16 ×2 |
 80 +---------------+---------------+--------------------------+
    | progress bar frame 48x8 + fill 48x8 | health bar frame 32x6 + fill 32x6 | tooltip tail 8x8 |
 96 +-----------------------------------------------------------+
```

- Buttons are 9-slice with 4 px borders; hover is brighter, pressed is 1 px lower with a darker
  face, disabled is desaturated greys.
- Cursors: hotspot at the top-left pixel (pointer, sword tip, boot toe) or the centre
  (crosshair, forbidden, hourglass).
"""),
    ('ui/portraits_heroes', 'UI · hero portraits (optional)', 'hero portraits', """
## This asset: hero portraits (optional)

Head-and-shoulders portraits for the hero panel, town screen and chat. Same characters as the
approved hero sheets (match faces, armour and colours exactly), turned **three-quarters to the
right**, lit from the upper left, outline around the silhouette, a flat dark background
(`#2E2433`) filling the whole cell. Team colour in magenta on collars/cloaks.

## Deliverable

`portraits_heroes.png` — **native 192 × 64 px** (×4 = 768 × 256). 12 cells of 32 × 32:

```
row 1   paladin   archmage  ranger    warlord   farseer   chieftain
row 2   warden    keeper    priestess  footman  grunt     sentinel      (last three: generic units)
```
"""),
]


def extras():
    for path, title, what, body in EXTRAS:
        refs = {
            'resources': '',
            'spells': 'the approved `icons_resources.png`',
            'emotes': 'the approved `icons_resources.png`',
            'combat effects': 'the approved `fx_fire.png` (for flame and smoke style)',
            'flags': 'the approved `building_human_house.png`',
            'rail vehicles': 'the approved `building_human_house.png`',
            'railway track': 'the approved `terrain_road_cobble.png`',
            'UI kit': '',
            'hero portraits': 'the approved hero sheets (`heroes/*.png`)',
        }[what]
        flat = path.startswith(('icons/', 'ui/'))
        write(f'{path}.md', doc(title, body.replace('{rules}', ICON_RULES), extra_refs=refs,
                                check=FLAT_CHECK if flat or path.startswith(('fx/', 'vehicles/rails')) else None))
    return len(EXTRAS)


def main():
    count = 0
    write('00-style-reference.md', doc('00 · Style reference sheet', STYLE_REF, consistency=STYLE_REF_CONSISTENCY))
    count += 1
    for name, desc, cliff, anim in TERRAIN:
        title = name.replace('_', ' ')
        body = (TERRAIN_ANIM if anim else TERRAIN_STATIC).format(title=title, name=name, desc=desc, cliff=cliff)
        write(f'terrain/terrain_{name}.md',
              doc(f'Terrain · {title}', body, extra_refs='the approved `terrain_grass.png` (for density and tone)' if name != 'grass' else ''))
        count += 1
    for name, desc, w, h in PROPS:
        body = PROP_BODY.format(title=name.replace('_', ' '), name=name, desc=desc, w=w, h=h, w4=w * 4, w16=w * 16, h4=h * 4)
        write(f'terrain/props_{name}.md', doc(f'Nature props · {name.replace("_", " ")}', body,
                                              extra_refs='the approved `props_trees_oak.png`' if name != 'trees_oak' else ''))
        count += 1
    write('terrain/fx_fire.md', doc('Effects · fire and smoke', FIRE))
    count += 1

    def character(file, faction, title, desc, kind, cell, group, folder, refs):
        size_note = {48: 'Cell 48×48: a normal unit is 24–28 px tall, a hero 28–32 px.',
                     64: 'Cell 64×64: mounted, flying or large creature; the rider/creature is about 36–44 px tall.',
                     96: 'Cell 96×96: boss-sized creature, about 56 px tall.'}[cell]
        special = {'melee': 'attack2', 'ranged': 'shoot', 'caster': 'cast', 'worker': 'work'}[kind]
        sheet = CHAR_SHEET.format(file=file, sw=cell * 6, sh=cell * 6, sw4=cell * 24, sh4=cell * 24, cw=cell, ch=cell,
                                  attack=ATTACK_TEXT[kind], special_row=SPECIAL_ROWS[kind], size_note=size_note, special=special)
        body = CHAR_BODY.format(group=group, title=title, faction=faction, desc=desc,
                                faction_note=FACTION_NOTES[faction]) + sheet
        write(f'{folder}/{file}.md', doc(f'{group} · {title}', body, extra_refs=refs))

    for h in HEROES:
        file, faction = h[0], h[1]
        ref_unit = {'Humans': 'unit_human_footman', 'Orcs': 'unit_orc_grunt', 'Elves': 'unit_elf_sentinel'}[faction]
        character(*h, 'Hero', 'heroes', f'the approved `{ref_unit}.png` (heroes are a bit bigger and more detailed, same style)')
        count += 1
    for u in UNITS:
        file = u[0]
        refs = '' if file == 'unit_human_footman' else 'the approved `unit_human_footman.png`'
        folder = 'monsters' if file.startswith('monster_') else 'units'
        character(*u, 'Monster' if folder == 'monsters' else 'Unit', folder, refs)
        count += 1
    for file, faction, title, size, desc, anim in STRUCTURES:
        cw, ch, size_note = STRUCT_SIZES[size]
        note = FACTION_NOTES.get(faction, 'Neutral building: **no team colour** unless stated.')
        body = STRUCT_BODY.format(faction=faction, title=title, desc=desc, faction_note=note, size_note=size_note, file=file,
                                  sw=cw * 4, sh=ch * 2, sw4=cw * 16, sh4=ch * 8, cw=cw, ch=ch, anim=anim)
        refs = '' if file == 'building_human_house' else 'the approved `building_human_house.png`'
        write(f'structures/{file}.md', doc(f'Structure · {title}', body, extra_refs=refs))
        count += 1
    count += extras()
    print(f'wrote {count} prompts to {os.path.normpath(OUT)}')


if __name__ == '__main__':
    main()
