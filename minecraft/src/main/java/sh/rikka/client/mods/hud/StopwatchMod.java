package sh.rikka.client.mods.hud;

import com.mojang.blaze3d.platform.InputConstants;
import java.time.Duration;
import java.util.Locale;
import net.minecraft.client.Minecraft;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.InputAction;
import sh.rikka.client.events.KeyInputEvent;
import sh.rikka.client.events.MouseInputEvent;
import sh.rikka.client.mod.SimpleHudMod;
import sh.rikka.client.setting.KeybindSetting;
import sh.rikka.client.setting.ScreenPosition;

public final class StopwatchMod extends SimpleHudMod {
	private static final int MILLIS_PER_CENTISECOND = 10;

	private final KeybindSetting startStop = keybind("start_stop", InputConstants.KEY_J);
	private final KeybindSetting reset = keybind("reset", InputConstants.KEY_K);
	private long accumulatedNanos;
	private long startedAt;
	private boolean running;

	public StopwatchMod(ScreenPosition defaultPosition) {
		super("stopwatch", defaultPosition, "timer", 11);
	}

	@Override
	protected void onEnable() {
		resetTimer();
	}

	@EventHandler
	private void onKeyInput(KeyInputEvent event) {
		handle(event.key(), event.action());
	}

	@EventHandler
	private void onMouseInput(MouseInputEvent event) {
		handle(event.key(), event.action());
	}

	private void handle(InputConstants.Key key, InputAction action) {
		if (action != InputAction.PRESS || Minecraft.getInstance().gui.screen() != null) {
			return;
		}
		if (this.startStop.matches(key)) {
			startOrStop();
		} else if (this.reset.matches(key)) {
			resetTimer();
		}
	}

	private void startOrStop() {
		long now = System.nanoTime();
		if (this.running) {
			this.accumulatedNanos += now - this.startedAt;
		} else {
			this.startedAt = now;
		}
		this.running = !this.running;
	}

	private void resetTimer() {
		this.accumulatedNanos = 0;
		this.running = false;
	}

	@Override
	protected String label() {
		long elapsedNanos = this.accumulatedNanos + (this.running ? System.nanoTime() - this.startedAt : 0);
		Duration elapsed = Duration.ofNanos(elapsedNanos);
		int centiseconds = elapsed.toMillisPart() / MILLIS_PER_CENTISECOND;
		if (elapsed.toHours() > 0) {
			return String.format(Locale.ROOT, "%d:%02d:%02d.%02d", elapsed.toHours(), elapsed.toMinutesPart(), elapsed.toSecondsPart(), centiseconds);
		}
		return String.format(Locale.ROOT, "%02d:%02d.%02d", elapsed.toMinutesPart(), elapsed.toSecondsPart(), centiseconds);
	}
}
