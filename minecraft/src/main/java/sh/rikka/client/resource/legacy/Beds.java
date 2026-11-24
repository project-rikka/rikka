package sh.rikka.client.resource.legacy;

import java.util.List;

final class Beds implements Upgrade {
	private static final List<String> COLORS = List.of(
		"white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray",
		"light_gray", "cyan", "purple", "blue", "brown", "green", "red", "black"
	);
	private static final String LEGACY_COLOR = "red";
	private static final int TILE = 16;
	private static final int ENTITY_WIDTH = 64;
	private static final Rect HEAD_UP = Rect.of(6, 6, 16, 16);
	private static final Rect FOOT_UP = Rect.of(6, 28, 16, 16);
	private static final Rect HEAD_EAST = Rect.of(15, 6, 16, 16);
	private static final Rect HEAD_WEST = Rect.of(-3, 6, 16, 16);
	private static final Rect FOOT_EAST = Rect.of(15, 28, 16, 16);
	private static final Rect FOOT_WEST = Rect.of(-3, 28, 16, 16);
	private static final Rect FOOT_END = Rect.of(22, 19, 16, 16);
	private static final int MATTRESS_TOP = 7;
	private static final int MATTRESS_BOTTOM = 13;

	@Override
	public void apply(Conversion conversion) {
		Assets assets = conversion.assets();
		Vanilla vanilla = conversion.vanilla();
		legacy(assets);
		for (String color : COLORS) {
			String id = "entity/bed/" + color;
			assets.findTexture(id).ifPresent(bed -> {
				assets.removeTexture(id);
				String prefix = "block/" + color + "_bed_";
				assets.putTexture(prefix + "head_up", () -> crop(bed.raster(assets.decoder()), HEAD_UP));
				assets.putTexture(prefix + "foot_up", () -> crop(bed.raster(assets.decoder()), FOOT_UP));
				mattress(assets, vanilla, prefix + "head_east", () -> crop(bed.raster(assets.decoder()), HEAD_EAST).unoriented(Raster.TRANSPOSE | Raster.FLIP_X));
				mattress(assets, vanilla, prefix + "head_west", () -> crop(bed.raster(assets.decoder()), HEAD_WEST).unoriented(Raster.TRANSPOSE | Raster.FLIP_Y));
				mattress(assets, vanilla, prefix + "foot_east", () -> crop(bed.raster(assets.decoder()), FOOT_EAST).unoriented(Raster.TRANSPOSE | Raster.FLIP_X));
				mattress(assets, vanilla, prefix + "foot_west", () -> crop(bed.raster(assets.decoder()), FOOT_WEST).unoriented(Raster.TRANSPOSE | Raster.FLIP_Y));
				mattress(assets, vanilla, prefix + "foot_south", () -> crop(bed.raster(assets.decoder()), FOOT_END).unoriented(Raster.FLIP_Y));
			});
		}
	}

	private static void legacy(Assets assets) {
		String prefix = "block/" + LEGACY_COLOR + "_bed_";
		assets.findTexture("block/bed_head_top").ifPresent(top -> assets.putTexture(prefix + "head_up", () -> top.raster(assets.decoder()).rotated(3)));
		assets.findTexture("block/bed_feet_top").ifPresent(top -> assets.putTexture(prefix + "foot_up", () -> top.raster(assets.decoder()).rotated(3)));
		assets.findTexture("block/bed_head_side").ifPresent(side -> {
			assets.put(Assets.texture(prefix + "head_east"), side);
			assets.putTexture(prefix + "head_west", () -> side.raster(assets.decoder()).flippedHorizontally());
		});
		assets.findTexture("block/bed_feet_side").ifPresent(side -> {
			assets.put(Assets.texture(prefix + "foot_east"), side);
			assets.putTexture(prefix + "foot_west", () -> side.raster(assets.decoder()).flippedHorizontally());
		});
		assets.findTexture("block/bed_feet_end").ifPresent(end -> assets.put(Assets.texture(prefix + "foot_south"), end));
		assets.findTexture("block/bed_head_end").ifPresent(end -> assets.put(Assets.texture("block/bed_head_north"), end));
		for (String part : List.of("head_top", "head_side", "head_end", "feet_top", "feet_side", "feet_end")) {
			assets.removeTexture("block/bed_" + part);
		}
	}

	private static Raster crop(Raster entity, Rect rect) {
		return entity.crop(rect.scaled(entity.scale(ENTITY_WIDTH)));
	}

	private static void mattress(Assets assets, Vanilla vanilla, String id, RasterSource strip) {
		assets.putTexture(id, () -> {
			Raster mattress = strip.get();
			double scale = mattress.scale(TILE);
			Raster result = vanilla.texture(id).map(base -> base.resized(mattress.width(), mattress.height())).orElse(Raster.blank(mattress.width(), mattress.height()));
			int top = (int) Math.round(MATTRESS_TOP * scale);
			int bottom = (int) Math.round(MATTRESS_BOTTOM * scale);
			result.draw(mattress, Rect.of(0, top, mattress.width(), bottom - top), 0, top);
			return result;
		});
	}
}
