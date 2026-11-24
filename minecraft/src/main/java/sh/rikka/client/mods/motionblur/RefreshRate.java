package sh.rikka.client.mods.motionblur;

import com.mojang.blaze3d.platform.VideoMode;
import com.mojang.blaze3d.platform.Window;

final class RefreshRate {
	private static final float UNKNOWN_HZ = 60.0F;

	private float hz;

	void invalidate() {
		this.hz = 0.0F;
	}

	float hz(Window window) {
		if (this.hz <= 0.0F) {
			this.hz = query(window);
		}
		return this.hz;
	}

	private static float query(Window window) {
		VideoMode mode = window.getActiveVideoMode();
		if (mode == null || mode.getRefreshRate() <= 0.0F) {
			return UNKNOWN_HZ;
		}
		return mode.getRefreshRate();
	}
}
