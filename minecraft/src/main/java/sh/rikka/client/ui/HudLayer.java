package sh.rikka.client.ui;

import com.mojang.blaze3d.platform.Window;
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
import sh.rikka.client.setting.Setting;

final class HudLayer {
	private final ModManager mods;
	private final EmbeddedSurface surface;
	private @Nullable SurfaceGeometry geometry;

	HudLayer(Function<EmbeddedSurfaceBuilder, EmbeddedSurface> open, ModManager mods) {
		this.mods = mods;
		this.surface = open.apply(new EmbeddedSurfaceBuilder(this::declare));
		for (Mod mod : mods.all()) {
			if (mod instanceof HudMod) {
				for (Setting<?, ?> setting : mod.settings().all()) {
					setting.onChange(value -> invalidate());
				}
			}
		}
	}

	boolean isShowing() {
		for (Mod mod : this.mods.all()) {
			if (mod instanceof HudMod && mod.isEnabled()) {
				return true;
			}
		}
		return false;
	}

	void draw(VulkanImage target, Window window) {
		SurfaceGeometry geometry = SurfaceGeometry.of(window, target, SurfaceScale.HUD);
		if (!geometry.equals(this.geometry)) {
			this.geometry = geometry;
			this.surface.resize(geometry.width(), geometry.height(), geometry.scale());
		}
		if (isAnimating()) {
			invalidate();
		}
		this.surface.render(target);
	}

	private boolean isAnimating() {
		for (Mod mod : this.mods.all()) {
			if (mod instanceof HudMod hud && hud.isEnabled() && hud.animating()) {
				return true;
			}
		}
		return false;
	}

	void invalidate() {
		this.surface.getWindow().invalidate();
	}

	@EventHandler
	private void onTick(TickEvent.Post event) {
		invalidate();
	}

	@EventHandler
	private void onToggle(ModToggleEvent event) {
		if (event.mod() instanceof HudMod) {
			invalidate();
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
