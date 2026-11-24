package sh.rikka.client.mods.hud;

import com.mojang.blaze3d.platform.InputConstants;
import it.unimi.dsi.fastutil.longs.LongArrayFIFOQueue;
import net.minecraft.client.Minecraft;
import net.minecraft.util.Util;
import sh.rikka.client.events.InputAction;
import sh.rikka.client.events.MouseInputEvent;

final class ClickRates {
	private static final long WINDOW_MILLIS = 1000;

	private final LongArrayFIFOQueue left = new LongArrayFIFOQueue();
	private final LongArrayFIFOQueue right = new LongArrayFIFOQueue();

	void record(MouseInputEvent event) {
		if (event.action() != InputAction.PRESS || Minecraft.getInstance().gui.screen() != null) {
			return;
		}
		switch (event.input().button()) {
			case InputConstants.MOUSE_BUTTON_LEFT -> record(this.left);
			case InputConstants.MOUSE_BUTTON_RIGHT -> record(this.right);
			default -> {
			}
		}
	}

	int left() {
		return perSecond(this.left);
	}

	int right() {
		return perSecond(this.right);
	}

	void clear() {
		this.left.clear();
		this.right.clear();
	}

	private static void record(LongArrayFIFOQueue clicks) {
		long now = Util.getMillis();
		prune(clicks, now);
		clicks.enqueue(now);
	}

	private static int perSecond(LongArrayFIFOQueue clicks) {
		prune(clicks, Util.getMillis());
		return clicks.size();
	}

	private static void prune(LongArrayFIFOQueue clicks, long now) {
		while (!clicks.isEmpty() && now - clicks.firstLong() > WINDOW_MILLIS) {
			clicks.dequeueLong();
		}
	}
}
