package sh.rikka.client.resource.legacy;

import java.io.IOException;
import java.util.Optional;

final class ContainerLayouts implements Upgrade {
	private static final int SLOT = GuiStyle.SLOT_SIZE;

	private static final Rect INVENTORY_CRAFTING = Rect.of(86, 24, 76, 38);
	private static final int INVENTORY_CRAFTING_DX = 10;
	private static final int INVENTORY_CRAFTING_DY = -8;
	private static final Rect INVENTORY_ARMOR_SLOT = Rect.of(7, 61, SLOT, SLOT);
	private static final Rect INVENTORY_OFFHAND_SLOT = Rect.of(76, 61, SLOT, SLOT);
	private static final Rect INVENTORY_PLAYER_BORDER = Rect.of(78, 7, 1, 72);
	private static final Rect INVENTORY_PLAYER_TRIM = Rect.of(76, 7, 3, 72);
	private static final int INVENTORY_PLAYER_BORDER_X = 75;

	private static final Rect CREATIVE_PLAYER = Rect.of(7, 4, 74, 47);
	private static final int CREATIVE_PLAYER_DX = 45;
	private static final Rect CREATIVE_OFFHAND_SLOT = Rect.of(34, 19, SLOT, SLOT);
	private static final Rect CREATIVE_ARMOR_SLOT = Rect.of(8, 5, SLOT, SLOT);

	private static final Rect BREWING_TOP = Rect.of(16, 15, 104, 61);
	private static final Rect BREWING_BOTTLE_SLOT = Rect.of(55, 45, SLOT, SLOT);

	private static final Rect ENCHANTING_LAPIS_SLOT = Rect.of(34, 46, SLOT, SLOT);

	private static final Rect HORSE_SADDLE_SLOT = Rect.of(7, 17, SLOT, SLOT);

	private static final Rect BEACON_SQUIGGLE = Rect.of(58, 108, 5, 18);
	private static final int BEACON_SQUIGGLE_X = 36;

	@Override
	public void apply(Conversion conversion) throws IOException {
		Optional<GuiStyle> learned = GuiStyle.learn(conversion);
		if (learned.isEmpty()) {
			return;
		}
		GuiStyle style = learned.get();
		if (conversion.sourceBefore(PackFormats.V1_11)) {
			edit(conversion, style, "inventory", (pack, target) -> {
				Raster block = pack.crop(scaled(style, INVENTORY_CRAFTING));
				Raster border = pack.crop(scaled(style, INVENTORY_PLAYER_BORDER));
				synthesize(pack, target, style, INVENTORY_CRAFTING);
				synthesize(pack, target, style, INVENTORY_PLAYER_TRIM);
				pack.draw(border, style.scaled(INVENTORY_PLAYER_BORDER_X), style.scaled(INVENTORY_PLAYER_BORDER.y()));
				pack.draw(block, style.scaled(INVENTORY_CRAFTING.x() + INVENTORY_CRAFTING_DX), style.scaled(INVENTORY_CRAFTING.y() + INVENTORY_CRAFTING_DY));
				if (conversion.sourceBefore(PackFormats.V1_9)) {
					pack.draw(pack.crop(scaled(style, INVENTORY_ARMOR_SLOT)), style.scaled(INVENTORY_OFFHAND_SLOT.x()), style.scaled(INVENTORY_OFFHAND_SLOT.y()));
				}
			});
			edit(conversion, style, "horse", (pack, target) -> {
				ghost(conversion, pack, style, HORSE_SADDLE_SLOT, "saddle");
				synthesize(pack, target, style, HORSE_SADDLE_SLOT);
			});
		}
		if (conversion.sourceBefore(PackFormats.V1_9)) {
			edit(conversion, style, "creative_inventory/tab_inventory", (pack, target) -> {
				Raster block = pack.crop(scaled(style, CREATIVE_PLAYER));
				Raster armorSlot = pack.crop(scaled(style, CREATIVE_ARMOR_SLOT));
				synthesize(pack, target, style, CREATIVE_PLAYER);
				pack.draw(block, style.scaled(CREATIVE_PLAYER.x() + CREATIVE_PLAYER_DX), style.scaled(CREATIVE_PLAYER.y()));
				pack.draw(armorSlot, style.scaled(CREATIVE_OFFHAND_SLOT.x()), style.scaled(CREATIVE_OFFHAND_SLOT.y()));
			});
			edit(conversion, style, "brewing_stand", (pack, target) -> {
				ghost(conversion, pack, style, BREWING_BOTTLE_SLOT, "potion");
				synthesize(pack, target, style, BREWING_TOP);
			});
		}
		if (conversion.sourceBefore(PackFormats.V1_13)) {
			edit(conversion, style, "enchanting_table", (pack, target) -> {
				ghost(conversion, pack, style, ENCHANTING_LAPIS_SLOT, "lapis_lazuli");
				pack.draw(style.slot(), style.scaled(ENCHANTING_LAPIS_SLOT.x()), style.scaled(ENCHANTING_LAPIS_SLOT.y()));
			});
		}
		if (conversion.sourceBefore(PackFormats.V1_15)) {
			edit(conversion, style, "beacon", (pack, target) ->
				pack.draw(pack.crop(scaled(style, BEACON_SQUIGGLE)), style.scaled(BEACON_SQUIGGLE_X), style.scaled(BEACON_SQUIGGLE.y())));
		}
	}

	@FunctionalInterface
	private interface Edit {
		void apply(Raster pack, Raster target) throws IOException;
	}

	private static void edit(Conversion conversion, GuiStyle style, String container, Edit edit) throws IOException {
		Assets assets = conversion.assets();
		String id = GuiStyle.CONTAINERS + container;
		Optional<Raster> target = conversion.vanilla().texture(id);
		if (!assets.hasTexture(id) || target.isEmpty()) {
			return;
		}
		Raster source = assets.textureRaster(id);
		Raster pack = source.width() == style.scaled(target.get().width())
			? source
			: source.resized(style.scaled(target.get().width()), style.scaled(source.height() * target.get().width() / source.width()));
		edit.apply(pack, target.get());
		assets.putTexture(id, () -> pack);
	}

	private static Rect scaled(GuiStyle style, Rect rect) {
		return rect.scaled(style.scale());
	}

	private static void synthesize(Raster pack, Raster target, GuiStyle style, Rect region) {
		Raster composed = GuiComposer.compose(target.crop(region), null, (x, y) -> false, style);
		pack.draw(composed, style.scaled(region.x()), style.scaled(region.y()));
	}

	private static void ghost(Conversion conversion, Raster pack, GuiStyle style, Rect frame, String icon) {
		String id = GuiStyle.SLOT_SPRITES + icon;
		if (conversion.assets().hasTexture(id)) {
			return;
		}
		Raster result = SlotIcon.extract(pack.crop(scaled(style, frame)), style.slot(), style.scale());
		if (!result.isTransparent()) {
			conversion.assets().putTexture(id, () -> result);
		}
	}
}
