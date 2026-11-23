package sh.rikka.client.hud;

import sh.rikka.client.mod.HudBounds;
import sh.rikka.client.mod.HudMod;

record HudBox(HudMod mod, float left, float top, float width, float height) {
	static HudBox of(HudMod mod, float screenWidth, float screenHeight) {
		HudBounds bounds = mod.bounds(screenWidth, screenHeight);
		return new HudBox(mod, bounds.left(), bounds.top(), bounds.width(), bounds.height());
	}

	boolean contains(float x, float y) {
		return x >= this.left && x < this.left + this.width && y >= this.top && y < this.top + this.height;
	}

	float right() {
		return this.left + this.width;
	}

	float bottom() {
		return this.top + this.height;
	}

	float centerX() {
		return this.left + this.width / 2.0F;
	}

	float centerY() {
		return this.top + this.height / 2.0F;
	}
}
