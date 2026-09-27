# 00 · Style reference sheet

> Copy everything below this line into the image AI. Attach the reference images listed in "Consistency". Deliver the PNG(s) with the exact file name(s).

---

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

| Group | Colours |
| --- | --- |
| Outline | `#1B1420` |
| Deep shadow | `#2E2433` |
| Greys / stone / steel | `#4A4553` `#6E6977` `#9A96A1` `#C9C6CC` `#F2F0EB` |
| Browns / wood / earth | `#3D2A1E` `#5C3F28` `#7D5A3A` `#A67C52` `#D1A574` |
| Human skin | `#8C5A3C` `#D99A6C` `#F2C8A0` |
| Orc skin | `#2E4A22` `#4F7A30` `#7FA848` |
| Nature greens | `#1F3D24` `#2F5A2E` `#4A7D3A` `#6FA848` `#A8D46A` |
| Gold / yellow | `#7A5A1E` `#C9962E` `#F2D35E` |
| Reds / fire | `#5E1F1F` `#9C2F2A` `#D8503C` `#F28A5E` `#FFD27A` |
| Blues / water / ice | `#1E2E5C` `#2F55A0` `#4A86D8` `#8EC3F2` `#D6EEFA` |
| Elf violet | `#3A2A5C` `#6A4AA8` `#A88AE8` |
| Magic teal | `#1F5E5E` `#2E9A8A` `#7AE8D8` |
| TEAM COLOUR (magenta key) | `#660066` `#990099` `#CC00CC` `#FF00FF` |

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

## Consistency with the rest of the pack

This sheet **is** the reference: every other asset will be drawn to match it. Make it the
cleanest possible example of the pixel rules above — exact palette, 1-pixel `#1B1420` outlines,
upper-left light, correct scale (hex 28×32, footman ~26 px tall).

## Final check before you deliver (fix anything that fails)

1. Image size is exactly the native size (or an exact integer multiple) — cells line up with the
   grid, nothing crosses a cell border.
2. Only master-palette colours (+ transparency / `#00FF00`) are used; no smoothing, no blur.
3. Magenta appears only on team-coloured parts.
4. Outline rule, upper-left light and 3/4 view are respected; sprites face right; feet on the
   bottom row of every cell.
5. No text, frames, shadows, watermarks or extra objects.
