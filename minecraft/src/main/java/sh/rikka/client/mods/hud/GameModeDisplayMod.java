package sh.rikka.client.mods.hud;

import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.MultiPlayerGameMode;
import sh.rikka.client.mod.SimpleHudMod;
import sh.rikka.client.setting.ScreenPosition;

public final class GameModeDisplayMod extends SimpleHudMod {
	public GameModeDisplayMod(ScreenPosition defaultPosition) {
		super("game_mode_display", defaultPosition, "sports_esports");
	}

	@Override
	protected String label() {
		MultiPlayerGameMode gameMode = Minecraft.getInstance().gameMode;
		return gameMode == null ? UNAVAILABLE : gameMode.getPlayerMode().getShortDisplayName().getString();
	}
}
