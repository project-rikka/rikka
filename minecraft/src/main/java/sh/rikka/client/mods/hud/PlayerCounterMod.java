package sh.rikka.client.mods.hud;

import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.multiplayer.ClientPacketListener;
import sh.rikka.client.mod.SimpleHudMod;
import sh.rikka.client.setting.EnumSetting;
import sh.rikka.client.setting.ScreenPosition;

public final class PlayerCounterMod extends SimpleHudMod {
	private final EnumSetting<PlayerScope> scope = choice("scope", PlayerScope.ONLINE);

	public PlayerCounterMod(ScreenPosition defaultPosition) {
		super("player_counter", defaultPosition, "group", 14);
	}

	@Override
	protected String label() {
		Minecraft minecraft = Minecraft.getInstance();
		return switch (this.scope.get()) {
			case ONLINE -> {
				ClientPacketListener connection = minecraft.getConnection();
				yield connection == null ? UNAVAILABLE : translate("format", connection.getListedOnlinePlayers().size());
			}
			case NEARBY -> {
				ClientLevel level = minecraft.level;
				yield level == null ? UNAVAILABLE : translate("format", level.players().size());
			}
		};
	}
}
