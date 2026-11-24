package sh.rikka.client.mods.camera;

import net.minecraft.client.CameraType;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.world.entity.Entity;

public final class CameraHooks {
	private CameraHooks() {
	}

	public static float viewYRot(Entity entity, float vanilla) {
		FreeLookMod freeLook = FreeLookMod.active();
		return freeLook != null && entity instanceof LocalPlayer player ? freeLook.yaw(player) : vanilla;
	}

	public static float viewXRot(Entity entity, float vanilla) {
		FreeLookMod freeLook = FreeLookMod.active();
		return freeLook != null && entity instanceof LocalPlayer player ? freeLook.pitch(player) : vanilla;
	}

	public static boolean freeLookTurn(LocalPlayer player, double horizontal, double vertical) {
		FreeLookMod freeLook = FreeLookMod.active();
		if (freeLook == null) {
			return false;
		}
		freeLook.turn(player, horizontal, vertical);
		return true;
	}

	public static double turnScale() {
		return ZoomMod.sensitivity();
	}

	public static float fov(float vanilla) {
		return ZoomMod.fov(vanilla);
	}

	public static CameraType cameraType(CameraType vanilla) {
		TapLookMod tapLook = TapLookMod.active();
		if (tapLook != null) {
			return tapLook.viewpoint().cameraType();
		}
		FreeLookMod freeLook = FreeLookMod.active();
		return freeLook != null && vanilla.isFirstPerson() ? freeLook.viewpoint().cameraType() : vanilla;
	}
}
