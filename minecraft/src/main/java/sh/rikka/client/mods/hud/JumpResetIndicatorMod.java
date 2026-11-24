package sh.rikka.client.mods.hud;

import net.minecraft.client.player.LocalPlayer;
import net.minecraft.util.Util;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.HurtEvent;
import sh.rikka.client.events.JumpEvent;
import sh.rikka.client.mod.SimpleHudMod;
import sh.rikka.client.setting.IntSetting;
import sh.rikka.client.setting.ScreenPosition;

public final class JumpResetIndicatorMod extends SimpleHudMod {
	private static final long SHOWN_MILLIS = 2500;
	private static final int MAX_WINDOW_TICKS = 20;

	private final IntSetting window = integer("window", 10, 1, MAX_WINDOW_TICKS);
	private @Nullable Moment hurt;
	private @Nullable Moment jump;

	public JumpResetIndicatorMod(ScreenPosition defaultPosition) {
		super("jump_reset_indicator", defaultPosition, "keyboard_double_arrow_up");
	}

	@Override
	protected void onDisable() {
		this.hurt = null;
		this.jump = null;
	}

	@EventHandler
	private void onHurt(HurtEvent event) {
		if (event.attackerId().isPresent()) {
			this.hurt = Moment.of(event.player());
		}
	}

	@EventHandler
	private void onJump(JumpEvent event) {
		this.jump = Moment.of(event.player());
	}

	@Override
	protected String label() {
		Moment hurt = this.hurt;
		Moment jump = this.jump;
		if (hurt == null || jump == null || Util.getMillis() - Math.max(hurt.at(), jump.at()) > SHOWN_MILLIS) {
			return translate("idle");
		}
		int offset = jump.tick() - hurt.tick() - 1;
		if (Math.abs(offset) >= this.window.get()) {
			return translate("idle");
		}
		if (offset == 0) {
			return translate("perfect");
		}
		return offset < 0 ? translate("early", -offset) : translate("late", offset);
	}

	private record Moment(int tick, long at) {
		static Moment of(LocalPlayer player) {
			return new Moment(player.tickCount, Util.getMillis());
		}
	}
}
