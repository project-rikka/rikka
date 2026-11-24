package sh.rikka.client.resource.legacy;

import java.util.List;

final class ChestLayouts implements Upgrade {
	private static final String DIRECTORY = "entity/chest/";
	private static final List<String> SINGLE = List.of("normal", "trapped", "christmas", "ender");
	private static final List<String> DOUBLE = List.of("normal", "trapped", "christmas");
	private static final int SINGLE_WIDTH = 64;
	private static final int DOUBLE_WIDTH = 128;
	private static final int SIZE = 64;

	private static final Part[] SINGLE_PARTS = {
		Part.flipV(14, 0, 28, 14, 28, 0), Part.flipV(28, 0, 42, 14, 14, 0), Part.flipV(28, 19, 42, 33, 14, 19), Part.flipV(14, 19, 28, 33, 28, 19),
		Part.turn(28, 14, 42, 19, 28, 14), Part.turn(42, 14, 56, 19, 14, 14), Part.turn(14, 14, 28, 19, 42, 14), Part.turn(0, 14, 14, 19, 0, 14),
		Part.turn(0, 33, 14, 43, 0, 33), Part.turn(42, 33, 56, 43, 14, 33), Part.turn(28, 33, 42, 43, 28, 33), Part.turn(14, 33, 28, 43, 42, 33),
		Part.flipV(0, 1, 1, 5, 0, 1), Part.copy(3, 0, 5, 1, 1, 0), Part.copy(1, 0, 3, 1, 3, 0), Part.turn(1, 1, 3, 5, 4, 1), Part.turn(3, 1, 6, 5, 1, 1),
	};
	private static final Part[] LEFT_PARTS = {
		Part.flipV(59, 19, 74, 33, 14, 19), Part.flipV(59, 0, 74, 14, 14, 0), Part.flipV(29, 19, 44, 33, 29, 19), Part.turn(29, 14, 44, 19, 43, 14),
		Part.flipV(29, 0, 44, 14, 29, 0), Part.turn(44, 33, 58, 43, 29, 33), Part.turn(29, 33, 44, 43, 43, 33), Part.turn(58, 33, 73, 43, 14, 33),
		Part.turn(44, 14, 58, 19, 29, 14), Part.turn(58, 14, 73, 19, 14, 14),
		Part.copy(4, 0, 5, 1, 1, 0), Part.copy(2, 0, 3, 1, 2, 0), Part.turn(2, 1, 5, 5, 1, 1),
	};
	private static final Part[] RIGHT_PARTS = {
		Part.flipV(44, 0, 59, 14, 14, 0), Part.flipV(14, 0, 29, 14, 29, 0), Part.flipV(14, 19, 29, 33, 29, 19), Part.turn(0, 14, 14, 19, 0, 14),
		Part.turn(73, 14, 88, 19, 14, 14), Part.turn(14, 14, 29, 19, 43, 14), Part.flipV(44, 19, 59, 33, 14, 19), Part.turn(14, 33, 29, 43, 43, 33),
		Part.turn(0, 33, 14, 43, 0, 33), Part.turn(73, 33, 88, 43, 14, 33),
		Part.copy(3, 0, 4, 1, 1, 0), Part.copy(1, 0, 2, 1, 2, 0), Part.flipV(0, 1, 1, 5, 0, 1), Part.flipV(5, 1, 6, 5, 1, 1), Part.flipV(1, 1, 2, 5, 3, 1),
	};

	@Override
	public void apply(Conversion conversion) {
		Assets assets = conversion.assets();
		for (String name : DOUBLE) {
			String id = DIRECTORY + name + "_double";
			assets.findTexture(id).ifPresent(source -> {
				assets.removeTexture(id);
				assets.putTexture(DIRECTORY + name + "_left", () -> remap(source.raster(assets.decoder()), DOUBLE_WIDTH, LEFT_PARTS));
				assets.putTexture(DIRECTORY + name + "_right", () -> remap(source.raster(assets.decoder()), DOUBLE_WIDTH, RIGHT_PARTS));
			});
		}
		for (String name : SINGLE) {
			String id = DIRECTORY + name;
			assets.findTexture(id).ifPresent(source -> assets.putTexture(id, () -> remap(source.raster(assets.decoder()), SINGLE_WIDTH, SINGLE_PARTS)));
		}
	}

	private static Raster remap(Raster source, int referenceWidth, Part[] parts) {
		double scale = source.scale(referenceWidth);
		int size = (int) Math.round(SIZE * scale);
		Raster result = Raster.blank(size, size);
		for (Part part : parts) {
			Rect from = Rect.of(part.x1(), part.y1(), part.x2() - part.x1(), part.y2() - part.y1()).scaled(scale);
			Raster piece = source.crop(from);
			Raster placed = switch (part.transform()) {
				case COPY -> piece;
				case FLIP_VERTICAL -> piece.flippedVertically();
				case TURN -> piece.rotated(2);
			};
			result.draw(placed, (int) Math.round(part.toX() * scale), (int) Math.round(part.toY() * scale));
		}
		return result;
	}

	private enum Transform {
		COPY,
		FLIP_VERTICAL,
		TURN
	}

	private record Part(int x1, int y1, int x2, int y2, int toX, int toY, Transform transform) {
		static Part copy(int x1, int y1, int x2, int y2, int toX, int toY) {
			return new Part(x1, y1, x2, y2, toX, toY, Transform.COPY);
		}

		static Part flipV(int x1, int y1, int x2, int y2, int toX, int toY) {
			return new Part(x1, y1, x2, y2, toX, toY, Transform.FLIP_VERTICAL);
		}

		static Part turn(int x1, int y1, int x2, int y2, int toX, int toY) {
			return new Part(x1, y1, x2, y2, toX, toY, Transform.TURN);
		}
	}
}
