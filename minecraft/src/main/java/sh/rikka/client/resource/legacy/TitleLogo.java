package sh.rikka.client.resource.legacy;

final class TitleLogo implements Upgrade {
	private static final String LOGO = "gui/title/minecraft";
	private static final int REFERENCE_WIDTH = 256;
	private static final Rect LEFT = Rect.of(0, 0, 155, 44);
	private static final Rect RIGHT = Rect.of(0, 45, 119, 44);
	private static final int COMBINED_WIDTH = 274;
	private static final int LOGO_WIDTH = 256;
	private static final int LOGO_HEIGHT = 44;
	private static final int TEXTURE_HEIGHT = 64;

	@Override
	public void apply(Conversion conversion) {
		Assets assets = conversion.assets();
		assets.findTexture(LOGO).ifPresent(logo -> assets.putTexture(LOGO, () -> {
			Raster source = logo.raster(assets.decoder());
			double scale = source.scale(REFERENCE_WIDTH);
			Rect left = LEFT.scaled(scale);
			Rect right = RIGHT.scaled(scale);
			Raster combined = Raster.blank((int) Math.round(COMBINED_WIDTH * scale), left.height());
			combined.draw(source, left, 0, 0);
			combined.draw(source, right, left.width(), 0);
			int width = (int) Math.round(LOGO_WIDTH * scale);
			int height = Math.min((int) Math.round(LOGO_HEIGHT * scale), combined.height() * width / combined.width());
			Raster result = Raster.blank(width, (int) Math.round(TEXTURE_HEIGHT * scale));
			result.draw(combined.averaged(width, height), 0, ((int) Math.round(LOGO_HEIGHT * scale) - height) / 2);
			return result;
		}));
	}
}
