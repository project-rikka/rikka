package sh.rikka.client.resource.legacy;

import java.io.IOException;
import java.util.List;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import sh.rikka.client.resource.legacy.LegacyData.SheetGroup;
import sh.rikka.client.resource.legacy.LegacyData.SheetSprite;

final class SheetSlicing implements Upgrade {
	private static final Logger LOGGER = LoggerFactory.getLogger(SheetSlicing.class);

	private final List<SheetGroup> groups;

	public SheetSlicing(int before) {
		this.groups = LegacyData.sheetGroups().stream().filter(group -> group.before() == before).toList();
	}

	@Override
	public void apply(Conversion conversion) {
		Assets assets = conversion.assets();
		for (SheetGroup group : this.groups) {
			if (!assets.hasTexture(group.sheet())) {
				continue;
			}
			Raster sheet;
			try {
				sheet = assets.textureRaster(group.sheet());
			} catch (IOException failure) {
				LOGGER.warn("skipped unreadable legacy sheet {}: {}", group.sheet(), failure.getMessage());
				continue;
			}
			int referenceWidth = group.referenceWidth(conversion.sourceFormat());
			double scale = sheet.scale(referenceWidth);
			for (SheetSprite sprite : group.sprites()) {
				if (sprite.since() > conversion.sourceFormat()) {
					continue;
				}
				Rect rect = sprite.rect(referenceWidth).scaled(scale);
				if (rect.width() == 0 || rect.height() == 0 || rect.right() > sheet.width() || rect.bottom() > sheet.height() || sheet.isTransparent(rect)) {
					continue;
				}
				assets.putTexture(sprite.sprite(), () -> sheet.crop(rect));
			}
			if (group.retire()) {
				assets.removeTexture(group.sheet());
			}
		}
	}
}
