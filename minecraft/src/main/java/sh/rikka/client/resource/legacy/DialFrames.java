package sh.rikka.client.resource.legacy;

import java.io.IOException;
import java.util.Locale;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

final class DialFrames implements Upgrade {
	private static final Logger LOGGER = LoggerFactory.getLogger(DialFrames.class);
	private static final Dial[] DIALS = {new Dial("items/clock", 64), new Dial("items/compass", 32)};

	@Override
	public void apply(Conversion conversion) {
		Assets assets = conversion.assets();
		for (Dial dial : DIALS) {
			if (!assets.hasTexture(dial.strip())) {
				continue;
			}
			Raster strip;
			try {
				strip = assets.textureRaster(dial.strip());
			} catch (IOException failure) {
				LOGGER.warn("skipped unreadable legacy dial {}: {}", dial.strip(), failure.getMessage());
				continue;
			}
			int size = strip.width();
			int available = Math.max(1, strip.height() / size);
			assets.removeTexture(dial.strip());
			for (int frame = 0; frame < dial.frames(); frame++) {
				int source = frame * available / dial.frames();
				Rect rect = Rect.of(0, source * size, size, size);
				assets.putTexture(dial.strip() + String.format(Locale.ROOT, "_%02d", frame), () -> strip.crop(rect));
			}
		}
	}

	private record Dial(String strip, int frames) {
	}
}
