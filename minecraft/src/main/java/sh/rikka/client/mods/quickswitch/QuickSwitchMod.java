package sh.rikka.client.mods.quickswitch;

import com.mojang.blaze3d.platform.InputConstants;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.EnumMap;
import java.util.List;
import java.util.Map;
import net.minecraft.client.Minecraft;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.world.entity.player.Inventory;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.event.CancellableEvent;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.InputAction;
import sh.rikka.client.events.KeyInputEvent;
import sh.rikka.client.events.MouseInputEvent;
import sh.rikka.client.mod.Category;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.KeybindSetting;

public final class QuickSwitchMod extends Mod {
	private final Map<HotbarItem, KeybindSetting> binds = new EnumMap<>(HotbarItem.class);
	private final BoolSetting cycle;

	public QuickSwitchMod() {
		super("quick_switch", Category.UTILITY, "swap_horiz");
		for (HotbarItem item : HotbarItem.values()) {
			this.binds.put(item, keybind(item.id()));
		}
		this.cycle = bool("cycle", true);
	}

	@EventHandler
	private void onKeyInput(KeyInputEvent event) {
		handle(event, event.key(), event.action());
	}

	@EventHandler
	private void onMouseInput(MouseInputEvent event) {
		handle(event, event.key(), event.action());
	}

	private void handle(CancellableEvent event, InputConstants.Key key, InputAction action) {
		if (action != InputAction.PRESS) {
			return;
		}
		Minecraft minecraft = Minecraft.getInstance();
		LocalPlayer player = minecraft.player;
		if (player == null || minecraft.level == null || minecraft.gui.screen() != null || player.isSpectator()) {
			return;
		}
		@Nullable HotbarItem item = bound(key);
		if (item == null) {
			return;
		}
		event.cancel();
		Inventory inventory = player.getInventory();
		List<Integer> slots = ranked(inventory, item, new ItemScores(minecraft.level.registryAccess()));
		if (slots.isEmpty()) {
			return;
		}
		int current = slots.indexOf(inventory.getSelectedSlot());
		int next = current >= 0 && this.cycle.get() ? (current + 1) % slots.size() : 0;
		inventory.setSelectedSlot(slots.get(next));
	}

	private @Nullable HotbarItem bound(InputConstants.Key key) {
		for (Map.Entry<HotbarItem, KeybindSetting> bind : this.binds.entrySet()) {
			if (bind.getValue().matches(key)) {
				return bind.getKey();
			}
		}
		return null;
	}

	private static List<Integer> ranked(Inventory inventory, HotbarItem item, ItemScores scores) {
		List<Ranked> matches = new ArrayList<>(Inventory.getSelectionSize());
		for (int slot = 0; slot < Inventory.getSelectionSize(); slot++) {
			double score = item.score(inventory.getItem(slot), scores);
			if (score != ItemScores.UNMATCHED) {
				matches.add(new Ranked(slot, score));
			}
		}
		matches.sort(Comparator.comparingDouble(Ranked::score).reversed().thenComparingInt(Ranked::slot));
		return matches.stream().map(Ranked::slot).toList();
	}

	private record Ranked(int slot, double score) {
	}
}
