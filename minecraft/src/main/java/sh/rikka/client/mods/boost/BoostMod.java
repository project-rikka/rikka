package sh.rikka.client.mods.boost;

import java.util.EnumMap;
import java.util.Map;
import sh.rikka.client.mod.Category;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.mods.boost.scale.RenderScale;
import sh.rikka.client.setting.BoolSetting;

public final class BoostMod extends Mod {
	private final Map<BoostFeature, BoolSetting> toggles = new EnumMap<>(BoostFeature.class);

	public BoostMod() {
		super("boost", Category.UTILITY, "bolt");
		for (BoostFeature feature : BoostFeature.values()) {
			this.toggles.put(feature, bool(feature.id(), feature.enabledByDefault()).onChange(on -> apply()));
		}
		BoolSetting scaled = this.toggles.get(BoostFeature.RENDER_SCALE);
		RenderScale.setScale(
			number("render_scale_value", 0.75, 0.5, 1.0).step(0.05).visibleWhen(scaled::get).onChange(RenderScale::setScale).get()
		);
		RenderScale.setSharpness(
			number("render_scale_sharpness", 0.8, 0.0, 1.0).step(0.05).visibleWhen(scaled::get).onChange(RenderScale::setSharpness).get()
		);
	}

	@Override
	protected void onEnable() {
		apply();
	}

	@Override
	protected void onDisable() {
		apply();
	}

	private void apply() {
		boolean enabled = isEnabled();
		this.toggles.forEach((feature, toggle) -> feature.setActive(enabled && toggle.get()));
	}
}
