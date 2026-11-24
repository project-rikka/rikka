package sh.rikka.client.mods.hud;

import java.util.Locale;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.world.entity.Entity;
import sh.rikka.client.mod.SimpleHudMod;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.EnumSetting;
import sh.rikka.client.setting.ScreenPosition;

public final class SpeedometerMod extends SimpleHudMod {
	private final EnumSetting<SpeedUnit> unit = choice("unit", SpeedUnit.BLOCKS_PER_SECOND);
	private final BoolSetting vertical = bool("vertical", false);

	public SpeedometerMod(ScreenPosition defaultPosition) {
		super("speedometer", defaultPosition, "speed");
	}

	@Override
	protected String label() {
		Minecraft minecraft = Minecraft.getInstance();
		LocalPlayer player = minecraft.player;
		ClientLevel level = minecraft.level;
		if (player == null || level == null) {
			return UNAVAILABLE;
		}
		Entity moving = player.getRootVehicle();
		double dx = moving.getX() - moving.xo;
		double dy = this.vertical.get() ? moving.getY() - moving.yo : 0.0;
		double dz = moving.getZ() - moving.zo;
		double blocksPerSecond = Math.sqrt(dx * dx + dy * dy + dz * dz) * level.tickRateManager().tickrate();
		SpeedUnit shown = this.unit.get();
		String speed = String.format(Locale.ROOT, "%.2f", shown.fromBlocksPerSecond(blocksPerSecond));
		return translate(shown.translationSuffix(), speed);
	}
}
