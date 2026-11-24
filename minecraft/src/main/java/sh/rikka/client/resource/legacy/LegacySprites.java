package sh.rikka.client.resource.legacy;

final class LegacySprites implements Upgrade {
	private static final String HOTBAR = GuiStyle.SPRITES + "hud/hotbar";
	private static final int HOTBAR_WIDTH = 182;
	private static final int OFFHAND_SLICE = 11;
	private static final int OFFHAND_WIDTH = 29;
	private static final int OFFHAND_HEIGHT = 24;
	private static final int OFFHAND_RIGHT_INSET = 7;
	private static final int BUTTON_WIDTH = 200;
	private static final int HANDLE_HALF = 4;
	private static final String EFFECT_LARGE = GuiStyle.CONTAINER_SPRITES + "inventory/effect_background_large";
	private static final String EFFECT_SMALL = GuiStyle.CONTAINER_SPRITES + "inventory/effect_background_small";
	private static final int EFFECT_LARGE_WIDTH = 120;
	private static final int EFFECT_SMALL_HALF = 16;

	@Override
	public void apply(Conversion conversion) {
		Assets assets = conversion.assets();
		if (conversion.sourceBefore(PackFormats.V1_9)) {
			offhand(assets, "hud/hotbar_offhand_left", 0);
			offhand(assets, "hud/hotbar_offhand_right", OFFHAND_RIGHT_INSET);
		}
		handle(assets, "widget/button", "widget/slider_handle");
		handle(assets, "widget/button_highlighted", "widget/slider_handle_highlighted");
		if (conversion.sourceBefore(PackFormats.V1_18) && !assets.hasTexture(EFFECT_SMALL)) {
			assets.findTexture(EFFECT_LARGE).ifPresent(large -> assets.putTexture(EFFECT_SMALL, () -> {
				Raster source = large.raster(assets.decoder());
				return source.edges((int) Math.round(EFFECT_SMALL_HALF * source.scale(EFFECT_LARGE_WIDTH)));
			}));
		}
	}

	private static void offhand(Assets assets, String sprite, int inset) {
		String id = GuiStyle.SPRITES + sprite;
		if (assets.hasTexture(id)) {
			return;
		}
		assets.findTexture(HOTBAR).ifPresent(hotbar -> assets.putTexture(id, () -> {
			Raster source = hotbar.raster(assets.decoder());
			double scale = source.scale(HOTBAR_WIDTH);
			Raster result = Raster.blank((int) Math.round(OFFHAND_WIDTH * scale), (int) Math.round(OFFHAND_HEIGHT * scale));
			result.draw(source.edges((int) Math.round(OFFHAND_SLICE * scale)), (int) Math.round(inset * scale), (int) Math.round(scale));
			return result;
		}));
	}

	private static void handle(Assets assets, String button, String handle) {
		String id = GuiStyle.SPRITES + handle;
		if (assets.hasTexture(id)) {
			return;
		}
		assets.findTexture(GuiStyle.SPRITES + button).ifPresent(source -> assets.putTexture(id, () -> {
			Raster image = source.raster(assets.decoder());
			return image.edges((int) Math.round(HANDLE_HALF * image.scale(BUTTON_WIDTH)));
		}));
	}
}
