package sh.rikka.client.resource.legacy;

import java.io.IOException;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Optional;

final class GuiSynthesis implements Upgrade {
	private static final int SHARED_MARGIN = 2;
	private static final List<String> CONTAINERS = List.of(
		"anvil", "beacon", "blast_furnace", "brewing_stand", "cartography_table", "crafter", "crafting_table", "dispenser",
		"enchanting_table", "furnace", "generic_54", "grindstone", "hopper", "horse", "inventory", "loom", "nautilus",
		"shulker_box", "smithing", "smoker", "stonecutter", "villager",
		"creative_inventory/tab_inventory", "creative_inventory/tab_items", "creative_inventory/tab_item_search"
	);
	private static final Map<String, List<String>> SPRITE_SETS = Map.of(
		"stonecutter", List.of("recipe", "recipe_highlighted", "recipe_selected", "scroller", "scroller_disabled"),
		"loom", List.of("error", "pattern", "pattern_highlighted", "pattern_selected", "scroller", "scroller_disabled"),
		"cartography_table", List.of("duplicated_map", "error", "locked", "map", "scaled_map"),
		"grindstone", List.of("error"),
		"smithing", List.of("error"),
		"crafter", List.of("disabled_slot", "powered_redstone", "unpowered_redstone"),
		"villager", List.of("discount_strikethrough", "experience_bar_background", "experience_bar_current", "experience_bar_result", "out_of_stock", "scroller", "scroller_disabled", "trade_arrow", "trade_arrow_out_of_stock"),
		"blast_furnace", List.of("burn_progress", "lit_progress"),
		"smoker", List.of("burn_progress", "lit_progress")
	);
	private static final Map<String, String> ALIASES = Map.ofEntries(
		Map.entry("blast_furnace/burn_progress", "furnace/burn_progress"),
		Map.entry("blast_furnace/lit_progress", "furnace/lit_progress"),
		Map.entry("smoker/burn_progress", "furnace/burn_progress"),
		Map.entry("smoker/lit_progress", "furnace/lit_progress"),
		Map.entry("stonecutter/scroller", "creative_inventory/scroller"),
		Map.entry("stonecutter/scroller_disabled", "creative_inventory/scroller_disabled"),
		Map.entry("loom/scroller", "creative_inventory/scroller"),
		Map.entry("loom/scroller_disabled", "creative_inventory/scroller_disabled"),
		Map.entry("grindstone/error", "anvil/error"),
		Map.entry("smithing/error", "anvil/error"),
		Map.entry("cartography_table/error", "anvil/error")
	);

	@Override
	public void apply(Conversion conversion) throws IOException {
		Optional<GuiStyle> learned = GuiStyle.learn(conversion);
		if (learned.isEmpty()) {
			return;
		}
		GuiStyle style = learned.get();
		Assets assets = conversion.assets();
		for (Map.Entry<String, String> alias : ALIASES.entrySet()) {
			String target = GuiStyle.CONTAINER_SPRITES + alias.getKey();
			assets.findTexture(GuiStyle.CONTAINER_SPRITES + alias.getValue()).ifPresent(source -> {
				if (!assets.hasTexture(target)) {
					assets.put(Assets.texture(target), source);
				}
			});
		}
		if (style.palette().identity()) {
			return;
		}
		Vanilla vanilla = conversion.vanilla();
		List<String> references = new ArrayList<>();
		List<String> missing = new ArrayList<>();
		for (String container : CONTAINERS) {
			String id = GuiStyle.CONTAINERS + container;
			if (assets.hasTexture(id)) {
				references.add(id);
			} else {
				missing.add(id);
			}
		}
		for (String id : missing) {
			Optional<Raster> target = vanilla.texture(id);
			if (target.isEmpty()) {
				continue;
			}
			assets.putTexture(id, () -> compose(assets, vanilla, references, target.get(), style));
			String container = id.substring(GuiStyle.CONTAINERS.length());
			for (String sprite : SPRITE_SETS.getOrDefault(container, List.of())) {
				String spriteId = GuiStyle.CONTAINER_SPRITES + container + "/" + sprite;
				if (assets.hasTexture(spriteId)) {
					continue;
				}
				Optional<Raster> vanillaSprite = vanilla.texture(spriteId);
				vanillaSprite.ifPresent(raster -> assets.putTexture(spriteId, () -> GuiComposer.recolor(raster, style)));
			}
		}
	}

	private static Raster compose(Assets assets, Vanilla vanilla, List<String> references, Raster target, GuiStyle style) throws IOException {
		String best = null;
		Raster bestVanilla = null;
		long bestScore = 0;
		for (String id : references) {
			Optional<Raster> reference = vanilla.texture(id);
			if (reference.isEmpty()) {
				continue;
			}
			long score = score(reference.get(), target);
			if (score > bestScore) {
				bestScore = score;
				best = id;
				bestVanilla = reference.get();
			}
		}
		if (best == null) {
			return GuiComposer.compose(target, null, (x, y) -> false, style);
		}
		Raster pack = assets.textureRaster(best);
		int width = style.scaled(bestVanilla.width());
		Raster referencePack = pack.width() == width ? pack : pack.resized(width, width * pack.height() / pack.width());
		boolean[] shared = shared(bestVanilla, target);
		return GuiComposer.compose(target, referencePack, (x, y) -> shared[y * target.width() + x], style);
	}

	private static boolean[] shared(Raster reference, Raster target) {
		int width = target.width();
		int height = target.height();
		boolean[] shared = new boolean[width * height];
		for (int y = 0; y < height; y++) {
			for (int x = 0; x < width; x++) {
				shared[y * width + x] = sameAround(reference, target, x, y);
			}
		}
		return shared;
	}

	private static boolean sameAround(Raster reference, Raster target, int x, int y) {
		for (int dy = -SHARED_MARGIN; dy <= SHARED_MARGIN; dy++) {
			for (int dx = -SHARED_MARGIN; dx <= SHARED_MARGIN; dx++) {
				int px = x + dx;
				int py = y + dy;
				if (px < 0 || py < 0 || px >= target.width() || py >= target.height()) {
					continue;
				}
				if (px >= reference.width() || py >= reference.height() || reference.get(px, py) != target.get(px, py)) {
					return false;
				}
			}
		}
		return true;
	}

	private static long score(Raster reference, Raster target) {
		long score = 0;
		int width = Math.min(reference.width(), target.width());
		int height = Math.min(reference.height(), target.height());
		for (int y = 0; y < height; y++) {
			for (int x = 0; x < width; x++) {
				int pixel = target.get(x, y);
				if (Colors.alpha(pixel) != 0 && pixel == reference.get(x, y)) {
					score++;
				}
			}
		}
		return score;
	}
}
