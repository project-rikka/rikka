package sh.rikka.client.ui;

import java.util.function.Function;
import org.jspecify.annotations.Nullable;
import os.rikka.EmbeddedSurface;
import os.rikka.EmbeddedSurfaceBuilder;
import os.rikka.View;
import os.rikka.VulkanImage;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.TickEvent;
import sh.rikka.client.mod.HudMod;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.mod.ModManager;
import sh.rikka.client.mod.ModToggleEvent;

final class HudLayer {
	private final ModManager mods;
	private final EmbeddedSurface surface;
	private @Nullable SurfaceGeometry geometry;

	HudLayer(Function<EmbeddedSurfaceBuilder, EmbeddedSurface> open, ModManager mods) {
		this.mods = mods;
		this.surface = open.apply(new EmbeddedSurfaceBuilder(this::declare));
	}

	boolean isShowing() {
		for (Mod mod : this.mods.all()) {
			if (mod instanceof HudMod && mod.isEnabled()) {
				return true;
			}
		}
		return false;
	}

	void draw(VulkanImage target, SurfaceGeometry geometry) {
		if (!geometry.equals(this.geometry)) {
			this.geometry = geometry;
			this.surface.resize(geometry.width(), geometry.height(), geometry.scale());
		}
		this.surface.render(target);
	}

	@EventHandler
	private void onTick(TickEvent.Post event) {
		this.surface.getWindow().invalidate();
	}

	@EventHandler
	private void onToggle(ModToggleEvent event) {
		if (event.mod() instanceof HudMod) {
			this.surface.getWindow().invalidate();
		}
	}

	private void declare(View view) {
		SurfaceGeometry shown = this.geometry;
		if (shown == null) {
			return;
		}
		for (Mod mod : this.mods.all()) {
			if (mod instanceof HudMod hud && hud.isEnabled()) {
				hud.place(view, shown.width(), shown.height());
			}
		}
	}
}
