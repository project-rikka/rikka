package sh.rikka.client.mods.visual;

import net.minecraft.client.player.RemotePlayer;
import net.minecraft.world.entity.Avatar;
import net.minecraft.world.entity.HumanoidArm;

public final class VisualHooks {
	private VisualHooks() {
	}

	public static float lightmapBrightness(float vanilla) {
		FullbrightMod fullbright = FullbrightMod.active();
		return fullbright == null ? vanilla : fullbright.brightness(vanilla);
	}

	public static HumanoidArm mainArm(Avatar avatar, HumanoidArm vanilla) {
		ForceMainHandMod forceMainHand = ForceMainHandMod.active();
		return forceMainHand != null && avatar instanceof RemotePlayer ? forceMainHand.arm() : vanilla;
	}
}
