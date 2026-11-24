package sh.rikka.client.mods.translate;

import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.client.multiplayer.PlayerInfo;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.translate.markup.Protection;

final class PlayerNames {
	private @Nullable Protection protection;

	Protection protection() {
		Protection current = this.protection;
		if (current == null) {
			current = Protection.withNames(names());
			this.protection = current;
		}
		return current;
	}

	void invalidate() {
		this.protection = null;
	}

	private static List<String> names() {
		Minecraft minecraft = Minecraft.getInstance();
		ClientPacketListener connection = minecraft.getConnection();
		List<String> names = new ArrayList<>();
		if (connection != null) {
			for (PlayerInfo info : connection.getOnlinePlayers()) {
				names.add(info.getProfile().name());
			}
		}
		names.add(minecraft.getUser().getName());
		return names;
	}
}
