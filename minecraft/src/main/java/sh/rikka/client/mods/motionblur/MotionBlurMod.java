package sh.rikka.client.mods.motionblur;

import org.jspecify.annotations.Nullable;
import sh.rikka.client.mod.Category;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.DoubleSetting;
import sh.rikka.client.setting.EnumSetting;

public final class MotionBlurMod extends Mod {
	private static volatile @Nullable MotionBlurMod active;

	private final DoubleSetting strength = number("strength", 1.0, 0.0, 2.0).step(0.01);
	private final EnumSetting<BlurAlgorithm> algorithm = choice("algorithm", BlurAlgorithm.VELOCITY);
	private final EnumSetting<BlurProfile> profile = choice("profile", BlurProfile.UNIFORM)
		.visibleWhen(() -> this.algorithm.get().usesVelocity());
	private final BoolSetting refreshRateScaling = bool("refresh_rate_scaling", true)
		.visibleWhen(() -> this.algorithm.is(BlurAlgorithm.VELOCITY));

	public MotionBlurMod() {
		super("motion_blur", Category.VISUAL, "motion_blur");
	}

	static @Nullable MotionBlurMod active() {
		return active;
	}

	float strength() {
		return this.strength.getFloat();
	}

	BlurAlgorithm algorithm() {
		return this.algorithm.get();
	}

	BlurProfile profile() {
		return this.profile.get();
	}

	boolean scalesWithRefreshRate() {
		return this.refreshRateScaling.get();
	}

	@Override
	protected void onEnable() {
		active = this;
	}

	@Override
	protected void onDisable() {
		active = null;
		MotionBlurHooks.release();
	}
}
