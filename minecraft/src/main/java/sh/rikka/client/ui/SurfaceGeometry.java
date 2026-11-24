package sh.rikka.client.ui;

import com.mojang.blaze3d.platform.Window;
import org.lwjgl.sdl.SDLError;
import org.lwjgl.sdl.SDLVideo;
import os.rikka.VulkanImage;
import sh.rikka.client.mod.HudMod;

record SurfaceGeometry(float width, float height, float scale, float guiScale) {
	static SurfaceGeometry of(Window window, VulkanImage target, SurfaceScale scale) {
		return switch (scale) {
			case DISPLAY -> scaled(window, target, displayScale(window));
			case HUD -> scaled(window, target, window.getGuiScale() / HudMod.UNITS_PER_GUI_PIXEL);
		};
	}

	private static float displayScale(Window window) {
		float scale = SDLVideo.SDL_GetWindowDisplayScale(window.handle());
		if (!(scale > 0.0F)) {
			throw new IllegalStateException("failed to read the display scale: " + SDLError.SDL_GetError());
		}
		return scale;
	}

	private static SurfaceGeometry scaled(Window window, VulkanImage target, float scale) {
		return new SurfaceGeometry(target.width() / scale, target.height() / scale, scale, window.getGuiScale());
	}

	float hudUnit() {
		return this.guiScale / HudMod.UNITS_PER_GUI_PIXEL / this.scale;
	}

	float fromGui(double coordinate) {
		return (float) (coordinate * this.guiScale / this.scale);
	}

	int toGui(float coordinate) {
		return Math.round(coordinate * this.scale / this.guiScale);
	}
}
