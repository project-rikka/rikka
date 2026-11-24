package sh.rikka.client.mods.hud;

import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.MouseInputEvent;
import sh.rikka.client.mod.SimpleHudMod;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.ScreenPosition;

public final class CpsDisplayMod extends SimpleHudMod {
	private final BoolSetting rightButton = bool("right_button", true);
	private final ClickRates clicks = new ClickRates();

	public CpsDisplayMod(ScreenPosition defaultPosition) {
		super("cps_display", defaultPosition, "mouse");
	}

	@Override
	protected void onDisable() {
		this.clicks.clear();
	}

	@EventHandler(receiveCancelled = true)
	private void onMouseInput(MouseInputEvent event) {
		this.clicks.record(event);
	}

	@Override
	protected String label() {
		int left = this.clicks.left();
		if (!this.rightButton.get()) {
			return translate("format", left);
		}
		return translate("format_both", left, this.clicks.right());
	}
}
