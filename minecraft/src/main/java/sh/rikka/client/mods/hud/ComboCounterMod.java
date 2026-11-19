package sh.rikka.client.mods.hud;

import net.minecraft.util.Util;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.HitEvent;
import sh.rikka.client.events.HurtEvent;
import sh.rikka.client.mod.SimpleHudMod;
import sh.rikka.client.setting.ScreenPosition;

public final class ComboCounterMod extends SimpleHudMod {
	private static final long TIMEOUT_MILLIS = 2000;

	private int combo;
	private long lastHitAt;

	public ComboCounterMod(ScreenPosition defaultPosition) {
		super("combo_counter", defaultPosition, "local_fire_department", 12);
	}

	@Override
	protected void onDisable() {
		this.combo = 0;
	}

	@EventHandler
	private void onHit(HitEvent event) {
		long now = Util.getMillis();
		this.combo = isActive(now) ? this.combo + 1 : 1;
		this.lastHitAt = now;
	}

	@EventHandler
	private void onHurt(HurtEvent event) {
		if (event.attackerId().isPresent()) {
			this.combo = 0;
		}
	}

	@Override
	protected String label() {
		return isActive(Util.getMillis()) ? translate("format", this.combo) : translate("idle");
	}

	private boolean isActive(long now) {
		return this.combo > 0 && now - this.lastHitAt <= TIMEOUT_MILLIS;
	}
}
