package sh.rikka.client.mods.mechanic;

import net.minecraft.client.KeyMapping;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.world.entity.LivingEntity;

public final class MechanicHooks {
	private MechanicHooks() {
	}

	public static boolean hasMissTime(boolean vanilla) {
		return vanilla && !HitDelayFixMod.isActive();
	}

	public static boolean clearsJumpDelay(LivingEntity entity) {
		return NoJumpDelayMod.isActive() && entity instanceof LocalPlayer;
	}

	public static boolean keyDown(KeyMapping key, boolean down) {
		SnapTapMod snapTap = SnapTapMod.active();
		return snapTap == null ? down : snapTap.resolve(key, down);
	}

	public static void keyPressed(KeyMapping key) {
		SnapTapMod snapTap = SnapTapMod.active();
		if (snapTap != null) {
			snapTap.pressed(key);
		}
	}
}
