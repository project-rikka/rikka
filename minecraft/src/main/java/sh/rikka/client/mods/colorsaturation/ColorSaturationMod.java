package sh.rikka.client.mods.colorsaturation;

import java.util.List;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.mod.Category;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.setting.DoubleSetting;

public final class ColorSaturationMod extends Mod {
	private static volatile @Nullable ColorSaturationMod active;

	private final DoubleSetting hue = number("hue", 0.0, -180.0, 180.0).step(1.0);
	private final DoubleSetting saturation = number("saturation", 1.0, 0.0, 2.0).step(0.01);
	private final DoubleSetting contrast = number("contrast", 1.0, 0.0, 2.0).step(0.01);
	private final DoubleSetting brightness = number("brightness", 0.0, -1.0, 1.0).step(0.01);

	private volatile ColorGrade grade;

	public ColorSaturationMod() {
		super("color_saturation", Category.VISUAL, "palette");
		this.grade = currentGrade();
		for (DoubleSetting setting : List.of(this.hue, this.saturation, this.contrast, this.brightness)) {
			setting.onChange(value -> this.grade = currentGrade());
		}
	}

	static @Nullable ColorSaturationMod active() {
		return active;
	}

	ColorGrade grade() {
		return this.grade;
	}

	private ColorGrade currentGrade() {
		return ColorGrade.of(this.hue.get(), this.saturation.get(), this.contrast.get(), this.brightness.get());
	}

	@Override
	protected void onEnable() {
		active = this;
	}

	@Override
	protected void onDisable() {
		active = null;
		ColorSaturationHooks.release();
	}
}
