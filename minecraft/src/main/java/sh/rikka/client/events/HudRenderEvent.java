package sh.rikka.client.events;

import net.minecraft.client.DeltaTracker;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.event.Event;

public final class HudRenderEvent implements Event {
	private @Nullable GuiGraphicsExtractor graphics;
	private @Nullable DeltaTracker deltaTracker;

	HudRenderEvent() {
	}

	void update(GuiGraphicsExtractor graphics, DeltaTracker deltaTracker) {
		this.graphics = graphics;
		this.deltaTracker = deltaTracker;
	}

	void clear() {
		this.graphics = null;
		this.deltaTracker = null;
	}

	public GuiGraphicsExtractor graphics() {
		GuiGraphicsExtractor current = this.graphics;
		if (current == null) {
			throw new IllegalStateException("HudRenderEvent is used outside its dispatch");
		}
		return current;
	}

	public DeltaTracker deltaTracker() {
		DeltaTracker current = this.deltaTracker;
		if (current == null) {
			throw new IllegalStateException("HudRenderEvent is used outside its dispatch");
		}
		return current;
	}

	public float partialTick() {
		return deltaTracker().getGameTimeDeltaPartialTick(false);
	}
}
