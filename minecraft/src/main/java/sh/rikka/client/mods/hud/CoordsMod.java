package sh.rikka.client.mods.hud;

import java.util.Locale;
import net.minecraft.client.Minecraft;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.core.BlockPos;
import sh.rikka.client.mod.SimpleHudMod;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.ScreenPosition;

public final class CoordsMod extends SimpleHudMod {
	private final BoolSetting direction = bool("direction", true);

	public CoordsMod(ScreenPosition defaultPosition) {
		super("coords", defaultPosition, "explore");
	}

	@Override
	protected String label() {
		LocalPlayer player = Minecraft.getInstance().player;
		if (player == null) {
			return UNAVAILABLE;
		}
		BlockPos position = player.blockPosition();
		String coordinates = String.format(Locale.ROOT, "X %d  Y %d  Z %d", position.getX(), position.getY(), position.getZ());
		if (!this.direction.get()) {
			return coordinates;
		}
		return coordinates + "  " + translate(Compass.fromYaw(player.getYRot()).translationSuffix());
	}
}
