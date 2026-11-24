package sh.rikka.client.ui;

import com.mojang.blaze3d.platform.Window;
import java.util.function.Function;
import org.jspecify.annotations.Nullable;
import os.rikka.EmbeddedSurface;
import os.rikka.EmbeddedSurfaceBuilder;
import os.rikka.ThemeBackground;
import os.rikka.View;
import os.rikka.VulkanImage;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.TickEvent;
import sh.rikka.client.hud.style.HudLook;
import sh.rikka.client.hud.style.HudStyle;
import sh.rikka.client.mod.HudMod;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.mod.ModManager;
import sh.rikka.client.mod.ModToggleEvent;
import sh.rikka.client.setting.Setting;

final class HudLayer {
	private final ModManager mods;
	private final HudStyle style;
	private final EmbeddedSurface surface;
	private @Nullable SurfaceGeometry geometry;

	HudLayer(Function<EmbeddedSurfaceBuilder, EmbeddedSurface> open, ModManager mods, HudStyle style) {
		this.mods = mods;
		this.style = style;
		this.surface = open.apply(new EmbeddedSurfaceBuilder(this::declare));
		wearSurface();
		for (Mod mod : mods.all()) {
			if (mod instanceof HudMod) {
				for (Setting<?, ?> setting : mod.settings().all()) {
					setting.onChange(value -> invalidate());
				}
			}
		}
		for (Setting<?, ?> setting : style.all()) {
			setting.onChange(value -> invalidate());
		}
		style.surface().onChange(surface -> wearSurface());
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
		if (this.style.isAnimated()) {
			return true;
		}
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

	private void wearSurface() {
		switch (this.style.surface().get()) {
			case BLUR -> this.surface.setBackground(ThemeBackground.GLASS);
			case LIQUID -> this.surface.setBackground(ThemeBackground.LIQUID);
			case NONE, THEME -> this.surface.followBackground();
		}
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
		try (HudLook look = HudLook.of(this.style, view)) {
			for (Mod mod : this.mods.all()) {
				if (mod instanceof HudMod hud && hud.isEnabled()) {
					hud.place(view, look, shown.width(), shown.height());
				}
			}
		}
	}
}
