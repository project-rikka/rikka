package sh.rikka.client.mods.hud;

import com.mojang.blaze3d.platform.InputConstants;
import it.unimi.dsi.fastutil.longs.LongArrayFIFOQueue;
import net.minecraft.client.Minecraft;
import net.minecraft.util.Util;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.InputAction;
import sh.rikka.client.events.MouseInputEvent;
import sh.rikka.client.mod.SimpleHudMod;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.ScreenPosition;

public final class CpsDisplayMod extends SimpleHudMod {
	private static final long WINDOW_MILLIS = 1000;

	private final BoolSetting rightButton = bool("right_button", true);
	private final LongArrayFIFOQueue leftClicks = new LongArrayFIFOQueue();
	private final LongArrayFIFOQueue rightClicks = new LongArrayFIFOQueue();

	public CpsDisplayMod(ScreenPosition defaultPosition) {
		super("cps_display", defaultPosition, "mouse", 12);
	}

	@Override
	protected void onDisable() {
		this.leftClicks.clear();
		this.rightClicks.clear();
	}

	@EventHandler(receiveCancelled = true)
	private void onMouseInput(MouseInputEvent event) {
		if (event.action() != InputAction.PRESS || Minecraft.getInstance().gui.screen() != null) {
			return;
		}
		switch (event.input().button()) {
			case InputConstants.MOUSE_BUTTON_LEFT -> record(this.leftClicks);
			case InputConstants.MOUSE_BUTTON_RIGHT -> record(this.rightClicks);
			default -> {
			}
		}
	}

	@Override
	protected String label() {
		long now = Util.getMillis();
		int left = clicksPerSecond(this.leftClicks, now);
		if (!this.rightButton.get()) {
			return translate("format", left);
		}
		return translate("format_both", left, clicksPerSecond(this.rightClicks, now));
	}

	private static void record(LongArrayFIFOQueue clicks) {
		long now = Util.getMillis();
		clicksPerSecond(clicks, now);
		clicks.enqueue(now);
	}

	private static int clicksPerSecond(LongArrayFIFOQueue clicks, long now) {
		while (!clicks.isEmpty() && now - clicks.firstLong() > WINDOW_MILLIS) {
			clicks.dequeueLong();
		}
		return clicks.size();
	}
}
