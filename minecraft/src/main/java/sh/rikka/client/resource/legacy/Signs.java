package sh.rikka.client.resource.legacy;

import java.util.List;

final class Signs implements Upgrade {
	private static final List<String> WOODS = List.of(
		"oak", "spruce", "birch", "jungle", "acacia", "dark_oak", "crimson", "warped", "mangrove", "bamboo", "cherry", "pale_oak"
	);
	private static final int ENTITY_WIDTH = 64;
	private static final int BLOCK_SIZE = 32;
	private static final int GUI_WIDTH = 24;
	private static final int GUI_HEIGHT = 26;
	private static final Piece[] SIGN_PIECES = {
		Piece.copy(2, 0, 24, 2, 0, 0),
		Piece.copy(2, 2, 24, 12, 0, 2),
		Piece.copy(26, 2, 2, 12, 24, 2),
		Piece.copy(28, 2, 24, 12, 0, 16),
		Piece.copy(0, 2, 2, 12, 24, 16),
		Piece.flipped(26, 0, 24, 2, 0, 28, Raster.FLIP_Y),
		Piece.copy(2, 16, 2, 14, 28, 0),
		Piece.copy(4, 16, 2, 14, 30, 0),
		Piece.copy(6, 16, 2, 14, 28, 16),
		Piece.copy(0, 16, 2, 14, 30, 16),
		Piece.copy(4, 14, 2, 2, 28, 30),
	};
	private static final Piece[] HANGING_SIGN_PIECES = {
		Piece.copy(4, 0, 16, 4, 0, 0),
		Piece.copy(4, 4, 16, 2, 0, 4),
		Piece.copy(20, 4, 4, 2, 16, 4),
		Piece.copy(24, 4, 16, 2, 0, 7),
		Piece.copy(0, 4, 4, 2, 16, 7),
		Piece.flipped(20, 0, 16, 4, 0, 9, Raster.FLIP_X | Raster.FLIP_Y),
		Piece.copy(14, 6, 12, 6, 20, 0),
		Piece.copy(0, 6, 3, 6, 22, 7),
		Piece.copy(6, 7, 3, 4, 28, 8),
		Piece.copy(2, 12, 14, 2, 2, 14),
		Piece.copy(0, 14, 2, 10, 0, 16),
		Piece.copy(2, 14, 14, 10, 2, 16),
		Piece.copy(16, 14, 2, 10, 16, 16),
		Piece.copy(18, 14, 14, 10, 18, 16),
		Piece.flipped(16, 12, 14, 2, 2, 26, Raster.FLIP_Y),
	};
	private static final Piece[] GUI_PIECES = {
		Piece.copy(2, 2, 24, 12, 0, 0),
		Piece.copy(2, 16, 2, 14, 11, 12),
	};

	@Override
	public void apply(Conversion conversion) {
		Assets assets = conversion.assets();
		for (String wood : WOODS) {
			String sign = "entity/signs/" + wood;
			assets.findTexture(sign).ifPresent(entity -> {
				assets.removeTexture(sign);
				assets.putTexture("block/" + wood + "_sign", () -> assemble(entity.raster(assets.decoder()), BLOCK_SIZE, BLOCK_SIZE, SIGN_PIECES));
				assets.putTexture("gui/signs/" + wood, () -> assemble(entity.raster(assets.decoder()), GUI_WIDTH, GUI_HEIGHT, GUI_PIECES));
			});
			String hanging = "entity/signs/hanging/" + wood;
			assets.findTexture(hanging).ifPresent(entity -> {
				assets.removeTexture(hanging);
				assets.putTexture("block/" + wood + "_hanging_sign", () -> assemble(entity.raster(assets.decoder()), BLOCK_SIZE, BLOCK_SIZE, HANGING_SIGN_PIECES));
			});
		}
	}

	private static Raster assemble(Raster entity, int width, int height, Piece[] pieces) {
		double scale = entity.scale(ENTITY_WIDTH);
		Raster result = Raster.blank((int) Math.round(width * scale), (int) Math.round(height * scale));
		for (Piece piece : pieces) {
			Raster part = entity.crop(piece.from().scaled(scale)).unoriented(piece.orientation());
			result.draw(part, (int) Math.round(piece.x() * scale), (int) Math.round(piece.y() * scale));
		}
		return result;
	}

	private record Piece(Rect from, int x, int y, int orientation) {
		static Piece copy(int fromX, int fromY, int width, int height, int x, int y) {
			return new Piece(Rect.of(fromX, fromY, width, height), x, y, 0);
		}

		static Piece flipped(int fromX, int fromY, int width, int height, int x, int y, int orientation) {
			return new Piece(Rect.of(fromX, fromY, width, height), x, y, orientation);
		}
	}
}
