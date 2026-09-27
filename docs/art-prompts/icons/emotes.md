# Icons · emotes

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

## This asset: chat emotes

Shown in speech bubbles above heroes in multiplayer (at ×2), often many at once, so each must be
recognisable from its silhouette and main colour alone.

Icons are small, bold and readable at native size **and** at ×2 on a dark UI panel. Draw each as
a chunky object seen slightly from above (3/4), lit from the upper left, with the **1-pixel
`#1B1420` outline** around the silhouette and 1–2 pixels of transparent margin inside the cell.
No background, no frame, no text.

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

## Consistency with the rest of the pack

- Attach the approved **style reference** (`style_reference.png`) and the approved `icons_resources.png` and match it exactly:
  same pixel density, outline colour, palette, light direction, proportions and level of detail.
- If a detail conflicts with the reference, the reference wins; if the reference conflicts with
  the pixel rules, the pixel rules win.

## Final check before you deliver (fix anything that fails)

1. Image size is exactly the native size (or an exact integer multiple) — cells line up with the
   grid, nothing crosses a cell border.
2. Only master-palette colours (+ transparency / `#00FF00`) are used; no smoothing, no blur.
3. Magenta appears only on team-coloured parts.
4. Outline rule and upper-left light are respected; each item is centred in its cell (standing
   objects such as flags and vehicles keep their base on the bottom row).
5. No text, letters, digits, frames, drop shadows, watermarks or extra objects.
