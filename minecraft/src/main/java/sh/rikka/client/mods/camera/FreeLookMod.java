package sh.rikka.client.mods.camera;

import com.mojang.blaze3d.platform.InputConstants;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.util.Mth;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.mod.Category;
import sh.rikka.client.mod.DefaultBind;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.EnumSetting;
import sh.rikka.client.setting.IntSetting;

public final class FreeLookMod extends Mod {
	private static final float TURN_SCALE = 0.15F;
	private static final float MAX_PITCH = 90.0F;
	private static volatile @Nullable FreeLookMod active;

	private final EnumSetting<Viewpoint> perspective = choice("perspective", Viewpoint.BEHIND);
	private final BoolSetting limitYaw = bool("limit_yaw", false);
	private final IntSetting maxYaw = integer("max_yaw", 90, 10, 180).step(5).visibleWhen(this.limitYaw::get);

	private boolean seeded;
	private float yaw;
	private float pitch;
	private float anchorYaw;

	public FreeLookMod() {
		super("free_look", Category.UTILITY, "360", DefaultBind.hold(InputConstants.KEY_LALT));
	}

	static @Nullable FreeLookMod active() {
		return active;
	}

	Viewpoint viewpoint() {
		return this.perspective.get();
	}

	@Override
	protected void onEnable() {
		this.seeded = false;
		active = this;
	}

	@Override
	protected void onDisable() {
		active = null;
	}

	float yaw(LocalPlayer player) {
		seed(player);
		return this.yaw;
	}

	float pitch(LocalPlayer player) {
		seed(player);
		return this.pitch;
	}

	void turn(LocalPlayer player, double horizontal, double vertical) {
		seed(player);
		this.pitch = Mth.clamp(this.pitch + (float) vertical * TURN_SCALE, -MAX_PITCH, MAX_PITCH);
		float turned = this.yaw + (float) horizontal * TURN_SCALE;
		if (this.limitYaw.get()) {
			float range = this.maxYaw.get();
			turned = Mth.clamp(turned, this.anchorYaw - range, this.anchorYaw + range);
		}
		this.yaw = turned;
	}

	private void seed(LocalPlayer player) {
		if (!this.seeded) {
			this.yaw = player.getYRot();
			this.pitch = player.getXRot();
			this.anchorYaw = this.yaw;
			this.seeded = true;
		}
	}
}
