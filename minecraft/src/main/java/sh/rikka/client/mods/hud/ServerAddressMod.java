package sh.rikka.client.mods.hud;

import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ServerData;
import sh.rikka.client.mod.SimpleHudMod;
import sh.rikka.client.setting.ScreenPosition;

public final class ServerAddressMod extends SimpleHudMod {
	public ServerAddressMod(ScreenPosition defaultPosition) {
		super("server_address", defaultPosition, "dns");
	}

	@Override
	protected String label() {
		Minecraft minecraft = Minecraft.getInstance();
		if (minecraft.isLocalServer()) {
			return translate("singleplayer");
		}
		ServerData server = minecraft.getCurrentServer();
		return server == null ? UNAVAILABLE : server.ip;
	}
}
