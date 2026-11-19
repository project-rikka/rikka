package sh.rikka.client.net;

import java.util.OptionalLong;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.client.multiplayer.PlayerInfo;
import net.minecraft.client.player.LocalPlayer;

public final class TabListLatency {
	private TabListLatency() {
	}

	public static OptionalLong of(Minecraft minecraft) {
		ClientPacketListener connection = minecraft.getConnection();
		LocalPlayer player = minecraft.player;
		if (connection == null || player == null) {
			return OptionalLong.empty();
		}
		PlayerInfo info = connection.getPlayerInfo(player.getUUID());
		return info == null ? OptionalLong.empty() : OptionalLong.of(info.getLatency());
	}
}
