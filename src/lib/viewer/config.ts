// Tile pyramid settings (D-014). Bump `version` whenever the output changes; older caches are then
// rebuilt. The last level defines the viewer's world px.
export const TILES = { version: 1, levels: [1024, 2048, 4096, 8192], tile: 512, block: 2048, quality: 0.8 } as const;
export const TOP_LEVEL = TILES.levels[TILES.levels.length - 1];
