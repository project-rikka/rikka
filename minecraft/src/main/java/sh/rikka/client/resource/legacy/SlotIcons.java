package sh.rikka.client.resource.legacy;

import java.util.List;
import java.util.Map;

final class SlotIcons implements Upgrade {
	private static final String PLAIN_SLOT = GuiStyle.CONTAINER_SPRITES + "slot";
	private static final Map<String, String> FRAMED = Map.of(
		"horse/saddle_slot", "saddle",
		"horse/armor_slot", "horse_armor",
		"horse/llama_armor_slot", "llama_armor"
	);
	private static final Map<String, String> ICONS = Map.of(
		"loom/banner_slot", "banner",
		"loom/dye_slot", "dye",
		"loom/pattern_slot", "banner_pattern"
	);

	@Override
	public void apply(Conversion conversion) {
		Assets assets = conversion.assets();
		for (Map.Entry<String, String> icon : ICONS.entrySet()) {
			Renaming.textures(conversion, Map.of(GuiStyle.CONTAINER_SPRITES + icon.getKey(), List.of(GuiStyle.SLOT_SPRITES + icon.getValue())));
		}
		for (Map.Entry<String, String> framed : FRAMED.entrySet()) {
			String id = GuiStyle.CONTAINER_SPRITES + framed.getKey();
			String target = GuiStyle.SLOT_SPRITES + framed.getValue();
			assets.findTexture(id).ifPresent(frame -> {
				assets.removeTexture(id);
				if (assets.hasTexture(target)) {
					return;
				}
				assets.findTexture(PLAIN_SLOT).ifPresent(plain -> assets.putTexture(target, () -> {
					Raster withIcon = frame.raster(assets.decoder());
					Raster empty = plain.raster(assets.decoder());
					Raster reference = empty.width() == withIcon.width() ? empty : empty.resized(withIcon.width(), withIcon.height());
					return SlotIcon.extract(withIcon, reference, withIcon.scale(GuiStyle.SLOT_SIZE));
				}));
			});
		}
	}
}
