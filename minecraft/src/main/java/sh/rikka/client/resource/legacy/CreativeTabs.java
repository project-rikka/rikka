package sh.rikka.client.resource.legacy;

final class CreativeTabs implements Upgrade {
	private static final String SHEET = "gui/container/creative_inventory/tabs";
	private static final int REFERENCE_WIDTH = 256;
	private static final int OLD_TAB_WIDTH = 28;
	private static final int NEW_TAB_WIDTH = 26;
	private static final int LEFT_COLUMNS = 12;
	private static final int[] SOURCE_TABS = {0, 1, 2, 3, 4, 4, 5};
	private static final int TABS_HEIGHT = 128;
	private static final Rect SCROLLERS = Rect.of(232, 0, 24, 15);

	@Override
	public void apply(Conversion conversion) {
		Assets assets = conversion.assets();
		assets.findTexture(SHEET).ifPresent(sheet -> assets.putTexture(SHEET, () -> {
			Raster source = sheet.raster(assets.decoder());
			double scale = source.scale(REFERENCE_WIDTH);
			Raster result = Raster.blank(source.width(), source.height());
			for (int tab = 0; tab < SOURCE_TABS.length; tab++) {
				int from = SOURCE_TABS[tab] * OLD_TAB_WIDTH;
				int to = tab * NEW_TAB_WIDTH;
				Rect left = Rect.of(from, 0, LEFT_COLUMNS, TABS_HEIGHT).scaled(scale);
				Rect right = Rect.of(from + OLD_TAB_WIDTH - (NEW_TAB_WIDTH - LEFT_COLUMNS), 0, NEW_TAB_WIDTH - LEFT_COLUMNS, TABS_HEIGHT).scaled(scale);
				result.draw(source, left, (int) Math.round(to * scale), 0);
				result.draw(source, right, (int) Math.round((to + LEFT_COLUMNS) * scale), 0);
			}
			Rect scrollers = SCROLLERS.scaled(scale);
			result.draw(source, scrollers, scrollers.x(), scrollers.y());
			return result;
		}));
	}
}
