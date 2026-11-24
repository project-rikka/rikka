package sh.rikka.client.mods.visual;

import java.util.Locale;
import net.minecraft.client.renderer.entity.state.TntRenderState;
import net.minecraft.network.chat.Component;
import net.minecraft.util.Mth;
import net.minecraft.world.entity.EntityAttachment;
import net.minecraft.world.entity.item.PrimedTnt;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.mod.Category;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.ColorSetting;
import sh.rikka.client.setting.IntSetting;

public final class TntTimerMod extends Mod {
	private static final float TICKS_PER_SECOND = 20.0F;
	private static final float SAFE_HUE = 1.0F / 3.0F;
	private static final float SATURATION = 0.85F;
	private static final String[] FORMATS = {"%.0f", "%.1f", "%.2f"};
	private static volatile @Nullable TntTimerMod active;

	private final IntSetting decimals = integer("decimals", 2, 0, FORMATS.length - 1);
	private final IntSetting distance = integer("distance", 32, 8, 64).step(4);
	private final BoolSetting gradient = bool("gradient", true);
	private final ColorSetting color = color("color", 0xFFFFFFFF).opaque().visibleWhen(() -> !this.gradient.get());

	public TntTimerMod() {
		super("tnt_timer", Category.VISUAL, "bomb");
	}

	static @Nullable TntTimerMod active() {
		return active;
	}

	@Override
	protected void onEnable() {
		active = this;
	}

	@Override
	protected void onDisable() {
		active = null;
	}

	void extracted(PrimedTnt tnt, TntRenderState state) {
		if (state.isInvisible || state.distanceToCameraSq > Mth.square((double) this.distance.get())) {
			return;
		}
		float remainingTicks = Math.max(0.0F, state.fuseRemainingInTicks - 1.0F);
		String seconds = String.format(Locale.ROOT, FORMATS[this.decimals.get()], remainingTicks / TICKS_PER_SECOND);
		Component timer = Component.literal(seconds).withColor(color(remainingTicks));
		if (state.nameTag == null) {
			state.nameTag = timer;
			state.nameTagAttachment = tnt.getAttachments().getNullable(EntityAttachment.NAME_TAG, 0, tnt.getYRot());
		} else {
			state.scoreText = timer;
		}
	}

	private int color(float remainingTicks) {
		if (!this.gradient.get()) {
			return this.color.argb() & 0xFFFFFF;
		}
		float safety = Math.clamp(remainingTicks / PrimedTnt.DEFAULT_FUSE_TIME, 0.0F, 1.0F);
		return Mth.hsvToRgb(safety * SAFE_HUE, SATURATION, 1.0F);
	}
}
