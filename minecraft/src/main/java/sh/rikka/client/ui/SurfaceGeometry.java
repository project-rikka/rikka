package sh.rikka.client.ui;

import com.mojang.blaze3d.platform.Window;
import org.lwjgl.sdl.SDLError;
import org.lwjgl.sdl.SDLVideo;
import os.rikka.VulkanImage;

record SurfaceGeometry(float width, float height, float scale, float guiScale) {
	static SurfaceGeometry of(Window window, VulkanImage target) {
		float scale = SDLVideo.SDL_GetWindowDisplayScale(window.handle());
		if (!(scale > 0.0F)) {
			throw new IllegalStateException("failed to read the display scale: " + SDLError.SDL_GetError());
		}
		return new SurfaceGeometry(target.width() / scale, target.height() / scale, scale, window.getGuiScale());
	}

	float fromGui(double coordinate) {
		return (float) (coordinate * this.guiScale / this.scale);
	}

	int toGui(float coordinate) {
		return Math.round(coordinate * this.scale / this.guiScale);
	}
}
