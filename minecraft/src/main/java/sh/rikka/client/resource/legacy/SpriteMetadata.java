package sh.rikka.client.resource.legacy;

import java.io.IOException;
import java.util.Optional;

final class SpriteMetadata implements Upgrade {
	@Override
	public void apply(Conversion conversion) throws IOException {
		Assets assets = conversion.assets();
		for (String path : assets.paths(Assets.TEXTURES + GuiStyle.SPRITES)) {
			String meta = path + Assets.MCMETA;
			if (!path.endsWith(Assets.PNG) || assets.has(meta)) {
				continue;
			}
			Optional<byte[]> vanilla = conversion.vanilla().read(meta);
			if (vanilla.isPresent()) {
				assets.put(meta, Asset.bytes(vanilla.get()));
			}
		}
	}
}
